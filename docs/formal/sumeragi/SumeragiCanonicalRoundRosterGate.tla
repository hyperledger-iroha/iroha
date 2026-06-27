---- MODULE SumeragiCanonicalRoundRosterGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for canonical consensus round roster selection.

This slice captures `canonical_round_roster_with_mode(...)` and the
roll-forward branch of `roster_from_commit_qc_history_roll_forward(...)`.
Round roster derivation must prefer commit-QC roll-forward evidence before
direct history and pending/live fallbacks, fail closed for unreachable future
heights, use the previous committed topology only at the committed height, and
canonicalize every nonempty roster. Roll-forward evidence must reject known
parent-hash mismatches, prefer exact parent sidecar/block-sync selections,
prefer exact parent QCs over older fallback QCs, enforce strict parent-QC
requirements when pending chain hashes are unavailable, ignore non-commit and
future-parent candidates, skip empty validator sets, sort newest height/view
first, require intermediate hashes, and filter live consensus keys.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FutureNoHistoryEmpty == "future_no_history_empty"
FutureRollForwardWins == "future_roll_forward_wins"
RollForwardBeforeDirectHistory == "roll_forward_before_direct_history"
DirectHistoryBeatsPending == "direct_history_beats_pending"
PendingNextAfterHistoryAbsent == "pending_next_after_history_absent"
PendingCanonicalized == "pending_canonicalized"
CommittedPrevTopologyWins == "committed_prev_topology_wins"
CommittedPrevEmptyUsesActive == "committed_prev_empty_uses_active"
ActiveFallbackCanonicalized == "active_fallback_canonicalized"
RollHeightZeroNone == "roll_height_zero_none"
RollParentHashMismatchNone == "roll_parent_hash_mismatch_none"
RollPersistedParentWins == "roll_persisted_parent_wins"
RollExactCandidateWins == "roll_exact_candidate_wins"
RollStrictParentRejectsFallback == "roll_strict_parent_rejects_fallback"
RollPendingChainAllowsFallback == "roll_pending_chain_allows_fallback"
RollIgnoresNonCommit == "roll_ignores_non_commit"
RollIgnoresCandidateAboveParent == "roll_ignores_candidate_above_parent"
RollSkipsEmptyValidatorSet == "roll_skips_empty_validator_set"
RollNewestCandidateFirst == "roll_newest_candidate_first"
RollRequiresIntermediateHashes == "roll_requires_intermediate_hashes"
RollFiltersLiveKeys == "roll_filters_live_keys"
RollCanonicalized == "roll_canonicalized"

Cases == {
  FutureNoHistoryEmpty,
  FutureRollForwardWins,
  RollForwardBeforeDirectHistory,
  DirectHistoryBeatsPending,
  PendingNextAfterHistoryAbsent,
  PendingCanonicalized,
  CommittedPrevTopologyWins,
  CommittedPrevEmptyUsesActive,
  ActiveFallbackCanonicalized,
  RollHeightZeroNone,
  RollParentHashMismatchNone,
  RollPersistedParentWins,
  RollExactCandidateWins,
  RollStrictParentRejectsFallback,
  RollPendingChainAllowsFallback,
  RollIgnoresNonCommit,
  RollIgnoresCandidateAboveParent,
  RollSkipsEmptyValidatorSet,
  RollNewestCandidateFirst,
  RollRequiresIntermediateHashes,
  RollFiltersLiveKeys,
  RollCanonicalized
}

