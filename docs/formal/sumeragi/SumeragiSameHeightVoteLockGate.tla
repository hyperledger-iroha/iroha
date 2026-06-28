---- MODULE SumeragiSameHeightVoteLockGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the aggregate same-height vote-lock helper.

This slice pins `same_height_vote_lock_blocking_candidate(...)` and the
`frontier_slot_competing_quorum_locked_for_view(...)` direct/aggregate lock
composition. The model focuses on deterministic case coverage: roster
selection and fallback, vote filtering, signer and roster membership checks,
candidate-hash exclusion, conflicting-voter and branch-voter deduplication,
best-branch tie breaks, saturating remaining-vote arithmetic, metadata
population, and the frontier slot's requested-view and observed-vote guards.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

EmptyRosterNone == 1
FallbackRosterUsed == 2
CandidateSpecificRosterUsed == 3
VotePhaseFiltered == 4
VoteHeightFiltered == 5
VoteEpochFiltered == 6
MissingSignerIgnored == 7
NonRosterSignerIgnored == 8
CandidateHashIgnored == 9
ConflictingVotersDeduped == 10
BranchVotersDeduped == 11
BranchViewMax == 12
CommitVoteObserved == 13
CandidatePossibleVotesSaturating == 14
CandidateStillViableNone == 15
CandidateNotViableReturnsLock == 16
RequiredFlooredToOne == 17
BestBranchByVoteCount == 18
BestBranchCommitTie == 19
BestBranchViewTie == 20
LockMetadataFields == 21
FrontierRequestedViewGate == 22
FrontierDirectSlotLock == 23
FrontierAggregateLock == 24
FrontierRequiresObservedVote == 25

Candidates == 1..25

UseLiveRoster == 1
UseCandidateRoster == 2
FallbackEffectiveTopology == 3
ReturnNoneEmptyRoster == 4
FilterPrepareCommit == 5
FilterHeight == 6
FilterEpoch == 7
ResolveSigner == 8
FilterRosterSigner == 9
ExcludeCandidateHash == 10
DedupConflictingVoters == 11
DedupBranchVoters == 12
TrackMaxBranchView == 13
TrackCommitObserved == 14
ComputeRequired == 15
FloorRequiredOne == 16
ComputeTotalValidators == 17
SaturatingRemainingVotes == 18
ReturnNoneIfCandidateViable == 19
ReturnLockIfCandidateNonviable == 20
SelectByVoteCount == 21
TieBreakCommitObserved == 22
TieBreakView == 23
FillLockMetadata == 24
RejectRequestedViewNotNewer == 25
DirectSlotVoteObserved == 26
DirectSlotLock == 27
AggregateSameHeightLock == 28
RequireExistingSlotVote == 29
ReturnFrontierLock == 30
UnderflowingRemainingVotes == 31
CountRejectedVote == 32
CountCandidateHash == 33
SelectLowerVoteCount == 34
SelectLowerBranchView == 35
DropLockMetadata == 36
DirectLeqLock == 37
UseLiveRosterForCandidateHash == 38
SkipFallback == 39

Actions == 1..39

CandidateMathBase ==
  {ComputeRequired, ComputeTotalValidators, DedupConflictingVoters,
   SaturatingRemainingVotes}

LockReturnBase ==
  CandidateMathBase \cup {ReturnLockIfCandidateNonviable, FillLockMetadata}

