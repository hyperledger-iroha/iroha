---- MODULE SumeragiFrontierGapRealignGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for Sumeragi frontier-gap realignment and committed
anchor range-pull pacing.

This slice models the recovery corridor around
`maybe_request_frontier_gap_realign_after_commit(...)` and
`request_range_pull_from_anchor_with_tier(...)`. It abstracts concrete hashes,
peers, windows, and network messages into representative boundary cases while
preserving the key safety obligations: post-commit reanchor requests require
future evidence strictly beyond the contiguous frontier and no local
tip-extending frontier payload, exact-body frontier repair suppresses generic
range pulls unless deep catch-up is allowed, canonical frontier realignment
uses the previous/latest committed anchor pair when available, targets fall
back deterministically from voting roster to commit topology to trusted peers,
local/duplicate targets are removed, shared-window and stride gates suppress
duplicate reanchors, successful sends record permits/cooldowns/window marks,
and emitted canonical frontier pulls use the high-priority recovery lane.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

NoFutureEvidenceNoRequest == 1
FutureEvidenceAtFrontierNoRequest == 2
FutureEvidenceBeyondMissingPayloadRequests == 3
LocalTipPayloadSuppresses == 4
ExactBodyOwnerSuppressesGenericPull == 5
ExactBodyLagExpiredRetriesExactRepair == 6
DeepCatchupBypassesExactOwnerSuppress == 7
CanonicalReanchorUsesPrevLatestAnchor == 8
NonCanonicalUsesLatestLatestAnchor == 9
MissingAnchorSuppresses == 10
VoteRosterTargetsPreferred == 11
CommitTopologyFallbackTargets == 12
TrustedPeersFallbackTargets == 13
LocalPeerRemovedFromTargets == 14
TargetsSortedDeduped == 15
EmptyTargetsSuppress == 16
PerPeerCooldownSkipsDuplicate == 17
SentZeroSuppress == 18
SuccessfulPullRecordsPermits == 19
SuccessfulPullMarksCanonicalWindow == 20
AlreadyEmittedWindowSuppresses == 21
CanonicalStrideSuppressesNonAligned == 22
CanonicalStrideAlignedEmits == 23
EveryThirdWindowAllPeers == 24
OtherWindowTwoPeerCohort == 25
RecoveryFsmSuppressesWindow == 26
MissingQcStallMarksWindow == 27
HighPriorityForCanonicalNextHeight == 28
LockLagFarFutureExtendsCooldown == 29
RangePullMetricIncrement == 30
CanonicalWindowRecordsDependencyWatermark == 31

Candidates == 1..31

NoBug == 0
NoFutureEvidenceRequestsBug == 1
AcceptSameHeightFutureBug == 2
SkipFutureEvidenceRequestBug == 3
IgnoreLocalTipPayloadBug == 4
BypassExactOwnerBug == 5
SkipExactRetryBug == 6
DeepCatchupStillSuppressedBug == 7
CanonicalUsesLatestAnchorBug == 8
NonCanonicalUsesPrevAnchorBug == 9
MissingAnchorRequestsBug == 10
SkipVoteRosterBug == 11
NoCommitTopologyFallbackBug == 12
NoTrustedFallbackBug == 13
SendToLocalPeerBug == 14
UnstableTargetOrderBug == 15
EmptyTargetsRequestBug == 16
IgnoreCooldownBug == 17
ZeroSentReturnsSuccessBug == 18
SkipPermitBug == 19
SkipCanonicalWindowMarkBug == 20
RepeatAlreadyEmittedWindowBug == 21
IgnoreStrideBug == 22
DropAlignedStrideBug == 23
AllPeerCadenceSkippedBug == 24
CohortUsesAllPeersBug == 25
IgnoreRecoveryFsmBug == 26
SkipMissingQcWindowMarkBug == 27
LowPriorityCanonicalBug == 28
LockLagCooldownNotExtendedBug == 29
MetricNotIncrementedBug == 30
DropDependencyWatermarkBug == 31

Bugs == 0..31

NoRequest == 1
RequestPull == 2
CheckFutureEvidence == 3
CheckFrontierStrictlyAhead == 4
CheckLocalPayload == 5
SuppressForLocalPayload == 6
SuppressForExactOwner == 7
RetryExactRepair == 8
TriggerLagEvent == 9
AllowDeepCatchup == 10
UsePrevLatestAnchor == 11
UseLatestLatestAnchor == 12
RequireAnchor == 13
SelectVoteRoster == 14
SelectCommitTopology == 15
SelectTrustedPeers == 16
RemoveLocalPeer == 17
SortTargets == 18
DedupTargets == 19
SuppressEmptyTargets == 20
CheckCooldown == 21
RecordCooldown == 22
RecordDirectPermit == 23
PostGetBlocksAfter == 24
MarkCanonicalWindow == 25
SuppressAlreadyEmittedWindow == 26
ApplyStride == 27
SuppressStrideMismatch == 28
UseTwoPeerCohort == 29
UseAllPeers == 30
RespectRecoveryFsm == 31
MarkMissingQcWindow == 32
UseHighPriority == 33
ExtendCooldown == 34
IncrementMetric == 35
RecordDependencyWatermark == 36