CanonicalHeightGate == 1
FutureBeyondNext == 2
RollForwardLookup == 3
RollForwardSource == 4
DirectHistoryLookup == 5
DirectHistorySource == 6
PendingLookup == 7
PendingSource == 8
PrevTopologySource == 9
PrevTopologyEmpty == 10
ActiveFallback == 11
ReturnEmpty == 12
ReturnNone == 13
Canonicalize == 14
Dedup == 15
Sort == 16
HeightZeroGuard == 17
TargetParentHash == 18
KnownParentHashCheck == 19
RejectParentMismatch == 20
PersistedParentLookup == 21
BlockSyncParentLookup == 22
ExactParentSelectionSource == 23
CommitHistoryScan == 24
CommitPhaseFilter == 25
TargetParentHeightFilter == 26
CandidateChainHashFilter == 27
ExactCandidateSource == 28
FallbackCandidateSource == 29
StrictParentQc == 30
PendingChainHashes == 31
NewestHeightViewSort == 32
OldestHeightViewSort == 33
EmptyValidatorSkip == 34
EmptyValidatorSource == 35
NonCommitCandidateSource == 36
FutureHeightCandidateSource == 37
RollForwardTopology == 38
IntermediateHashLookup == 39
LiveKeyFilter == 40

SpecActions(c) ==
  CASE c = FutureNoHistoryEmpty ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       FutureBeyondNext, ReturnEmpty}
    [] c = FutureRollForwardWins ->
      {CanonicalHeightGate, RollForwardLookup, RollForwardSource,
       Canonicalize, Dedup, Sort}
    [] c = RollForwardBeforeDirectHistory ->
      {CanonicalHeightGate, RollForwardLookup, RollForwardSource,
       Canonicalize, Dedup, Sort}
    [] c = DirectHistoryBeatsPending ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       DirectHistorySource, Canonicalize, Dedup, Sort}
    [] c = PendingNextAfterHistoryAbsent ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] c = PendingCanonicalized ->
      {PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] c = CommittedPrevTopologyWins ->
      {PendingLookup, PrevTopologySource, Canonicalize, Dedup, Sort}
    [] c = CommittedPrevEmptyUsesActive ->
      {PendingLookup, PrevTopologyEmpty, ActiveFallback, Canonicalize,
       Dedup, Sort}
    [] c = ActiveFallbackCanonicalized ->
      {PendingLookup, ActiveFallback, Canonicalize, Dedup, Sort}
    [] c = RollHeightZeroNone ->
      {HeightZeroGuard, ReturnNone}
    [] c = RollParentHashMismatchNone ->
      {TargetParentHash, KnownParentHashCheck, RejectParentMismatch,
       ReturnNone}
    [] c = RollPersistedParentWins ->
      {TargetParentHash, PersistedParentLookup, BlockSyncParentLookup,
       ExactParentSelectionSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] c = RollExactCandidateWins ->
      {CommitHistoryScan, CommitPhaseFilter, ExactCandidateSource,
       NewestHeightViewSort, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] c = RollStrictParentRejectsFallback ->
      {TargetParentHash, StrictParentQc, CommitHistoryScan, CommitPhaseFilter,
       ReturnNone}
    [] c = RollPendingChainAllowsFallback ->
      {TargetParentHash, PendingChainHashes, CommitHistoryScan,
       CommitPhaseFilter, CandidateChainHashFilter, FallbackCandidateSource,
       NewestHeightViewSort, RollForwardTopology, IntermediateHashLookup,
       LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] c = RollIgnoresNonCommit ->
      {CommitHistoryScan, CommitPhaseFilter, FallbackCandidateSource,
       RollForwardTopology, LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] c = RollIgnoresCandidateAboveParent ->
      {CommitHistoryScan, TargetParentHeightFilter, FallbackCandidateSource,
       RollForwardTopology, LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] c = RollSkipsEmptyValidatorSet ->
      {CommitHistoryScan, CommitPhaseFilter, EmptyValidatorSkip,
       FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] c = RollNewestCandidateFirst ->
      {CommitHistoryScan, CommitPhaseFilter, NewestHeightViewSort,
       FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] c = RollRequiresIntermediateHashes ->
      {FallbackCandidateSource, RollForwardTopology, IntermediateHashLookup,
       ReturnNone}
    [] c = RollFiltersLiveKeys ->
      {FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] c = RollCanonicalized ->
      {FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] OTHER -> {}