SpecActions(candidate) ==
  CASE candidate = EmptyRosterNone ->
      {ReturnNoneEmptyRoster}
    [] candidate = FallbackRosterUsed ->
      {UseLiveRoster, FallbackEffectiveTopology} \cup LockReturnBase
    [] candidate = CandidateSpecificRosterUsed ->
      {UseCandidateRoster, FilterRosterSigner} \cup LockReturnBase
    [] candidate = VotePhaseFiltered ->
      {FilterPrepareCommit, ReturnNoneIfCandidateViable}
    [] candidate = VoteHeightFiltered ->
      {FilterHeight, ReturnNoneIfCandidateViable}
    [] candidate = VoteEpochFiltered ->
      {FilterEpoch, ReturnNoneIfCandidateViable}
    [] candidate = MissingSignerIgnored ->
      {ResolveSigner, ReturnNoneIfCandidateViable}
    [] candidate = NonRosterSignerIgnored ->
      {FilterRosterSigner, ReturnNoneIfCandidateViable}
    [] candidate = CandidateHashIgnored ->
      {ExcludeCandidateHash, ReturnNoneIfCandidateViable}
    [] candidate = ConflictingVotersDeduped ->
      CandidateMathBase \cup {ReturnNoneIfCandidateViable}
    [] candidate = BranchVotersDeduped ->
      {DedupBranchVoters, SelectByVoteCount, FillLockMetadata}
    [] candidate = BranchViewMax ->
      {TrackMaxBranchView, TieBreakView, FillLockMetadata}
    [] candidate = CommitVoteObserved ->
      {TrackCommitObserved, TieBreakCommitObserved, FillLockMetadata}
    [] candidate = CandidatePossibleVotesSaturating ->
      CandidateMathBase
    [] candidate = CandidateStillViableNone ->
      CandidateMathBase \cup {ReturnNoneIfCandidateViable}
    [] candidate = CandidateNotViableReturnsLock ->
      LockReturnBase
    [] candidate = RequiredFlooredToOne ->
      {ComputeRequired, FloorRequiredOne}
    [] candidate = BestBranchByVoteCount ->
      {DedupBranchVoters, SelectByVoteCount}
    [] candidate = BestBranchCommitTie ->
      {SelectByVoteCount, TieBreakCommitObserved}
    [] candidate = BestBranchViewTie ->
      {SelectByVoteCount, TieBreakCommitObserved, TieBreakView}
    [] candidate = LockMetadataFields ->
      {ComputeRequired, ComputeTotalValidators, SaturatingRemainingVotes,
       FillLockMetadata}
    [] candidate = FrontierRequestedViewGate ->
      {RejectRequestedViewNotNewer}
    [] candidate = FrontierDirectSlotLock ->
      {DirectSlotVoteObserved, DirectSlotLock, ReturnFrontierLock}
    [] candidate = FrontierAggregateLock ->
      {DirectSlotVoteObserved, AggregateSameHeightLock, ReturnFrontierLock}
    [] candidate = FrontierRequiresObservedVote ->
      {RequireExistingSlotVote}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = EmptyRosterNone /\
          Bug = "empty_roster_returns_lock" ->
      (spec \ {ReturnNoneEmptyRoster}) \cup {ReturnLockIfCandidateNonviable}
    [] candidate = FallbackRosterUsed /\
          Bug = "fallback_roster_skipped" ->
      (spec \ {FallbackEffectiveTopology, ReturnLockIfCandidateNonviable})
        \cup {SkipFallback, ReturnNoneEmptyRoster}
    [] candidate = CandidateSpecificRosterUsed /\
          Bug = "candidate_hash_uses_live_roster" ->
      (spec \ {UseCandidateRoster}) \cup {UseLiveRosterForCandidateHash}
    [] candidate = VotePhaseFiltered /\
          Bug = "non_vote_phase_counted" ->
      (spec \ {FilterPrepareCommit, ReturnNoneIfCandidateViable}) \cup
        {CountRejectedVote, ReturnLockIfCandidateNonviable}
    [] candidate = VoteHeightFiltered /\
          Bug = "wrong_height_counted" ->
      (spec \ {FilterHeight, ReturnNoneIfCandidateViable}) \cup
        {CountRejectedVote, ReturnLockIfCandidateNonviable}
    [] candidate = VoteEpochFiltered /\
          Bug = "wrong_epoch_counted" ->
      (spec \ {FilterEpoch, ReturnNoneIfCandidateViable}) \cup
        {CountRejectedVote, ReturnLockIfCandidateNonviable}
    [] candidate = MissingSignerIgnored /\
          Bug = "missing_signer_counted" ->
      (spec \ {ResolveSigner, ReturnNoneIfCandidateViable}) \cup
        {CountRejectedVote, ReturnLockIfCandidateNonviable}
    [] candidate = NonRosterSignerIgnored /\
          Bug = "non_roster_signer_counted" ->
      (spec \ {FilterRosterSigner, ReturnNoneIfCandidateViable}) \cup
        {CountRejectedVote, ReturnLockIfCandidateNonviable}
    [] candidate = CandidateHashIgnored /\
          Bug = "candidate_hash_counted" ->
      (spec \ {ExcludeCandidateHash, ReturnNoneIfCandidateViable}) \cup
        {CountCandidateHash, ReturnLockIfCandidateNonviable}
    [] candidate = ConflictingVotersDeduped /\
          Bug = "conflicting_voters_not_deduped" ->
      (spec \ {DedupConflictingVoters}) \cup {CountRejectedVote}
    [] candidate = BranchVotersDeduped /\
          Bug = "branch_voters_not_deduped" ->
      spec \ {DedupBranchVoters}
    [] candidate = BranchViewMax /\
          Bug = "branch_view_uses_min" ->
      (spec \ {TrackMaxBranchView}) \cup {SelectLowerBranchView}
    [] candidate = CommitVoteObserved /\
          Bug = "commit_vote_not_recorded" ->
      spec \ {TrackCommitObserved}
    [] candidate = CandidatePossibleVotesSaturating /\
          Bug = "remaining_votes_underflows" ->
      (spec \ {SaturatingRemainingVotes}) \cup {UnderflowingRemainingVotes}
    [] candidate = CandidateStillViableNone /\
          Bug = "viable_candidate_returns_lock" ->
      (spec \ {ReturnNoneIfCandidateViable}) \cup
        {ReturnLockIfCandidateNonviable, FillLockMetadata}
    [] candidate = CandidateNotViableReturnsLock /\
          Bug = "nonviable_candidate_returns_none" ->
      (spec \ {ReturnLockIfCandidateNonviable, FillLockMetadata}) \cup
        {ReturnNoneIfCandidateViable}
    [] candidate = RequiredFlooredToOne /\
          Bug = "required_not_floored" ->
      spec \ {FloorRequiredOne}
    [] candidate = BestBranchByVoteCount /\
          Bug = "selects_lower_vote_count" ->
      (spec \ {SelectByVoteCount}) \cup {SelectLowerVoteCount}
    [] candidate = BestBranchCommitTie /\
          Bug = "commit_tie_ignored" ->
      spec \ {TieBreakCommitObserved}
    [] candidate = BestBranchViewTie /\
          Bug = "view_tie_ignored" ->
      (spec \ {TieBreakView}) \cup {SelectLowerBranchView}
    [] candidate = LockMetadataFields /\
          Bug = "lock_metadata_dropped" ->
      (spec \ {FillLockMetadata}) \cup {DropLockMetadata}
    [] candidate = FrontierRequestedViewGate /\
          Bug = "frontier_allows_same_view" ->
      (spec \ {RejectRequestedViewNotNewer}) \cup {ReturnFrontierLock}
    [] candidate = FrontierDirectSlotLock /\
          Bug = "frontier_direct_uses_leq" ->
      (spec \ {DirectSlotLock}) \cup {DirectLeqLock}
    [] candidate = FrontierRequiresObservedVote /\
          Bug = "frontier_aggregate_without_observed_vote" ->
      (spec \ {RequireExistingSlotVote}) \cup
        {AggregateSameHeightLock, ReturnFrontierLock}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_roster_returns_lock",
       "fallback_roster_skipped",
       "candidate_hash_uses_live_roster",
       "non_vote_phase_counted",
       "wrong_height_counted",
       "wrong_epoch_counted",
       "missing_signer_counted",
       "non_roster_signer_counted",
       "candidate_hash_counted",
       "conflicting_voters_not_deduped",
       "branch_voters_not_deduped",
       "branch_view_uses_min",
       "commit_vote_not_recorded",
       "remaining_votes_underflows",
       "viable_candidate_returns_lock",
       "nonviable_candidate_returns_none",
       "required_not_floored",
       "selects_lower_vote_count",
       "commit_tie_ignored",
       "view_tie_ignored",
       "lock_metadata_dropped",
       "frontier_allows_same_view",
       "frontier_direct_uses_leq",
       "frontier_aggregate_without_observed_vote"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

