---- MODULE SumeragiFrontierSlotTrackerGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for the exact-frontier slot tracker FSM.

This slice models the observable state/action contract of
`FrontierSlot::new(...)` and `FrontierSlot::step(...)` in
`main_loop/slot_tracker.rs`. Concrete hashes, peers, instants, and durations
are collapsed into representative cases while preserving the safety-critical
branches: constructor mode/phase selection, higher-view owner replacement,
same-candidate duplicate handling, exact body repair, vote/commit-QC evidence,
bounded quorum-timeout rebroadcast before view rotation, deep/passive catch-up,
explicit view advance, finalization, and compatibility-field synchronization.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NewMissingInert == 1
NewMissingExactFetch == 2
NewBodyPresentBlockCreated == 3
NewBodyPresentExactRepair == 4
HigherBlockCreatedWithBody == 5
HigherBlockCreatedMissingBody == 6
SameBlockCreatedDuplicateBodyPreservesRebroadcast == 7
SameBlockCreatedFreshMissingStartsLag == 8
MismatchedLowerBlockCreatedIgnored == 9
BodyAvailableMissingRequestsCommit == 10
BodyAvailableDuplicatePreservesRebroadcast == 11
BodyAvailablePassiveReturnsNormal == 12
VoteObservedMissingUrgentFetch == 13
VoteObservedWithBodyCommitPipeline == 14
VoteObservedDifferentHigherRepairsMissing == 15
VoteObservedDuplicatePreservesRebroadcast == 16
CommitQcMissingUrgentFetch == 17
CommitQcWithBodyCommitPipeline == 18
CommitQcDuplicatePreservesRebroadcast == 19
AuthoritativeSupersedeMissingResetsNormal == 20
AuthoritativeSupersedeBodyNoDirectPipeline == 21
FutureGapHigherExactFetchNormalFetch == 22
FutureGapSameUnarmedNoFetch == 23
FetchRetryDueNormalExactMissingFetch == 24
FetchRetryDueDeepNoFetch == 25
QuorumTimeoutExactMissingFirstRebroadcast == 26
QuorumTimeoutExactMissingSecondViewChange == 27
QuorumTimeoutExactMissingLagExpiredDeep == 28
QuorumTimeoutBodyPresentFirstArmsOnly == 29
QuorumTimeoutBodyPresentSecondViewChange == 30
QuorumTimeoutDeepFirstReenter == 31
QuorumTimeoutDeepSecondViewChange == 32
QuorumTimeoutPassiveReasonOnly == 33
LagWindowExpiredNormalExactDeep == 34
LagWindowExpiredPassiveReasonOnly == 35
LagWindowExpiredDeepReenter == 36
ViewAdvanceImmediate == 37
CommitHeightAtOrAboveFinalizes == 38
CommitHeightBelowNoRetire == 39

Candidates == 1..39

SetModeNormal == 1
SetModeDeepCatchup == 2
SetModeFinalized == 3
PreserveMode == 4
SetOwnerProposalLed == 5
SetOwnerBlockCreatedLed == 6
SetOwnerExactSlotRepair == 7
SetPhaseAwaitBlockCreated == 8
SetPhaseAwaitBody == 9
SetPhaseValidateBody == 10
SetPhaseAwaitCommitQc == 11
BodyMissing == 12
BodyAvailable == 13
ValidationUnknown == 14
ValidationPending == 15
VoteNone == 16
VotesObserved == 17
CommitQcObserved == 18
BlockCreatedSeen == 19
BlockCreatedUnseen == 20
ExactFetchArmed == 21
ExactFetchUnarmed == 22
RequestCommitPipeline == 23
NoCommitPipeline == 24
FetchBody == 25
FetchBodyUrgent == 26
NoFetchBody == 27
EnterDeepCatchup == 28
NoDeepCatchup == 29
RequestViewChange == 30
NoViewChange == 31
RetireSlot == 32
NoRetire == 33
IncrementGeneration == 34
PreserveGeneration == 35
UnlockOwner == 36
UpdateCandidate == 37
PreserveCandidate == 38
ResetQuorumProgress == 39
RecordVoteProgress == 40
RecordCommitQcProgress == 41
RecordBodyProgress == 42
RecordBlockProgress == 43
PreserveRebroadcastGuard == 44
ArmRebroadcastGuard == 45
ClearRebroadcastGuard == 46
NoteLag == 47
RecordDeepReason == 48
RecordLastReason == 49
TrackRequester == 50
UpdateActiveView == 51
SyncCompatFields == 52
IgnoreMismatched == 53
NoUrgentFetch == 54
NoTrackRequester == 55

Actions == 1..55

NoExternalActions ==
  {NoCommitPipeline, NoFetchBody, NoUrgentFetch, NoDeepCatchup,
   NoViewChange, NoRetire}

NoTerminalActions == {NoViewChange, NoRetire}