Actions == 1..36

SpecActions(candidate) ==
  CASE candidate = NoFutureEvidenceNoRequest ->
      {NoRequest, CheckFutureEvidence}
    [] candidate = FutureEvidenceAtFrontierNoRequest ->
      {NoRequest, CheckFutureEvidence, CheckFrontierStrictlyAhead}
    [] candidate = FutureEvidenceBeyondMissingPayloadRequests ->
      {RequestPull, CheckFutureEvidence, CheckFrontierStrictlyAhead,
       CheckLocalPayload}
    [] candidate = LocalTipPayloadSuppresses ->
      {NoRequest, CheckLocalPayload, SuppressForLocalPayload}
    [] candidate = ExactBodyOwnerSuppressesGenericPull ->
      {NoRequest, SuppressForExactOwner, RetryExactRepair}
    [] candidate = ExactBodyLagExpiredRetriesExactRepair ->
      {NoRequest, SuppressForExactOwner, TriggerLagEvent, RetryExactRepair}
    [] candidate = DeepCatchupBypassesExactOwnerSuppress ->
      {RequestPull, AllowDeepCatchup}
    [] candidate = CanonicalReanchorUsesPrevLatestAnchor ->
      {RequestPull, RequireAnchor, UsePrevLatestAnchor}
    [] candidate = NonCanonicalUsesLatestLatestAnchor ->
      {RequestPull, RequireAnchor, UseLatestLatestAnchor}
    [] candidate = MissingAnchorSuppresses -> {NoRequest, RequireAnchor}
    [] candidate = VoteRosterTargetsPreferred ->
      {RequestPull, SelectVoteRoster}
    [] candidate = CommitTopologyFallbackTargets ->
      {RequestPull, SelectVoteRoster, SelectCommitTopology}
    [] candidate = TrustedPeersFallbackTargets ->
      {RequestPull, SelectVoteRoster, SelectCommitTopology, SelectTrustedPeers}
    [] candidate = LocalPeerRemovedFromTargets -> {RemoveLocalPeer}
    [] candidate = TargetsSortedDeduped -> {SortTargets, DedupTargets}
    [] candidate = EmptyTargetsSuppress ->
      {NoRequest, SuppressEmptyTargets}
    [] candidate = PerPeerCooldownSkipsDuplicate ->
      {NoRequest, CheckCooldown}
    [] candidate = SentZeroSuppress ->
      {NoRequest, CheckCooldown, SuppressEmptyTargets}
    [] candidate = SuccessfulPullRecordsPermits ->
      {RequestPull, RecordCooldown, RecordDirectPermit, PostGetBlocksAfter}
    [] candidate = SuccessfulPullMarksCanonicalWindow ->
      {RequestPull, MarkCanonicalWindow}
    [] candidate = AlreadyEmittedWindowSuppresses ->
      {NoRequest, SuppressAlreadyEmittedWindow}
    [] candidate = CanonicalStrideSuppressesNonAligned ->
      {NoRequest, ApplyStride, SuppressStrideMismatch}
    [] candidate = CanonicalStrideAlignedEmits ->
      {RequestPull, ApplyStride, MarkCanonicalWindow}
    [] candidate = EveryThirdWindowAllPeers -> {RequestPull, UseAllPeers}
    [] candidate = OtherWindowTwoPeerCohort -> {RequestPull, UseTwoPeerCohort}
    [] candidate = RecoveryFsmSuppressesWindow ->
      {NoRequest, RespectRecoveryFsm}
    [] candidate = MissingQcStallMarksWindow ->
      {RequestPull, MarkMissingQcWindow}
    [] candidate = HighPriorityForCanonicalNextHeight ->
      {RequestPull, UseHighPriority}
    [] candidate = LockLagFarFutureExtendsCooldown ->
      {RequestPull, ExtendCooldown}
    [] candidate = RangePullMetricIncrement ->
      {RequestPull, IncrementMetric}
    [] candidate = CanonicalWindowRecordsDependencyWatermark ->
      {RequestPull, MarkCanonicalWindow, RecordDependencyWatermark}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = NoFutureEvidenceNoRequest /\
          Bug = NoFutureEvidenceRequestsBug ->
      (spec \ {NoRequest}) \cup {RequestPull}
    [] candidate = FutureEvidenceAtFrontierNoRequest /\
          Bug = AcceptSameHeightFutureBug ->
      (spec \ {NoRequest}) \cup {RequestPull}
    [] candidate = FutureEvidenceBeyondMissingPayloadRequests /\
          Bug = SkipFutureEvidenceRequestBug ->
      (spec \ {RequestPull}) \cup {NoRequest}
    [] candidate = LocalTipPayloadSuppresses /\ Bug = IgnoreLocalTipPayloadBug ->
      (spec \ {NoRequest, SuppressForLocalPayload}) \cup {RequestPull}
    [] candidate = ExactBodyOwnerSuppressesGenericPull /\
          Bug = BypassExactOwnerBug ->
      (spec \ {NoRequest, SuppressForExactOwner}) \cup {RequestPull}
    [] candidate = ExactBodyLagExpiredRetriesExactRepair /\
          Bug = SkipExactRetryBug ->
      spec \ {RetryExactRepair, TriggerLagEvent}
    [] candidate = DeepCatchupBypassesExactOwnerSuppress /\
          Bug = DeepCatchupStillSuppressedBug ->
      (spec \ {RequestPull, AllowDeepCatchup}) \cup {NoRequest}
    [] candidate = CanonicalReanchorUsesPrevLatestAnchor /\
          Bug = CanonicalUsesLatestAnchorBug ->
      (spec \ {UsePrevLatestAnchor}) \cup {UseLatestLatestAnchor}
    [] candidate = NonCanonicalUsesLatestLatestAnchor /\
          Bug = NonCanonicalUsesPrevAnchorBug ->
      (spec \ {UseLatestLatestAnchor}) \cup {UsePrevLatestAnchor}
    [] candidate = MissingAnchorSuppresses /\ Bug = MissingAnchorRequestsBug ->
      (spec \ {NoRequest}) \cup {RequestPull}
    [] candidate = VoteRosterTargetsPreferred /\ Bug = SkipVoteRosterBug ->
      (spec \ {SelectVoteRoster}) \cup {SelectCommitTopology}
    [] candidate = CommitTopologyFallbackTargets /\
          Bug = NoCommitTopologyFallbackBug ->
      spec \ {SelectCommitTopology}
    [] candidate = TrustedPeersFallbackTargets /\ Bug = NoTrustedFallbackBug ->
      spec \ {SelectTrustedPeers}
    [] candidate = LocalPeerRemovedFromTargets /\ Bug = SendToLocalPeerBug ->
      spec \ {RemoveLocalPeer}
    [] candidate = TargetsSortedDeduped /\ Bug = UnstableTargetOrderBug ->
      spec \ {SortTargets, DedupTargets}
    [] candidate = EmptyTargetsSuppress /\ Bug = EmptyTargetsRequestBug ->
      (spec \ {NoRequest, SuppressEmptyTargets}) \cup {RequestPull}
    [] candidate = PerPeerCooldownSkipsDuplicate /\ Bug = IgnoreCooldownBug ->
      (spec \ {NoRequest, CheckCooldown}) \cup {RequestPull}
    [] candidate = SentZeroSuppress /\ Bug = ZeroSentReturnsSuccessBug ->
      (spec \ {NoRequest}) \cup {RequestPull}
    [] candidate = SuccessfulPullRecordsPermits /\ Bug = SkipPermitBug ->
      spec \ {RecordDirectPermit}
    [] candidate = SuccessfulPullMarksCanonicalWindow /\
          Bug = SkipCanonicalWindowMarkBug ->
      spec \ {MarkCanonicalWindow}
    [] candidate = AlreadyEmittedWindowSuppresses /\
          Bug = RepeatAlreadyEmittedWindowBug ->
      (spec \ {NoRequest, SuppressAlreadyEmittedWindow}) \cup {RequestPull}
    [] candidate = CanonicalStrideSuppressesNonAligned /\ Bug = IgnoreStrideBug ->
      (spec \ {NoRequest, SuppressStrideMismatch}) \cup {RequestPull}
    [] candidate = CanonicalStrideAlignedEmits /\ Bug = DropAlignedStrideBug ->
      (spec \ {RequestPull}) \cup {NoRequest}
    [] candidate = EveryThirdWindowAllPeers /\ Bug = AllPeerCadenceSkippedBug ->
      (spec \ {UseAllPeers}) \cup {UseTwoPeerCohort}
    [] candidate = OtherWindowTwoPeerCohort /\ Bug = CohortUsesAllPeersBug ->
      (spec \ {UseTwoPeerCohort}) \cup {UseAllPeers}
    [] candidate = RecoveryFsmSuppressesWindow /\ Bug = IgnoreRecoveryFsmBug ->
      (spec \ {NoRequest, RespectRecoveryFsm}) \cup {RequestPull}
    [] candidate = MissingQcStallMarksWindow /\
          Bug = SkipMissingQcWindowMarkBug ->
      spec \ {MarkMissingQcWindow}
    [] candidate = HighPriorityForCanonicalNextHeight /\
          Bug = LowPriorityCanonicalBug ->
      spec \ {UseHighPriority}
    [] candidate = LockLagFarFutureExtendsCooldown /\
          Bug = LockLagCooldownNotExtendedBug ->
      spec \ {ExtendCooldown}
    [] candidate = RangePullMetricIncrement /\ Bug = MetricNotIncrementedBug ->
      spec \ {IncrementMetric}
    [] candidate = CanonicalWindowRecordsDependencyWatermark /\
          Bug = DropDependencyWatermarkBug ->
      spec \ {RecordDependencyWatermark}
    [] OTHER -> spec