SameHeightVoteLockActionsMatchSpec ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

SameHeightVoteLockExactness ==
  /\ SameHeightVoteLockActionsMatchSpec

SameHeightVoteLockCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SameHeightVoteLockExactness

Safety ==
  SameHeightVoteLockExactness

BugEmptyRosterReturnsLock ==
  ImplementationActions(EmptyRosterNone) = SpecActions(EmptyRosterNone)

BugFallbackRosterSkipped ==
  ImplementationActions(FallbackRosterUsed) = SpecActions(FallbackRosterUsed)

BugCandidateHashUsesLiveRoster ==
  ImplementationActions(CandidateSpecificRosterUsed) =
    SpecActions(CandidateSpecificRosterUsed)

BugNonVotePhaseCounted ==
  ImplementationActions(VotePhaseFiltered) = SpecActions(VotePhaseFiltered)

BugWrongHeightCounted ==
  ImplementationActions(VoteHeightFiltered) = SpecActions(VoteHeightFiltered)

BugWrongEpochCounted ==
  ImplementationActions(VoteEpochFiltered) = SpecActions(VoteEpochFiltered)

BugMissingSignerCounted ==
  ImplementationActions(MissingSignerIgnored) = SpecActions(MissingSignerIgnored)

BugNonRosterSignerCounted ==
  ImplementationActions(NonRosterSignerIgnored) =
    SpecActions(NonRosterSignerIgnored)

