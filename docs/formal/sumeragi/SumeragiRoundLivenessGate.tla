---- MODULE SumeragiRoundLivenessGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `slot_has_round_liveness(height, view)`.

The helper joins four local evidence sources used by the pacemaker and idle
view-change logic: exact-slot proposal evidence, live frontier-owner work,
prior-view active pending ownership with a local commit vote or observed commit
QC, and local same-height vote history for the contiguous frontier. Earlier
source misses must fall through to later valid sources.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ProposalEvidence == "proposal_evidence"
ProposalWrongSlot == "proposal_wrong_slot"
ProposalWrongSlotWithFrontier == "proposal_wrong_slot_with_frontier"

FrontierOwnerLive == "frontier_owner_live"
FrontierOwnerWrongHeight == "frontier_owner_wrong_height"
FrontierOwnerDead == "frontier_owner_dead"
FrontierOwnerDeadWithPending == "frontier_owner_dead_with_pending"

PendingPriorCommitActive == "pending_prior_commit_active"
PendingPriorQcActive == "pending_prior_qc_active"
PendingSameViewCommit == "pending_same_view_commit"
PendingFutureViewCommit == "pending_future_view_commit"
PendingPriorNoEvidence == "pending_prior_no_evidence"
PendingPriorInactive == "pending_prior_inactive"
PendingInactiveWithLocalVote == "pending_inactive_with_local_vote"

LocalVoteExactContiguous == "local_vote_exact_contiguous"
LocalVoteLaterWins == "local_vote_later_wins"
LocalVoteExactNonContiguous == "local_vote_exact_non_contiguous"
LocalVoteWrongEpoch == "local_vote_wrong_epoch"
LocalVotePrevote == "local_vote_prevote"
RemoteVote == "remote_vote"
LocalVotePriorQueriedLater == "local_vote_prior_queried_later"
LocalVoteLaterMasksPriorQuery == "local_vote_later_masks_prior_query"

NoLiveness == "no_liveness"

Cases == {
  ProposalEvidence,
  ProposalWrongSlot,
  ProposalWrongSlotWithFrontier,
  FrontierOwnerLive,
  FrontierOwnerWrongHeight,
  FrontierOwnerDead,
  FrontierOwnerDeadWithPending,
  PendingPriorCommitActive,
  PendingPriorQcActive,
  PendingSameViewCommit,
  PendingFutureViewCommit,
  PendingPriorNoEvidence,
  PendingPriorInactive,
  PendingInactiveWithLocalVote,
  LocalVoteExactContiguous,
  LocalVoteLaterWins,
  LocalVoteExactNonContiguous,
  LocalVoteWrongEpoch,
  LocalVotePrevote,
  RemoteVote,
  LocalVotePriorQueriedLater,
  LocalVoteLaterMasksPriorQuery,
  NoLiveness
}

ProposalAcceptedCases == {ProposalEvidence}
ProposalWrongSlotCases == {ProposalWrongSlot, ProposalWrongSlotWithFrontier}

FrontierAcceptedCases == {FrontierOwnerLive, ProposalWrongSlotWithFrontier}
FrontierWrongHeightCases == {FrontierOwnerWrongHeight}
FrontierDeadCases == {FrontierOwnerDead, FrontierOwnerDeadWithPending}

PendingAcceptedCases == {
  PendingPriorCommitActive,
  PendingPriorQcActive,
  FrontierOwnerDeadWithPending
}
PendingSameViewCases == {PendingSameViewCommit}
PendingFutureViewCases == {PendingFutureViewCommit}
PendingNoEvidenceCases == {PendingPriorNoEvidence}
PendingInactiveCases == {PendingPriorInactive, PendingInactiveWithLocalVote}