ConstructorCommon ==
  {VoteNone, PreserveGeneration, ClearRebroadcastGuard, SyncCompatFields}

SpecActions(candidate) ==
  CASE candidate = NewMissingInert ->
      ConstructorCommon \cup {SetModeNormal, SetOwnerProposalLed,
        SetPhaseAwaitBlockCreated, BodyMissing, ValidationUnknown,
        BlockCreatedUnseen, ExactFetchUnarmed, NoteLag, NoTrackRequester}
    [] candidate = NewMissingExactFetch ->
      ConstructorCommon \cup {SetModeNormal, SetOwnerExactSlotRepair,
        SetPhaseAwaitBody, BodyMissing, ValidationUnknown,
        BlockCreatedUnseen, ExactFetchArmed, NoteLag, TrackRequester}
    [] candidate = NewBodyPresentBlockCreated ->
      ConstructorCommon \cup {SetModeNormal, SetOwnerBlockCreatedLed,
        SetPhaseValidateBody, BodyAvailable, ValidationPending,
        BlockCreatedSeen, ExactFetchArmed, NoTrackRequester}
    [] candidate = NewBodyPresentExactRepair ->
      ConstructorCommon \cup {SetModeNormal, SetOwnerExactSlotRepair,
        SetPhaseValidateBody, BodyAvailable, ValidationPending,
        BlockCreatedUnseen, ExactFetchUnarmed, NoTrackRequester}
    [] candidate = HigherBlockCreatedWithBody ->
      {SetModeNormal, SetOwnerBlockCreatedLed, SetPhaseValidateBody,
       BodyAvailable, ValidationPending, VoteNone, BlockCreatedSeen,
       ExactFetchArmed, RequestCommitPipeline, NoFetchBody, NoUrgentFetch,
       NoDeepCatchup, NoViewChange, NoRetire, IncrementGeneration,
       UnlockOwner, UpdateCandidate, ResetQuorumProgress, RecordBlockProgress,
       ClearRebroadcastGuard, UpdateActiveView, SyncCompatFields}
    [] candidate = HigherBlockCreatedMissingBody ->
      NoExternalActions \cup {SetModeNormal, SetOwnerBlockCreatedLed,
        SetPhaseAwaitBody, BodyMissing, ValidationUnknown, VoteNone,
        BlockCreatedSeen, ExactFetchArmed, IncrementGeneration, UnlockOwner,
        UpdateCandidate, ResetQuorumProgress, RecordBlockProgress,
        ClearRebroadcastGuard, NoteLag, UpdateActiveView, SyncCompatFields}
    [] candidate = SameBlockCreatedDuplicateBodyPreservesRebroadcast ->
      NoExternalActions \cup {PreserveMode, SetOwnerBlockCreatedLed,
        SetPhaseValidateBody, BodyAvailable, ValidationPending,
        BlockCreatedSeen, ExactFetchArmed, PreserveGeneration,
        PreserveCandidate, PreserveRebroadcastGuard, SyncCompatFields}
    [] candidate = SameBlockCreatedFreshMissingStartsLag ->
      NoExternalActions \cup {PreserveMode, SetOwnerBlockCreatedLed,
        SetPhaseAwaitBody, BodyMissing, ValidationUnknown, BlockCreatedSeen,
        ExactFetchArmed, PreserveGeneration, PreserveCandidate, NoteLag,
        PreserveRebroadcastGuard, TrackRequester, SyncCompatFields}
    [] candidate = MismatchedLowerBlockCreatedIgnored ->
      NoExternalActions \cup {PreserveMode, PreserveGeneration,
        PreserveCandidate, PreserveRebroadcastGuard, IgnoreMismatched,
        SyncCompatFields}
    [] candidate = BodyAvailableMissingRequestsCommit ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseValidateBody,
       BodyAvailable, ValidationPending, RequestCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, RecordBodyProgress,
       ClearRebroadcastGuard, SyncCompatFields}
    [] candidate = BodyAvailableDuplicatePreservesRebroadcast ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseValidateBody,
       BodyAvailable, ValidationPending, NoCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, PreserveRebroadcastGuard,
       SyncCompatFields}
    [] candidate = BodyAvailablePassiveReturnsNormal ->
      {SetModeNormal, SetOwnerExactSlotRepair, SetPhaseValidateBody,
       BodyAvailable, ValidationPending, RequestCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, RecordBodyProgress,
       ClearRebroadcastGuard, SyncCompatFields}
    [] candidate = VoteObservedMissingUrgentFetch ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseAwaitBody, BodyMissing,
       VotesObserved, ExactFetchArmed, NoCommitPipeline, FetchBody,
       FetchBodyUrgent, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, RecordVoteProgress,
       ClearRebroadcastGuard, SyncCompatFields}
    [] candidate = VoteObservedWithBodyCommitPipeline ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseAwaitCommitQc,
       BodyAvailable, VotesObserved, RequestCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, RecordVoteProgress,
       ClearRebroadcastGuard, SyncCompatFields}
    [] candidate = VoteObservedDifferentHigherRepairsMissing ->
      NoExternalActions \cup {PreserveMode, SetOwnerExactSlotRepair,
        SetPhaseAwaitBody, BodyMissing, ValidationUnknown, VotesObserved,
        BlockCreatedUnseen, ExactFetchArmed, IncrementGeneration, UnlockOwner,
        UpdateCandidate, NoteLag, UpdateActiveView, SyncCompatFields}
    [] candidate = VoteObservedDuplicatePreservesRebroadcast ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseAwaitCommitQc,
       BodyAvailable, VotesObserved, RequestCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, PreserveRebroadcastGuard,
       SyncCompatFields}
    [] candidate = CommitQcMissingUrgentFetch ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseAwaitBody, BodyMissing,
       CommitQcObserved, ExactFetchArmed, NoCommitPipeline, FetchBody,
       FetchBodyUrgent, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, RecordCommitQcProgress,
       ClearRebroadcastGuard, SyncCompatFields}
    [] candidate = CommitQcWithBodyCommitPipeline ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseAwaitCommitQc,
       BodyAvailable, CommitQcObserved, RequestCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, RecordCommitQcProgress,
       ClearRebroadcastGuard, SyncCompatFields}
    [] candidate = CommitQcDuplicatePreservesRebroadcast ->
      {PreserveMode, SetOwnerExactSlotRepair, SetPhaseAwaitCommitQc,
       BodyAvailable, CommitQcObserved, RequestCommitPipeline, NoFetchBody,
       NoUrgentFetch, NoDeepCatchup, NoViewChange, NoRetire,
       PreserveGeneration, PreserveCandidate, PreserveRebroadcastGuard,
       SyncCompatFields}
    [] candidate = AuthoritativeSupersedeMissingResetsNormal ->
      NoExternalActions \cup {SetModeNormal, SetOwnerBlockCreatedLed,
        SetPhaseAwaitBody, BodyMissing, ValidationUnknown, VoteNone,
        BlockCreatedSeen, ExactFetchArmed, IncrementGeneration, UnlockOwner,
        UpdateCandidate, ResetQuorumProgress, RecordBlockProgress,
        ClearRebroadcastGuard, TrackRequester, UpdateActiveView,
        SyncCompatFields}
    [] candidate = AuthoritativeSupersedeBodyNoDirectPipeline ->
      NoExternalActions \cup {SetModeNormal, SetOwnerBlockCreatedLed,
        SetPhaseValidateBody, BodyAvailable, ValidationPending, VoteNone,
        BlockCreatedSeen, ExactFetchArmed, IncrementGeneration, UnlockOwner,
        UpdateCandidate, ResetQuorumProgress, RecordBlockProgress,
        ClearRebroadcastGuard, UpdateActiveView, SyncCompatFields}
    [] candidate = FutureGapHigherExactFetchNormalFetch ->
      {SetModeNormal, SetOwnerExactSlotRepair, SetPhaseAwaitBody,
       BodyMissing, ValidationUnknown, VoteNone, BlockCreatedUnseen,
       ExactFetchArmed, NoCommitPipeline, FetchBody, NoUrgentFetch,
       NoDeepCatchup, NoViewChange, NoRetire, IncrementGeneration,
       UnlockOwner, UpdateCandidate, ResetQuorumProgress,
       ClearRebroadcastGuard, NoteLag, UpdateActiveView, TrackRequester,
       SyncCompatFields}
    [] candidate = FutureGapSameUnarmedNoFetch ->
      NoExternalActions \cup {SetModeNormal, SetOwnerExactSlotRepair,
        SetPhaseAwaitBody, BodyMissing, ExactFetchUnarmed,
        PreserveGeneration, PreserveCandidate, PreserveRebroadcastGuard,
        NoteLag, SyncCompatFields}
    [] candidate = FetchRetryDueNormalExactMissingFetch ->
      {PreserveMode, PreserveGeneration, PreserveCandidate,
       PreserveRebroadcastGuard, NoCommitPipeline, FetchBody, NoUrgentFetch,
       NoDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = FetchRetryDueDeepNoFetch ->
      NoExternalActions \cup {PreserveMode, PreserveGeneration,
        PreserveCandidate, PreserveRebroadcastGuard, SyncCompatFields}
    [] candidate = QuorumTimeoutExactMissingFirstRebroadcast ->
      {PreserveMode, PreserveGeneration, PreserveCandidate,
       ArmRebroadcastGuard, NoCommitPipeline, FetchBody, FetchBodyUrgent,
       NoDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = QuorumTimeoutExactMissingSecondViewChange ->
      {PreserveMode, SetOwnerExactSlotRepair, PreserveGeneration,
       PreserveCandidate, ClearRebroadcastGuard, NoCommitPipeline,
       NoFetchBody, NoUrgentFetch, NoDeepCatchup, RequestViewChange,
       NoRetire, UpdateActiveView, SyncCompatFields}
    [] candidate = QuorumTimeoutExactMissingLagExpiredDeep ->
      {SetModeDeepCatchup, SetOwnerExactSlotRepair, PreserveGeneration,
       PreserveCandidate, ClearRebroadcastGuard, RecordDeepReason,
       RecordLastReason, NoCommitPipeline, NoFetchBody, NoUrgentFetch,
       EnterDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = QuorumTimeoutBodyPresentFirstArmsOnly ->
      {PreserveMode, PreserveGeneration, PreserveCandidate,
       ArmRebroadcastGuard, NoCommitPipeline, NoFetchBody, NoUrgentFetch,
       NoDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = QuorumTimeoutBodyPresentSecondViewChange ->
      {PreserveMode, SetOwnerExactSlotRepair, PreserveGeneration,
       PreserveCandidate, ClearRebroadcastGuard, NoCommitPipeline,
       NoFetchBody, NoUrgentFetch, NoDeepCatchup, RequestViewChange,
       NoRetire, UpdateActiveView, SyncCompatFields}
    [] candidate = QuorumTimeoutDeepFirstReenter ->
      {PreserveMode, PreserveGeneration, PreserveCandidate,
       ArmRebroadcastGuard, NoCommitPipeline, NoFetchBody, NoUrgentFetch,
       EnterDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = QuorumTimeoutDeepSecondViewChange ->
      {PreserveMode, SetOwnerExactSlotRepair, PreserveGeneration,
       PreserveCandidate, ClearRebroadcastGuard, NoCommitPipeline,
       NoFetchBody, NoUrgentFetch, NoDeepCatchup, RequestViewChange,
       NoRetire, UpdateActiveView, SyncCompatFields}
    [] candidate = QuorumTimeoutPassiveReasonOnly ->
      NoExternalActions \cup {PreserveMode, PreserveGeneration,
        PreserveCandidate, PreserveRebroadcastGuard, RecordLastReason,
        SyncCompatFields}
    [] candidate = LagWindowExpiredNormalExactDeep ->
      {SetModeDeepCatchup, SetOwnerExactSlotRepair, PreserveGeneration,
       PreserveCandidate, ClearRebroadcastGuard, RecordDeepReason,
       RecordLastReason, NoCommitPipeline, NoFetchBody, NoUrgentFetch,
       EnterDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = LagWindowExpiredPassiveReasonOnly ->
      NoExternalActions \cup {PreserveMode, PreserveGeneration,
        PreserveCandidate, PreserveRebroadcastGuard, RecordLastReason,
        SyncCompatFields}
    [] candidate = LagWindowExpiredDeepReenter ->
      {PreserveMode, PreserveGeneration, PreserveCandidate,
       PreserveRebroadcastGuard, NoCommitPipeline, NoFetchBody, NoUrgentFetch,
       EnterDeepCatchup, NoViewChange, NoRetire, SyncCompatFields}
    [] candidate = ViewAdvanceImmediate ->
      {PreserveMode, SetOwnerExactSlotRepair, PreserveGeneration,
       PreserveCandidate, ClearRebroadcastGuard, NoCommitPipeline,
       NoFetchBody, NoUrgentFetch, NoDeepCatchup, RequestViewChange,
       NoRetire, UpdateActiveView, SyncCompatFields}
    [] candidate = CommitHeightAtOrAboveFinalizes ->
      {SetModeFinalized, PreserveGeneration, PreserveCandidate,
       PreserveRebroadcastGuard, NoCommitPipeline, NoFetchBody, NoUrgentFetch,
       NoDeepCatchup, NoViewChange, RetireSlot, SyncCompatFields}
    [] candidate = CommitHeightBelowNoRetire ->
      NoExternalActions \cup {PreserveMode, PreserveGeneration,
        PreserveCandidate, PreserveRebroadcastGuard, SyncCompatFields}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = NewMissingInert /\ Bug = "new_missing_inert_arms_fetch" ->
      (spec \ {SetOwnerProposalLed, SetPhaseAwaitBlockCreated,
        ExactFetchUnarmed}) \cup {SetOwnerExactSlotRepair, SetPhaseAwaitBody,
        ExactFetchArmed}
    [] candidate = NewBodyPresentBlockCreated /\
          Bug = "new_body_present_wrong_phase" ->
      (spec \ {SetPhaseValidateBody}) \cup {SetPhaseAwaitBody}
    [] candidate = HigherBlockCreatedWithBody /\
          Bug = "higher_block_created_keeps_generation" ->
      (spec \ {IncrementGeneration, UnlockOwner}) \cup {PreserveGeneration}
    [] candidate = HigherBlockCreatedWithBody /\
          Bug = "higher_block_created_skips_pipeline" ->
      (spec \ {RequestCommitPipeline}) \cup {NoCommitPipeline}
    [] candidate = SameBlockCreatedDuplicateBodyPreservesRebroadcast /\
          Bug = "same_duplicate_clears_rebroadcast" ->
      (spec \ {PreserveRebroadcastGuard}) \cup {ClearRebroadcastGuard}
    [] candidate = MismatchedLowerBlockCreatedIgnored /\
          Bug = "mismatch_mutates_candidate" ->
      (spec \ {IgnoreMismatched, PreserveCandidate}) \cup {UpdateCandidate}
    [] candidate = BodyAvailableMissingRequestsCommit /\
          Bug = "body_available_skips_pipeline" ->
      (spec \ {RequestCommitPipeline}) \cup {NoCommitPipeline}
    [] candidate = BodyAvailableDuplicatePreservesRebroadcast /\
          Bug = "body_duplicate_clears_rebroadcast" ->
      (spec \ {PreserveRebroadcastGuard}) \cup {ClearRebroadcastGuard}
    [] candidate = BodyAvailablePassiveReturnsNormal /\
          Bug = "passive_body_keeps_passive" ->
      (spec \ {SetModeNormal}) \cup {PreserveMode}
    [] candidate = VoteObservedMissingUrgentFetch /\
          Bug = "vote_missing_waits_commit_qc" ->
      (spec \ {SetPhaseAwaitBody, FetchBody, FetchBodyUrgent,
        NoCommitPipeline}) \cup {SetPhaseAwaitCommitQc, NoFetchBody,
        NoUrgentFetch, RequestCommitPipeline}
    [] candidate = VoteObservedWithBodyCommitPipeline /\
          Bug = "vote_body_skips_pipeline" ->
      (spec \ {RequestCommitPipeline}) \cup {NoCommitPipeline}
    [] candidate = VoteObservedDifferentHigherRepairsMissing /\
          Bug = "vote_higher_keeps_stale_candidate" ->
      (spec \ {UpdateCandidate, IncrementGeneration, UnlockOwner}) \cup
        {PreserveCandidate, PreserveGeneration}
    [] candidate = VoteObservedDuplicatePreservesRebroadcast /\
          Bug = "vote_duplicate_clears_rebroadcast" ->
      (spec \ {PreserveRebroadcastGuard}) \cup {ClearRebroadcastGuard}
    [] candidate = CommitQcMissingUrgentFetch /\
          Bug = "commit_qc_missing_skips_fetch" ->
      (spec \ {FetchBody, FetchBodyUrgent}) \cup {NoFetchBody,
        NoUrgentFetch}
    [] candidate = CommitQcWithBodyCommitPipeline /\
          Bug = "commit_qc_body_skips_pipeline" ->
      (spec \ {RequestCommitPipeline}) \cup {NoCommitPipeline}
    [] candidate = CommitQcDuplicatePreservesRebroadcast /\
          Bug = "commit_qc_duplicate_clears_rebroadcast" ->
      (spec \ {PreserveRebroadcastGuard}) \cup {ClearRebroadcastGuard}
    [] candidate = AuthoritativeSupersedeMissingResetsNormal /\
          Bug = "authoritative_supersede_keeps_old_mode" ->
      (spec \ {SetModeNormal}) \cup {PreserveMode}
    [] candidate = AuthoritativeSupersedeBodyNoDirectPipeline /\
          Bug = "authoritative_supersede_requests_pipeline" ->
      (spec \ {NoCommitPipeline}) \cup {RequestCommitPipeline}
    [] candidate = FutureGapHigherExactFetchNormalFetch /\
          Bug = "future_gap_exact_skips_fetch" ->
      (spec \ {FetchBody}) \cup {NoFetchBody}
    [] candidate = FutureGapSameUnarmedNoFetch /\
          Bug = "future_gap_unarmed_fetches" ->
      (spec \ {NoFetchBody}) \cup {FetchBody}
    [] candidate = FetchRetryDueNormalExactMissingFetch /\
          Bug = "fetch_retry_normal_skips_fetch" ->
      (spec \ {FetchBody}) \cup {NoFetchBody}
    [] candidate = FetchRetryDueDeepNoFetch /\ Bug = "fetch_retry_deep_fetches" ->
      (spec \ {NoFetchBody}) \cup {FetchBody}
    [] candidate = QuorumTimeoutExactMissingFirstRebroadcast /\
          Bug = "quorum_timeout_first_rotates" ->
      (spec \ {FetchBody, FetchBodyUrgent, NoViewChange,
        ArmRebroadcastGuard}) \cup {NoFetchBody, NoUrgentFetch,
        RequestViewChange, ClearRebroadcastGuard}
    [] candidate = QuorumTimeoutExactMissingSecondViewChange /\
          Bug = "quorum_timeout_second_rebroadcasts" ->
      (spec \ {RequestViewChange, ClearRebroadcastGuard, NoFetchBody,
        NoUrgentFetch}) \cup {NoViewChange, ArmRebroadcastGuard,
        FetchBody, FetchBodyUrgent}
    [] candidate = QuorumTimeoutExactMissingSecondViewChange /\
          Bug = "quorum_timeout_second_wrong_view" ->
      spec \ {UpdateActiveView}
    [] candidate = QuorumTimeoutExactMissingLagExpiredDeep /\
          Bug = "quorum_timeout_lag_expired_skips_deep" ->
      (spec \ {SetModeDeepCatchup, EnterDeepCatchup, RecordDeepReason}) \cup
        {SetModeNormal, NoDeepCatchup}
    [] candidate = QuorumTimeoutBodyPresentFirstArmsOnly /\
          Bug = "body_present_timeout_enters_deep" ->
      (spec \ {NoDeepCatchup}) \cup {EnterDeepCatchup}
    [] candidate = QuorumTimeoutDeepSecondViewChange /\
          Bug = "deep_timeout_loops_without_rotation" ->
      (spec \ {RequestViewChange, ClearRebroadcastGuard, NoDeepCatchup}) \cup
        {NoViewChange, ArmRebroadcastGuard, EnterDeepCatchup}
    [] candidate = QuorumTimeoutPassiveReasonOnly /\
          Bug = "passive_timeout_requests_view_change" ->
      (spec \ {NoViewChange}) \cup {RequestViewChange}
    [] candidate = LagWindowExpiredNormalExactDeep /\
          Bug = "lag_expired_normal_fetches_instead" ->
      (spec \ {SetModeDeepCatchup, EnterDeepCatchup, NoFetchBody,
        RecordDeepReason}) \cup {SetModeNormal, NoDeepCatchup, FetchBody}
    [] candidate = LagWindowExpiredDeepReenter /\
          Bug = "lag_expired_deep_drops_reason" ->
      (spec \ {EnterDeepCatchup}) \cup {NoDeepCatchup}
    [] candidate = ViewAdvanceImmediate /\ Bug = "view_advance_keeps_guard" ->
      (spec \ {ClearRebroadcastGuard}) \cup {PreserveRebroadcastGuard}
    [] candidate = CommitHeightBelowNoRetire /\
          Bug = "commit_height_below_retires" ->
      (spec \ {NoRetire, PreserveMode}) \cup {RetireSlot, SetModeFinalized}
    [] candidate = CommitHeightAtOrAboveFinalizes /\
          Bug = "commit_height_at_or_above_not_finalized" ->
      (spec \ {RetireSlot, SetModeFinalized}) \cup {NoRetire, PreserveMode}
    [] candidate = HigherBlockCreatedMissingBody /\ Bug = "missing_sync_compat" ->
      spec \ {SyncCompatFields}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

Bugs == {
  "none",
  "new_missing_inert_arms_fetch",
  "new_body_present_wrong_phase",
  "higher_block_created_keeps_generation",
  "higher_block_created_skips_pipeline",
  "same_duplicate_clears_rebroadcast",
  "mismatch_mutates_candidate",
  "body_available_skips_pipeline",
  "body_duplicate_clears_rebroadcast",
  "passive_body_keeps_passive",
  "vote_missing_waits_commit_qc",
  "vote_body_skips_pipeline",
  "vote_higher_keeps_stale_candidate",
  "vote_duplicate_clears_rebroadcast",
  "commit_qc_missing_skips_fetch",
  "commit_qc_body_skips_pipeline",
  "commit_qc_duplicate_clears_rebroadcast",
  "authoritative_supersede_keeps_old_mode",
  "authoritative_supersede_requests_pipeline",
  "future_gap_exact_skips_fetch",
  "future_gap_unarmed_fetches",
  "fetch_retry_normal_skips_fetch",
  "fetch_retry_deep_fetches",
  "quorum_timeout_first_rotates",
  "quorum_timeout_second_rebroadcasts",
  "quorum_timeout_second_wrong_view",
  "quorum_timeout_lag_expired_skips_deep",
  "body_present_timeout_enters_deep",
  "deep_timeout_loops_without_rotation",
  "passive_timeout_requests_view_change",
  "lag_expired_normal_fetches_instead",
  "lag_expired_deep_drops_reason",
  "view_advance_keeps_guard",
  "commit_height_below_retires",
  "commit_height_at_or_above_not_finalized",
  "missing_sync_compat"
}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A candidate \in Candidates:
       ImplementationActions(candidate) \subseteq Actions

SlotTrackerStepMatchesSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ConstructorMatchesSpec ==
  \A candidate \in {
    NewMissingInert,
    NewMissingExactFetch,
    NewBodyPresentBlockCreated,
    NewBodyPresentExactRepair
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

BlockCreatedMatchesSpec ==
  \A candidate \in {
    HigherBlockCreatedWithBody,
    HigherBlockCreatedMissingBody,
    SameBlockCreatedDuplicateBodyPreservesRebroadcast,
    SameBlockCreatedFreshMissingStartsLag,
    MismatchedLowerBlockCreatedIgnored
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

BodyAvailableMatchesSpec ==
  \A candidate \in {
    BodyAvailableMissingRequestsCommit,
    BodyAvailableDuplicatePreservesRebroadcast,
    BodyAvailablePassiveReturnsNormal
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

VoteAndCommitQcMatchesSpec ==
  \A candidate \in {
    VoteObservedMissingUrgentFetch,
    VoteObservedWithBodyCommitPipeline,
    VoteObservedDifferentHigherRepairsMissing,
    VoteObservedDuplicatePreservesRebroadcast,
    CommitQcMissingUrgentFetch,
    CommitQcWithBodyCommitPipeline,
    CommitQcDuplicatePreservesRebroadcast
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

AuthoritativeAndFetchMatchesSpec ==
  \A candidate \in {
    AuthoritativeSupersedeMissingResetsNormal,
    AuthoritativeSupersedeBodyNoDirectPipeline,
    FutureGapHigherExactFetchNormalFetch,
    FutureGapSameUnarmedNoFetch,
    FetchRetryDueNormalExactMissingFetch,
    FetchRetryDueDeepNoFetch
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

QuorumTimeoutMatchesSpec ==
  \A candidate \in {
    QuorumTimeoutExactMissingFirstRebroadcast,
    QuorumTimeoutExactMissingSecondViewChange,
    QuorumTimeoutExactMissingLagExpiredDeep,
    QuorumTimeoutBodyPresentFirstArmsOnly,
    QuorumTimeoutBodyPresentSecondViewChange,
    QuorumTimeoutDeepFirstReenter,
    QuorumTimeoutDeepSecondViewChange,
    QuorumTimeoutPassiveReasonOnly
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

LagViewCommitMatchesSpec ==
  \A candidate \in {
    LagWindowExpiredNormalExactDeep,
    LagWindowExpiredPassiveReasonOnly,
    LagWindowExpiredDeepReenter,
    ViewAdvanceImmediate,
    CommitHeightAtOrAboveFinalizes,
    CommitHeightBelowNoRetire
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

UrgentFetchRequiresBodyFetch ==
  \A candidate \in Candidates:
    FetchBodyUrgent \in ImplementationActions(candidate) =>
      FetchBody \in ImplementationActions(candidate)

ViewChangeClearsRebroadcastGuard ==
  \A candidate \in Candidates:
    RequestViewChange \in ImplementationActions(candidate) =>
      ClearRebroadcastGuard \in ImplementationActions(candidate)

CommitPipelineRequiresBodyAvailable ==
  \A candidate \in Candidates:
    RequestCommitPipeline \in ImplementationActions(candidate) =>
      BodyAvailable \in ImplementationActions(candidate)

RetiredSlotsAreFinalized ==
  \A candidate \in Candidates:
    RetireSlot \in ImplementationActions(candidate) =>
      SetModeFinalized \in ImplementationActions(candidate)

EveryStepSyncsCompatFields ==
  \A candidate \in Candidates:
    SyncCompatFields \in ImplementationActions(candidate)

ConstructorAnchors ==
  /\ SetOwnerProposalLed \in SpecActions(NewMissingInert)
  /\ SetPhaseAwaitBlockCreated \in SpecActions(NewMissingInert)
  /\ ExactFetchUnarmed \in SpecActions(NewMissingInert)
  /\ SetOwnerExactSlotRepair \in SpecActions(NewMissingExactFetch)
  /\ SetPhaseAwaitBody \in SpecActions(NewMissingExactFetch)
  /\ TrackRequester \in SpecActions(NewMissingExactFetch)
  /\ SetOwnerBlockCreatedLed \in SpecActions(NewBodyPresentBlockCreated)
  /\ SetPhaseValidateBody \in SpecActions(NewBodyPresentBlockCreated)
  /\ RequestCommitPipeline \notin SpecActions(NewBodyPresentExactRepair)

BlockCreatedAnchors ==
  /\ IncrementGeneration \in SpecActions(HigherBlockCreatedWithBody)
  /\ UnlockOwner \in SpecActions(HigherBlockCreatedWithBody)
  /\ RequestCommitPipeline \in SpecActions(HigherBlockCreatedWithBody)
  /\ NoteLag \in SpecActions(HigherBlockCreatedMissingBody)
  /\ PreserveRebroadcastGuard
       \in SpecActions(SameBlockCreatedDuplicateBodyPreservesRebroadcast)
  /\ IgnoreMismatched \in SpecActions(MismatchedLowerBlockCreatedIgnored)
  /\ UpdateCandidate \notin SpecActions(MismatchedLowerBlockCreatedIgnored)

BodyEvidenceAnchors ==
  /\ RequestCommitPipeline \in SpecActions(BodyAvailableMissingRequestsCommit)
  /\ PreserveRebroadcastGuard
       \in SpecActions(BodyAvailableDuplicatePreservesRebroadcast)
  /\ SetModeNormal \in SpecActions(BodyAvailablePassiveReturnsNormal)

VoteAndCommitQcAnchors ==
  /\ FetchBodyUrgent \in SpecActions(VoteObservedMissingUrgentFetch)
  /\ FetchBody \in SpecActions(VoteObservedMissingUrgentFetch)
  /\ RequestCommitPipeline \in SpecActions(VoteObservedWithBodyCommitPipeline)
  /\ IncrementGeneration \in SpecActions(VoteObservedDifferentHigherRepairsMissing)
  /\ PreserveRebroadcastGuard
       \in SpecActions(VoteObservedDuplicatePreservesRebroadcast)
  /\ FetchBodyUrgent \in SpecActions(CommitQcMissingUrgentFetch)
  /\ RequestCommitPipeline \in SpecActions(CommitQcWithBodyCommitPipeline)
  /\ PreserveRebroadcastGuard
       \in SpecActions(CommitQcDuplicatePreservesRebroadcast)

AuthoritativeAndFetchAnchors ==
  /\ SetModeNormal \in SpecActions(AuthoritativeSupersedeMissingResetsNormal)
  /\ NoCommitPipeline \in SpecActions(AuthoritativeSupersedeBodyNoDirectPipeline)
  /\ FetchBody \in SpecActions(FutureGapHigherExactFetchNormalFetch)
  /\ NoFetchBody \in SpecActions(FutureGapSameUnarmedNoFetch)
  /\ FetchBody \in SpecActions(FetchRetryDueNormalExactMissingFetch)
  /\ NoFetchBody \in SpecActions(FetchRetryDueDeepNoFetch)

QuorumTimeoutAnchors ==
  /\ ArmRebroadcastGuard
       \in SpecActions(QuorumTimeoutExactMissingFirstRebroadcast)
  /\ RequestViewChange
       \in SpecActions(QuorumTimeoutExactMissingSecondViewChange)
  /\ UpdateActiveView
       \in SpecActions(QuorumTimeoutExactMissingSecondViewChange)
  /\ SetModeDeepCatchup
       \in SpecActions(QuorumTimeoutExactMissingLagExpiredDeep)
  /\ NoDeepCatchup \in SpecActions(QuorumTimeoutBodyPresentFirstArmsOnly)
  /\ RequestViewChange
       \in SpecActions(QuorumTimeoutBodyPresentSecondViewChange)
  /\ EnterDeepCatchup \in SpecActions(QuorumTimeoutDeepFirstReenter)
  /\ RequestViewChange \in SpecActions(QuorumTimeoutDeepSecondViewChange)
  /\ RecordLastReason \in SpecActions(QuorumTimeoutPassiveReasonOnly)

LagViewCommitAnchors ==
  /\ SetModeDeepCatchup \in SpecActions(LagWindowExpiredNormalExactDeep)
  /\ RecordLastReason \in SpecActions(LagWindowExpiredPassiveReasonOnly)
  /\ EnterDeepCatchup \in SpecActions(LagWindowExpiredDeepReenter)
  /\ RequestViewChange \in SpecActions(ViewAdvanceImmediate)
  /\ ClearRebroadcastGuard \in SpecActions(ViewAdvanceImmediate)
  /\ RetireSlot \in SpecActions(CommitHeightAtOrAboveFinalizes)
  /\ SetModeFinalized \in SpecActions(CommitHeightAtOrAboveFinalizes)
  /\ NoRetire \in SpecActions(CommitHeightBelowNoRetire)

Safety ==
  /\ SlotTrackerStepMatchesSpec
  /\ ConstructorMatchesSpec
  /\ BlockCreatedMatchesSpec
  /\ BodyAvailableMatchesSpec
  /\ VoteAndCommitQcMatchesSpec
  /\ AuthoritativeAndFetchMatchesSpec
  /\ QuorumTimeoutMatchesSpec
  /\ LagViewCommitMatchesSpec
  /\ UrgentFetchRequiresBodyFetch
  /\ ViewChangeClearsRebroadcastGuard
  /\ CommitPipelineRequiresBodyAvailable
  /\ RetiredSlotsAreFinalized
  /\ EveryStepSyncsCompatFields
  /\ ConstructorAnchors
  /\ BlockCreatedAnchors
  /\ BodyEvidenceAnchors
  /\ VoteAndCommitQcAnchors
  /\ AuthoritativeAndFetchAnchors
  /\ QuorumTimeoutAnchors
  /\ LagViewCommitAnchors

=============================================================================
====