Init ==
  tried = {}

Next ==
  \E candidate \in Candidates \ tried:
    tried' = tried \cup {candidate}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried: ImplementationActions(candidate) \subseteq Actions

FrontierGapAdmissionCases == {
  NoFutureEvidenceNoRequest, FutureEvidenceAtFrontierNoRequest,
  FutureEvidenceBeyondMissingPayloadRequests, LocalTipPayloadSuppresses,
  ExactBodyOwnerSuppressesGenericPull, ExactBodyLagExpiredRetriesExactRepair,
  DeepCatchupBypassesExactOwnerSuppress
}

FrontierGapAnchorCases == {
  CanonicalReanchorUsesPrevLatestAnchor, NonCanonicalUsesLatestLatestAnchor,
  MissingAnchorSuppresses
}

FrontierGapTargetCases == {
  VoteRosterTargetsPreferred, CommitTopologyFallbackTargets,
  TrustedPeersFallbackTargets, LocalPeerRemovedFromTargets, TargetsSortedDeduped,
  EmptyTargetsSuppress
}

FrontierGapSendAccountingCases == {
  PerPeerCooldownSkipsDuplicate, SentZeroSuppress,
  SuccessfulPullRecordsPermits, SuccessfulPullMarksCanonicalWindow
}

FrontierGapWindowCases == {
  AlreadyEmittedWindowSuppresses, CanonicalStrideSuppressesNonAligned,
  CanonicalStrideAlignedEmits, EveryThirdWindowAllPeers,
  OtherWindowTwoPeerCohort, RecoveryFsmSuppressesWindow,
  MissingQcStallMarksWindow
}