ActualActions(c) ==
  CASE Bug = "allow_future_without_history"
       /\ c = FutureNoHistoryEmpty ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       FutureBeyondNext, ActiveFallback, Canonicalize, Dedup, Sort}
    [] Bug = "skip_roll_forward"
       /\ c = FutureRollForwardWins ->
      {CanonicalHeightGate, DirectHistoryLookup, FutureBeyondNext,
       ReturnEmpty}
    [] Bug = "direct_history_before_roll_forward"
       /\ c = RollForwardBeforeDirectHistory ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       DirectHistorySource, Canonicalize, Dedup, Sort}
    [] Bug = "pending_before_history"
       /\ c = DirectHistoryBeatsPending ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] Bug = "skip_pending_next"
       /\ c = PendingNextAfterHistoryAbsent ->
      {CanonicalHeightGate, RollForwardLookup, DirectHistoryLookup,
       ActiveFallback, Canonicalize, Dedup, Sort}
    [] Bug = "skip_prev_committed"
       /\ c = CommittedPrevTopologyWins ->
      {PendingLookup, ActiveFallback, Canonicalize, Dedup, Sort}
    [] Bug = "prev_empty_blocks_active"
       /\ c = CommittedPrevEmptyUsesActive ->
      {PendingLookup, PrevTopologyEmpty, ReturnEmpty}
    [] Bug = "skip_canonicalize"
       /\ c = RollCanonicalized ->
      {FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Dedup, Sort}
    [] Bug = "preserve_duplicates"
       /\ c = RollCanonicalized ->
      {FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Sort}
    [] Bug = "preserve_order"
       /\ c = RollCanonicalized ->
      {FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup}
    [] Bug = "height_zero_rolls_forward"
       /\ c = RollHeightZeroNone ->
      {RollForwardTopology, LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] Bug = "accept_parent_hash_mismatch"
       /\ c = RollParentHashMismatchNone ->
      {TargetParentHash, KnownParentHashCheck, CommitHistoryScan,
       FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] Bug = "persisted_parent_not_preferred"
       /\ c = RollPersistedParentWins ->
      {TargetParentHash, PersistedParentLookup, BlockSyncParentLookup,
       CommitHistoryScan, ExactCandidateSource, RollForwardTopology,
       LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] Bug = "exact_candidate_not_preferred"
       /\ c = RollExactCandidateWins ->
      {CommitHistoryScan, CommitPhaseFilter, FallbackCandidateSource,
       NewestHeightViewSort, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] Bug = "strict_parent_allows_fallback"
       /\ c = RollStrictParentRejectsFallback ->
      {TargetParentHash, StrictParentQc, CommitHistoryScan, CommitPhaseFilter,
       FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] Bug = "pending_chain_ignored"
       /\ c = RollPendingChainAllowsFallback ->
      {TargetParentHash, CommitHistoryScan, CommitPhaseFilter, ReturnNone}
    [] Bug = "accept_non_commit_history"
       /\ c = RollIgnoresNonCommit ->
      {CommitHistoryScan, NonCommitCandidateSource, RollForwardTopology,
       LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] Bug = "accept_candidate_above_parent"
       /\ c = RollIgnoresCandidateAboveParent ->
      {CommitHistoryScan, FutureHeightCandidateSource, RollForwardTopology,
       LiveKeyFilter, Canonicalize, Dedup, Sort}
    [] Bug = "empty_validator_set_selected"
       /\ c = RollSkipsEmptyValidatorSet ->
      {CommitHistoryScan, CommitPhaseFilter, EmptyValidatorSource,
       RollForwardTopology, ReturnNone}
    [] Bug = "oldest_candidate_first"
       /\ c = RollNewestCandidateFirst ->
      {CommitHistoryScan, CommitPhaseFilter, OldestHeightViewSort,
       FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] Bug = "missing_intermediate_hash_rolls"
       /\ c = RollRequiresIntermediateHashes ->
      {FallbackCandidateSource, RollForwardTopology, LiveKeyFilter,
       Canonicalize, Dedup, Sort}
    [] Bug = "skip_live_key_filter"
       /\ c = RollFiltersLiveKeys ->
      {FallbackCandidateSource, RollForwardTopology, Canonicalize, Dedup,
       Sort}
    [] OTHER -> SpecActions(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

CanonicalRoundRosterMatchesSpec ==
  \A c \in Cases: ActualActions(c) = SpecActions(c)

CanonicalRoundRosterExactness ==
  CanonicalRoundRosterMatchesSpec

CanonicalRoundRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CanonicalRoundRosterExactness

SafetyFast == CanonicalRoundRosterExactness

Safety ==
  CanonicalRoundRosterCorrectnessEnvelope

BugAllowFutureWithoutHistory ==
  ActualActions(FutureNoHistoryEmpty) = SpecActions(FutureNoHistoryEmpty)

BugSkipRollForward ==
  ActualActions(FutureRollForwardWins) = SpecActions(FutureRollForwardWins)

BugDirectHistoryBeforeRollForward ==
  ActualActions(RollForwardBeforeDirectHistory) =
    SpecActions(RollForwardBeforeDirectHistory)

BugPendingBeforeHistory ==
  ActualActions(DirectHistoryBeatsPending) =
    SpecActions(DirectHistoryBeatsPending)

BugSkipPendingNext ==
  ActualActions(PendingNextAfterHistoryAbsent) =
    SpecActions(PendingNextAfterHistoryAbsent)

BugSkipPrevCommitted ==
  ActualActions(CommittedPrevTopologyWins) =
    SpecActions(CommittedPrevTopologyWins)

BugPrevEmptyBlocksActive ==
  ActualActions(CommittedPrevEmptyUsesActive) =
    SpecActions(CommittedPrevEmptyUsesActive)

BugSkipCanonicalize ==
  ActualActions(RollCanonicalized) = SpecActions(RollCanonicalized)

BugPreserveDuplicates ==
  ActualActions(RollCanonicalized) = SpecActions(RollCanonicalized)

BugPreserveOrder ==
  ActualActions(RollCanonicalized) = SpecActions(RollCanonicalized)

BugHeightZeroRollsForward ==
  ActualActions(RollHeightZeroNone) = SpecActions(RollHeightZeroNone)

BugAcceptParentHashMismatch ==
  ActualActions(RollParentHashMismatchNone) =
    SpecActions(RollParentHashMismatchNone)

BugPersistedParentNotPreferred ==
  ActualActions(RollPersistedParentWins) =
    SpecActions(RollPersistedParentWins)

BugExactCandidateNotPreferred ==
  ActualActions(RollExactCandidateWins) = SpecActions(RollExactCandidateWins)

BugStrictParentAllowsFallback ==
  ActualActions(RollStrictParentRejectsFallback) =
    SpecActions(RollStrictParentRejectsFallback)

BugPendingChainIgnored ==
  ActualActions(RollPendingChainAllowsFallback) =
    SpecActions(RollPendingChainAllowsFallback)

BugAcceptNonCommitHistory ==
  ActualActions(RollIgnoresNonCommit) = SpecActions(RollIgnoresNonCommit)

BugAcceptCandidateAboveParent ==
  ActualActions(RollIgnoresCandidateAboveParent) =
    SpecActions(RollIgnoresCandidateAboveParent)

BugEmptyValidatorSetSelected ==
  ActualActions(RollSkipsEmptyValidatorSet) =
    SpecActions(RollSkipsEmptyValidatorSet)

BugOldestCandidateFirst ==
  ActualActions(RollNewestCandidateFirst) =
    SpecActions(RollNewestCandidateFirst)

BugMissingIntermediateHashRolls ==
  ActualActions(RollRequiresIntermediateHashes) =
    SpecActions(RollRequiresIntermediateHashes)

BugSkipLiveKeyFilter ==
  ActualActions(RollFiltersLiveKeys) = SpecActions(RollFiltersLiveKeys)

====