LocalVoteAcceptedCases == {
  LocalVoteExactContiguous,
  LocalVoteLaterWins,
  PendingInactiveWithLocalVote
}
LocalVoteNonContiguousCases == {LocalVoteExactNonContiguous}
LocalVoteWrongEpochCases == {LocalVoteWrongEpoch}
LocalVoteWrongPhaseCases == {LocalVotePrevote}
LocalVoteRemoteCases == {RemoteVote}
LocalVoteWrongViewCases == {
  LocalVotePriorQueriedLater,
  LocalVoteLaterMasksPriorQuery
}

ProposalAccepted(c) == c \in ProposalAcceptedCases
FrontierAccepted(c) == c \in FrontierAcceptedCases
PendingAccepted(c) == c \in PendingAcceptedCases
LocalVoteAccepted(c) == c \in LocalVoteAcceptedCases

AfterProposal(c) == ~ProposalAccepted(c)
AfterFrontier(c) == AfterProposal(c) /\ ~FrontierAccepted(c)
AfterPending(c) == AfterFrontier(c) /\ ~PendingAccepted(c)

SpecResult(c) ==
  ProposalAccepted(c)
    \/ FrontierAccepted(c)
    \/ PendingAccepted(c)
    \/ LocalVoteAccepted(c)

ReturnTrue == 1
ReturnFalse == 2
CheckProposalEvidence == 3
CheckFrontierOwner == 4
CheckPendingOwner == 5
CheckLocalSameHeightVote == 6
ProposalEvidenceAccepted == 7
ProposalSlotMismatchIgnored == 8
FrontierOwnerAccepted == 9
FrontierOwnerHeightMismatchIgnored == 10
FrontierOwnerDeadIgnored == 11
PendingCommitVoteAccepted == 12
PendingCommitQcAccepted == 13
PendingSameViewRejected == 14
PendingFutureViewRejected == 15
PendingNoEvidenceIgnored == 16
PendingInactiveIgnored == 17
LocalVoteAcceptedAction == 18
LocalVoteNonContiguousRejected == 19
LocalVoteWrongEpochIgnored == 20
LocalVoteWrongPhaseIgnored == 21
LocalVoteRemoteIgnored == 22
LocalVoteWrongViewRejected == 23

ActionUniverse == 1..23

ProposalAction(c) ==
  CASE ProposalAccepted(c) -> {ProposalEvidenceAccepted}
    [] c \in ProposalWrongSlotCases -> {ProposalSlotMismatchIgnored}
    [] OTHER -> {}

FrontierAction(c) ==
  CASE FrontierAccepted(c) -> {FrontierOwnerAccepted}
    [] c \in FrontierWrongHeightCases -> {FrontierOwnerHeightMismatchIgnored}
    [] c \in FrontierDeadCases -> {FrontierOwnerDeadIgnored}
    [] OTHER -> {}

PendingAction(c) ==
  CASE c = PendingPriorCommitActive -> {PendingCommitVoteAccepted}
    [] c = PendingPriorQcActive -> {PendingCommitQcAccepted}
    [] c = FrontierOwnerDeadWithPending -> {PendingCommitVoteAccepted}
    [] c \in PendingSameViewCases -> {PendingSameViewRejected}
    [] c \in PendingFutureViewCases -> {PendingFutureViewRejected}
    [] c \in PendingNoEvidenceCases -> {PendingNoEvidenceIgnored}
    [] c \in PendingInactiveCases -> {PendingInactiveIgnored}
    [] OTHER -> {}

LocalVoteAction(c) ==
  CASE LocalVoteAccepted(c) -> {LocalVoteAcceptedAction}
    [] c \in LocalVoteNonContiguousCases -> {LocalVoteNonContiguousRejected}
    [] c \in LocalVoteWrongEpochCases -> {LocalVoteWrongEpochIgnored}
    [] c \in LocalVoteWrongPhaseCases -> {LocalVoteWrongPhaseIgnored}
    [] c \in LocalVoteRemoteCases -> {LocalVoteRemoteIgnored}
    [] c \in LocalVoteWrongViewCases -> {LocalVoteWrongViewRejected}
    [] OTHER -> {}