BugCandidateHashCounted ==
  ImplementationActions(CandidateHashIgnored) = SpecActions(CandidateHashIgnored)

BugConflictingVotersNotDeduped ==
  ImplementationActions(ConflictingVotersDeduped) =
    SpecActions(ConflictingVotersDeduped)

BugBranchVotersNotDeduped ==
  ImplementationActions(BranchVotersDeduped) =
    SpecActions(BranchVotersDeduped)

BugBranchViewUsesMin ==
  ImplementationActions(BranchViewMax) = SpecActions(BranchViewMax)

BugCommitVoteNotRecorded ==
  ImplementationActions(CommitVoteObserved) = SpecActions(CommitVoteObserved)

BugRemainingVotesUnderflows ==
  ImplementationActions(CandidatePossibleVotesSaturating) =
    SpecActions(CandidatePossibleVotesSaturating)

BugViableCandidateReturnsLock ==
  ImplementationActions(CandidateStillViableNone) =
    SpecActions(CandidateStillViableNone)

BugNonviableCandidateReturnsNone ==
  ImplementationActions(CandidateNotViableReturnsLock) =
    SpecActions(CandidateNotViableReturnsLock)

BugRequiredNotFloored ==
  ImplementationActions(RequiredFlooredToOne) =
    SpecActions(RequiredFlooredToOne)

BugSelectsLowerVoteCount ==
  ImplementationActions(BestBranchByVoteCount) =
    SpecActions(BestBranchByVoteCount)

BugCommitTieIgnored ==
  ImplementationActions(BestBranchCommitTie) = SpecActions(BestBranchCommitTie)

BugViewTieIgnored ==
  ImplementationActions(BestBranchViewTie) = SpecActions(BestBranchViewTie)

BugLockMetadataDropped ==
  ImplementationActions(LockMetadataFields) = SpecActions(LockMetadataFields)

BugFrontierAllowsSameView ==
  ImplementationActions(FrontierRequestedViewGate) =
    SpecActions(FrontierRequestedViewGate)

BugFrontierDirectUsesLeq ==
  ImplementationActions(FrontierDirectSlotLock) =
    SpecActions(FrontierDirectSlotLock)

BugFrontierAggregateWithoutObservedVote ==
  ImplementationActions(FrontierRequiresObservedVote) =
    SpecActions(FrontierRequiresObservedVote)

=============================================================================
====