FrontierGapRecoveryMetadataCases == {
  HighPriorityForCanonicalNextHeight, LockLagFarFutureExtendsCooldown,
  RangePullMetricIncrement, CanonicalWindowRecordsDependencyWatermark
}

FrontierGapGroupedCases ==
  FrontierGapAdmissionCases \cup FrontierGapAnchorCases \cup
  FrontierGapTargetCases \cup FrontierGapSendAccountingCases \cup
  FrontierGapWindowCases \cup FrontierGapRecoveryMetadataCases

FrontierGapCaseGroupsComplete ==
  FrontierGapGroupedCases = Candidates

FrontierGapAdmissionExact ==
  \A candidate \in tried:
    candidate \in FrontierGapAdmissionCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

FrontierGapAnchorExact ==
  \A candidate \in tried:
    candidate \in FrontierGapAnchorCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

FrontierGapTargetExact ==
  \A candidate \in tried:
    candidate \in FrontierGapTargetCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

FrontierGapSendAccountingExact ==
  \A candidate \in tried:
    candidate \in FrontierGapSendAccountingCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

FrontierGapWindowExact ==
  \A candidate \in tried:
    candidate \in FrontierGapWindowCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

FrontierGapRecoveryMetadataExact ==
  \A candidate \in tried:
    candidate \in FrontierGapRecoveryMetadataCases =>
      ImplementationActions(candidate) = SpecActions(candidate)

FrontierGapRealignExactness ==
  /\ FrontierGapCaseGroupsComplete
  /\ FrontierGapAdmissionExact
  /\ FrontierGapAnchorExact
  /\ FrontierGapTargetExact
  /\ FrontierGapSendAccountingExact
  /\ FrontierGapWindowExact
  /\ FrontierGapRecoveryMetadataExact

Safety ==
  FrontierGapRealignExactness

====