SpecActions(c) ==
  {CheckProposalEvidence}
    \cup (IF SpecResult(c) THEN {ReturnTrue} ELSE {ReturnFalse})
    \cup ProposalAction(c)
    \cup (IF AfterProposal(c) THEN {CheckFrontierOwner} ELSE {})
    \cup (IF AfterProposal(c) THEN FrontierAction(c) ELSE {})
    \cup (IF AfterFrontier(c) THEN {CheckPendingOwner} ELSE {})
    \cup (IF AfterFrontier(c) THEN PendingAction(c) ELSE {})
    \cup (IF AfterPending(c) THEN {CheckLocalSameHeightVote} ELSE {})
    \cup (IF AfterPending(c) THEN LocalVoteAction(c) ELSE {})

RejectAtCurrentStage(spec, acceptedAction) ==
  (spec \ {ReturnTrue, acceptedAction}) \cup {ReturnFalse}

AcceptAtProposal(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckFrontierOwner,
           CheckPendingOwner, CheckLocalSameHeightVote}) \cup
    {ReturnTrue, ProposalEvidenceAccepted}

AcceptAtFrontier(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckPendingOwner,
           CheckLocalSameHeightVote}) \cup {ReturnTrue, FrontierOwnerAccepted}

AcceptAtPending(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction, CheckLocalSameHeightVote}) \cup
    {ReturnTrue, PendingCommitVoteAccepted}

AcceptAtLocalVote(spec, ignoredAction) ==
  (spec \ {ReturnFalse, ignoredAction}) \cup
    {ReturnTrue, LocalVoteAcceptedAction}

BlockAfterProposal(spec) ==
  (spec \ {ReturnTrue, FrontierOwnerAccepted, CheckFrontierOwner,
           CheckPendingOwner, CheckLocalSameHeightVote}) \cup {ReturnFalse}

BlockAfterFrontier(spec) ==
  (spec \ {ReturnTrue, PendingCommitVoteAccepted, CheckPendingOwner,
           CheckLocalSameHeightVote}) \cup {ReturnFalse}

BlockAfterPending(spec) ==
  (spec \ {ReturnTrue, LocalVoteAcceptedAction, CheckLocalSameHeightVote}) \cup
    {ReturnFalse}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "reject_proposal_evidence"
       /\ c = ProposalEvidence ->
      RejectAtCurrentStage(spec, ProposalEvidenceAccepted)
    [] Bug = "accept_proposal_wrong_slot"
       /\ c = ProposalWrongSlot ->
      AcceptAtProposal(spec, ProposalSlotMismatchIgnored)
    [] Bug = "proposal_miss_blocks_frontier"
       /\ c = ProposalWrongSlotWithFrontier ->
      BlockAfterProposal(spec)
    [] Bug = "reject_frontier_owner"
       /\ c = FrontierOwnerLive ->
      RejectAtCurrentStage(spec, FrontierOwnerAccepted)
    [] Bug = "accept_frontier_wrong_height"
       /\ c = FrontierOwnerWrongHeight ->
      AcceptAtFrontier(spec, FrontierOwnerHeightMismatchIgnored)
    [] Bug = "accept_frontier_dead"
       /\ c = FrontierOwnerDead ->
      AcceptAtFrontier(spec, FrontierOwnerDeadIgnored)
    [] Bug = "frontier_dead_blocks_pending"
       /\ c = FrontierOwnerDeadWithPending ->
      BlockAfterFrontier(spec)
    [] Bug = "reject_pending_commit"
       /\ c = PendingPriorCommitActive ->
      RejectAtCurrentStage(spec, PendingCommitVoteAccepted)
    [] Bug = "reject_pending_qc"
       /\ c = PendingPriorQcActive ->
      RejectAtCurrentStage(spec, PendingCommitQcAccepted)
    [] Bug = "accept_pending_same_view"
       /\ c = PendingSameViewCommit ->
      AcceptAtPending(spec, PendingSameViewRejected)
    [] Bug = "accept_pending_future_view"
       /\ c = PendingFutureViewCommit ->
      AcceptAtPending(spec, PendingFutureViewRejected)
    [] Bug = "accept_pending_no_evidence"
       /\ c = PendingPriorNoEvidence ->
      AcceptAtPending(spec, PendingNoEvidenceIgnored)
    [] Bug = "accept_pending_inactive"
       /\ c = PendingPriorInactive ->
      AcceptAtPending(spec, PendingInactiveIgnored)
    [] Bug = "pending_inactive_blocks_local_vote"
       /\ c = PendingInactiveWithLocalVote ->
      BlockAfterPending(spec)
    [] Bug = "reject_local_vote"
       /\ c = LocalVoteExactContiguous ->
      RejectAtCurrentStage(spec, LocalVoteAcceptedAction)
    [] Bug = "reject_local_later_vote"
       /\ c = LocalVoteLaterWins ->
      RejectAtCurrentStage(spec, LocalVoteAcceptedAction)
    [] Bug = "accept_local_noncontiguous"
       /\ c = LocalVoteExactNonContiguous ->
      AcceptAtLocalVote(spec, LocalVoteNonContiguousRejected)
    [] Bug = "accept_local_wrong_epoch"
       /\ c = LocalVoteWrongEpoch ->
      AcceptAtLocalVote(spec, LocalVoteWrongEpochIgnored)
    [] Bug = "accept_local_prevote"
       /\ c = LocalVotePrevote ->
      AcceptAtLocalVote(spec, LocalVoteWrongPhaseIgnored)
    [] Bug = "accept_remote_vote"
       /\ c = RemoteVote ->
      AcceptAtLocalVote(spec, LocalVoteRemoteIgnored)
    [] Bug = "accept_local_prior_for_later_view"
       /\ c = LocalVotePriorQueriedLater ->
      AcceptAtLocalVote(spec, LocalVoteWrongViewRejected)
    [] Bug = "accept_local_prior_masked_by_later"
       /\ c = LocalVoteLaterMasksPriorQuery ->
      AcceptAtLocalVote(spec, LocalVoteWrongViewRejected)
    [] Bug = "accept_no_liveness"
       /\ c = NoLiveness ->
      (spec \ {ReturnFalse}) \cup {ReturnTrue, LocalVoteAcceptedAction}
    [] OTHER -> spec

ImplementationResult(c) == ReturnTrue \in ImplementationActions(c)

Bugs == {
  "none",
  "reject_proposal_evidence",
  "accept_proposal_wrong_slot",
  "proposal_miss_blocks_frontier",
  "reject_frontier_owner",
  "accept_frontier_wrong_height",
  "accept_frontier_dead",
  "frontier_dead_blocks_pending",
  "reject_pending_commit",
  "reject_pending_qc",
  "accept_pending_same_view",
  "accept_pending_future_view",
  "accept_pending_no_evidence",
  "accept_pending_inactive",
  "pending_inactive_blocks_local_vote",
  "reject_local_vote",
  "reject_local_later_vote",
  "accept_local_noncontiguous",
  "accept_local_wrong_epoch",
  "accept_local_prevote",
  "accept_remote_vote",
  "accept_local_prior_for_later_view",
  "accept_local_prior_masked_by_later",
  "accept_no_liveness"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultsMatchSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

AcceptedSourcesProduceLiveness ==
  /\ ImplementationResult(ProposalEvidence)
  /\ ImplementationResult(FrontierOwnerLive)
  /\ ImplementationResult(PendingPriorCommitActive)
  /\ ImplementationResult(PendingPriorQcActive)
  /\ ImplementationResult(LocalVoteExactContiguous)
  /\ ImplementationResult(LocalVoteLaterWins)

RejectedSourcesDoNotProduceLiveness ==
  /\ ~ImplementationResult(ProposalWrongSlot)
  /\ ~ImplementationResult(FrontierOwnerWrongHeight)
  /\ ~ImplementationResult(FrontierOwnerDead)
  /\ ~ImplementationResult(PendingSameViewCommit)
  /\ ~ImplementationResult(PendingFutureViewCommit)
  /\ ~ImplementationResult(PendingPriorNoEvidence)
  /\ ~ImplementationResult(PendingPriorInactive)
  /\ ~ImplementationResult(LocalVoteExactNonContiguous)
  /\ ~ImplementationResult(LocalVoteWrongEpoch)
  /\ ~ImplementationResult(LocalVotePrevote)
  /\ ~ImplementationResult(RemoteVote)
  /\ ~ImplementationResult(LocalVotePriorQueriedLater)
  /\ ~ImplementationResult(LocalVoteLaterMasksPriorQuery)
  /\ ~ImplementationResult(NoLiveness)

FallbackAfterEarlierMissesPreserved ==
  /\ ImplementationResult(ProposalWrongSlotWithFrontier)
  /\ ProposalSlotMismatchIgnored \in
       ImplementationActions(ProposalWrongSlotWithFrontier)
  /\ FrontierOwnerAccepted \in
       ImplementationActions(ProposalWrongSlotWithFrontier)
  /\ ImplementationResult(FrontierOwnerDeadWithPending)
  /\ FrontierOwnerDeadIgnored \in
       ImplementationActions(FrontierOwnerDeadWithPending)
  /\ PendingCommitVoteAccepted \in
       ImplementationActions(FrontierOwnerDeadWithPending)
  /\ ImplementationResult(PendingInactiveWithLocalVote)
  /\ PendingInactiveIgnored \in
       ImplementationActions(PendingInactiveWithLocalVote)
  /\ LocalVoteAcceptedAction \in
       ImplementationActions(PendingInactiveWithLocalVote)

LocalSameHeightVoteGate ==
  /\ ImplementationResult(LocalVoteExactContiguous)
  /\ ImplementationResult(LocalVoteLaterWins)
  /\ ~ImplementationResult(LocalVoteExactNonContiguous)
  /\ ~ImplementationResult(LocalVoteWrongEpoch)
  /\ ~ImplementationResult(LocalVotePrevote)
  /\ ~ImplementationResult(RemoteVote)
  /\ ~ImplementationResult(LocalVotePriorQueriedLater)
  /\ ~ImplementationResult(LocalVoteLaterMasksPriorQuery)

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckProposalEvidence \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterProposal(c) => CheckFrontierOwner \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterFrontier(c) => CheckPendingOwner \in ImplementationActions(c)
  /\ \A c \in Cases:
       AfterPending(c) =>
         CheckLocalSameHeightVote \in ImplementationActions(c)

ReturnActionMatchesResult ==
  \A c \in Cases:
    /\ (ReturnTrue \in ImplementationActions(c)) = ImplementationResult(c)
    /\ (ReturnFalse \in ImplementationActions(c)) = ~ImplementationResult(c)
    /\ ~(
         ReturnTrue \in ImplementationActions(c)
           /\ ReturnFalse \in ImplementationActions(c)
       )

AcceptedSourceActionAnchors ==
  /\ ProposalEvidenceAccepted \in ImplementationActions(ProposalEvidence)
  /\ FrontierOwnerAccepted \in ImplementationActions(FrontierOwnerLive)
  /\ PendingCommitVoteAccepted \in
       ImplementationActions(PendingPriorCommitActive)
  /\ PendingCommitQcAccepted \in ImplementationActions(PendingPriorQcActive)
  /\ LocalVoteAcceptedAction \in
       ImplementationActions(LocalVoteExactContiguous)
  /\ LocalVoteAcceptedAction \in ImplementationActions(LocalVoteLaterWins)

RejectedSourceActionAnchors ==
  /\ ProposalSlotMismatchIgnored \in ImplementationActions(ProposalWrongSlot)
  /\ FrontierOwnerHeightMismatchIgnored \in
       ImplementationActions(FrontierOwnerWrongHeight)
  /\ FrontierOwnerDeadIgnored \in ImplementationActions(FrontierOwnerDead)
  /\ PendingSameViewRejected \in ImplementationActions(PendingSameViewCommit)
  /\ PendingFutureViewRejected \in
       ImplementationActions(PendingFutureViewCommit)
  /\ PendingNoEvidenceIgnored \in
       ImplementationActions(PendingPriorNoEvidence)
  /\ PendingInactiveIgnored \in ImplementationActions(PendingPriorInactive)
  /\ LocalVoteNonContiguousRejected \in
       ImplementationActions(LocalVoteExactNonContiguous)
  /\ LocalVoteWrongEpochIgnored \in ImplementationActions(LocalVoteWrongEpoch)
  /\ LocalVoteWrongPhaseIgnored \in ImplementationActions(LocalVotePrevote)
  /\ LocalVoteRemoteIgnored \in ImplementationActions(RemoteVote)
  /\ LocalVoteWrongViewRejected \in
       ImplementationActions(LocalVotePriorQueriedLater)
  /\ LocalVoteWrongViewRejected \in
       ImplementationActions(LocalVoteLaterMasksPriorQuery)

RejectedSourceReturnAnchors ==
  /\ ReturnFalse \in ImplementationActions(ProposalWrongSlot)
  /\ ReturnFalse \in ImplementationActions(FrontierOwnerWrongHeight)
  /\ ReturnFalse \in ImplementationActions(FrontierOwnerDead)
  /\ ReturnFalse \in ImplementationActions(PendingSameViewCommit)
  /\ ReturnFalse \in ImplementationActions(PendingFutureViewCommit)
  /\ ReturnFalse \in ImplementationActions(PendingPriorNoEvidence)
  /\ ReturnFalse \in ImplementationActions(PendingPriorInactive)
  /\ ReturnFalse \in ImplementationActions(LocalVoteExactNonContiguous)
  /\ ReturnFalse \in ImplementationActions(LocalVoteWrongEpoch)
  /\ ReturnFalse \in ImplementationActions(LocalVotePrevote)
  /\ ReturnFalse \in ImplementationActions(RemoteVote)
  /\ ReturnFalse \in ImplementationActions(LocalVotePriorQueriedLater)
  /\ ReturnFalse \in ImplementationActions(LocalVoteLaterMasksPriorQuery)
  /\ ReturnFalse \in ImplementationActions(NoLiveness)

ShortCircuitAndFallbackAnchors ==
  /\ CheckFrontierOwner \notin ImplementationActions(ProposalEvidence)
  /\ CheckPendingOwner \notin ImplementationActions(FrontierOwnerLive)
  /\ CheckLocalSameHeightVote \notin
       ImplementationActions(PendingPriorCommitActive)
  /\ CheckLocalSameHeightVote \notin ImplementationActions(PendingPriorQcActive)
  /\ CheckFrontierOwner \in ImplementationActions(ProposalWrongSlotWithFrontier)
  /\ CheckPendingOwner \in ImplementationActions(FrontierOwnerDeadWithPending)
  /\ CheckLocalSameHeightVote \in
       ImplementationActions(PendingInactiveWithLocalVote)

RoundLivenessCoreSafety ==
  /\ ResultsMatchSpec
  /\ ActionsMatchSpec
  /\ AcceptedSourcesProduceLiveness
  /\ RejectedSourcesDoNotProduceLiveness
  /\ FallbackAfterEarlierMissesPreserved
  /\ LocalSameHeightVoteGate
  /\ LookupShapeMatchesShortCircuit
  /\ ReturnActionMatchesResult
  /\ AcceptedSourceActionAnchors
  /\ RejectedSourceActionAnchors
  /\ RejectedSourceReturnAnchors
  /\ ShortCircuitAndFallbackAnchors

NoBugInvariant == RoundLivenessCoreSafety

SafetyFast == RoundLivenessCoreSafety

RoundLivenessCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RoundLivenessCoreSafety

====
