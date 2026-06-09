---- MODULE SumeragiVoteRosterSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for block-specific vote roster selection.

This slice captures `roster_for_vote_with_mode(...)`,
`validation_roster_for_vote_with_mode(...)`, and
`roster_for_new_view_with_mode(...)`. Vote validation must keep validated
cached rosters stable for non-frontier heights, but must let exact sidecar or
block-sync evidence and the live next-height roster override stale cached
material at `committed_height + 1`. Historical votes remap through Kura block
height/view metadata before consulting persisted/block-sync roster material,
the committed height may fall back to the previous commit topology, older
committed heights fail closed when no canonical roster evidence exists, and
future pending blocks may use parent-hash roll-forward before the canonical
round roster fallback. The validation wrapper may use a cached roster only when
the cached height matches the vote height, and the NEW_VIEW helper has its own
past/live/future routing.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

VoteCachedPastWins == "vote_cached_past_wins"
VoteNextExactBeforeCache == "vote_next_exact_before_cache"
VoteNextLiveBeforeCache == "vote_next_live_before_cache"
VoteNextCachedFallbackWhenLiveEmpty ==
  "vote_next_cached_fallback_when_live_empty"
VoteExactPersistedWins == "vote_exact_persisted_wins"
VoteExactBlockSyncWins == "vote_exact_block_sync_wins"
VoteEmptyExactIgnored == "vote_empty_exact_ignored"
VoteHistoricalRemapsBlockHeightView ==
  "vote_historical_remaps_block_height_view"
VoteHistoricalFallbackUsesIncomingView ==
  "vote_historical_fallback_uses_incoming_view"
VoteCommittedPrevTopologyWins == "vote_committed_prev_topology_wins"
VoteCommittedPrevEmptyUsesCanonical ==
  "vote_committed_prev_empty_uses_canonical"
VotePastNoSelectionEmpty == "vote_past_no_selection_empty"
VoteFuturePendingRollForwardWins == "vote_future_pending_roll_forward_wins"
VoteFuturePendingRollForwardMissBeyondNextEmpty ==
  "vote_future_pending_roll_forward_miss_beyond_next_empty"
VoteFutureNoPendingUsesCanonical == "vote_future_no_pending_uses_canonical"
VoteCanonicalized == "vote_canonicalized"
ValidationCacheHeightMatchWins == "validation_cache_height_match_wins"
ValidationCacheHeightMismatchObservesSidecar ==
  "validation_cache_height_mismatch_observes_sidecar"
ValidationEmptyCacheObservesSidecar ==
  "validation_empty_cache_observes_sidecar"
NewViewPastDelegatesVote == "new_view_past_delegates_vote"
NewViewNextUsesLive == "new_view_next_uses_live"
NewViewFutureRollForwardWins == "new_view_future_roll_forward_wins"
NewViewFutureCanonicalFallback == "new_view_future_canonical_fallback"
NewViewFutureCanonicalEmpty == "new_view_future_canonical_empty"

Cases == {
  VoteCachedPastWins,
  VoteNextExactBeforeCache,
  VoteNextLiveBeforeCache,
  VoteNextCachedFallbackWhenLiveEmpty,
  VoteExactPersistedWins,
  VoteExactBlockSyncWins,
  VoteEmptyExactIgnored,
  VoteHistoricalRemapsBlockHeightView,
  VoteHistoricalFallbackUsesIncomingView,
  VoteCommittedPrevTopologyWins,
  VoteCommittedPrevEmptyUsesCanonical,
  VotePastNoSelectionEmpty,
  VoteFuturePendingRollForwardWins,
  VoteFuturePendingRollForwardMissBeyondNextEmpty,
  VoteFutureNoPendingUsesCanonical,
  VoteCanonicalized,
  ValidationCacheHeightMatchWins,
  ValidationCacheHeightMismatchObservesSidecar,
  ValidationEmptyCacheObservesSidecar,
  NewViewPastDelegatesVote,
  NewViewNextUsesLive,
  NewViewFutureRollForwardWins,
  NewViewFutureCanonicalFallback,
  NewViewFutureCanonicalEmpty
}

CacheLookup == 1
CacheNonEmpty == 2
CacheHeightCheck == 3
CachedSource == 4
NextHeightGuard == 5
SidecarQuarantineGate == 6
ExactPersistedLookup == 7
ExactBlockSyncLookup == 8
ExactSelectionSource == 9
NonEmptySelectionGuard == 10
LiveRosterLookup == 11
LiveSource == 12
HistoricalCommittedGuard == 13
HistoricalHeightLookup == 14
HistoricalBlockViewLookup == 15
HistoricalIncomingViewFallback == 16
HistoricalRosterLookup == 17
PrevTopologySource == 18
PrevTopologyEmpty == 19
PendingBlockLookup == 20
PendingHeightCheck == 21
ParentHashSource == 22
RollForwardLookup == 23
RollForwardSource == 24
CanonicalRoundLookup == 25
CanonicalRoundSource == 26
ReturnEmpty == 27
ObserveSidecarMismatch == 28
DelegateVoteRoster == 29
Canonicalize == 30
Dedup == 31
Sort == 32

SpecActions(c) ==
  CASE c = VoteCachedPastWins ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, CachedSource,
       Canonicalize, Dedup, Sort}
    [] c = VoteNextExactBeforeCache ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, SidecarQuarantineGate,
       ExactPersistedLookup, ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] c = VoteNextLiveBeforeCache ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, SidecarQuarantineGate,
       ExactPersistedLookup, ExactBlockSyncLookup, LiveRosterLookup,
       LiveSource, Canonicalize, Dedup, Sort}
    [] c = VoteNextCachedFallbackWhenLiveEmpty ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, SidecarQuarantineGate,
       ExactPersistedLookup, ExactBlockSyncLookup, LiveRosterLookup,
       CachedSource, Canonicalize, Dedup, Sort}
    [] c = VoteExactPersistedWins ->
      {SidecarQuarantineGate, ExactPersistedLookup, ExactSelectionSource,
       Canonicalize, Dedup, Sort}
    [] c = VoteExactBlockSyncWins ->
      {SidecarQuarantineGate, ExactPersistedLookup, ExactBlockSyncLookup,
       ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] c = VoteEmptyExactIgnored ->
      {SidecarQuarantineGate, ExactPersistedLookup, ExactBlockSyncLookup,
       NonEmptySelectionGuard, CanonicalRoundLookup, CanonicalRoundSource,
       Canonicalize, Dedup, Sort}
    [] c = VoteHistoricalRemapsBlockHeightView ->
      {HistoricalCommittedGuard, HistoricalHeightLookup,
       HistoricalBlockViewLookup, SidecarQuarantineGate,
       HistoricalRosterLookup, ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] c = VoteHistoricalFallbackUsesIncomingView ->
      {HistoricalCommittedGuard, HistoricalHeightLookup,
       HistoricalIncomingViewFallback, SidecarQuarantineGate,
       HistoricalRosterLookup, ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] c = VoteCommittedPrevTopologyWins ->
      {HistoricalCommittedGuard, HistoricalRosterLookup, PrevTopologySource,
       Canonicalize, Dedup, Sort}
    [] c = VoteCommittedPrevEmptyUsesCanonical ->
      {HistoricalCommittedGuard, HistoricalRosterLookup, PrevTopologyEmpty,
       CanonicalRoundLookup, CanonicalRoundSource, Canonicalize, Dedup, Sort}
    [] c = VotePastNoSelectionEmpty ->
      {HistoricalCommittedGuard, HistoricalRosterLookup, ReturnEmpty}
    [] c = VoteFuturePendingRollForwardWins ->
      {PendingBlockLookup, PendingHeightCheck, ParentHashSource,
       RollForwardLookup, RollForwardSource, Canonicalize, Dedup, Sort}
    [] c = VoteFuturePendingRollForwardMissBeyondNextEmpty ->
      {PendingBlockLookup, PendingHeightCheck, ParentHashSource,
       RollForwardLookup, ReturnEmpty}
    [] c = VoteFutureNoPendingUsesCanonical ->
      {PendingBlockLookup, CanonicalRoundLookup, CanonicalRoundSource,
       Canonicalize, Dedup, Sort}
    [] c = VoteCanonicalized ->
      {ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] c = ValidationCacheHeightMatchWins ->
      {CacheLookup, CacheNonEmpty, CacheHeightCheck, CachedSource,
       Canonicalize, Dedup, Sort}
    [] c = ValidationCacheHeightMismatchObservesSidecar ->
      {CacheLookup, CacheNonEmpty, CacheHeightCheck, ObserveSidecarMismatch,
       DelegateVoteRoster}
    [] c = ValidationEmptyCacheObservesSidecar ->
      {CacheLookup, ObserveSidecarMismatch, DelegateVoteRoster}
    [] c = NewViewPastDelegatesVote ->
      {HistoricalCommittedGuard, DelegateVoteRoster}
    [] c = NewViewNextUsesLive ->
      {NextHeightGuard, LiveRosterLookup, LiveSource}
    [] c = NewViewFutureRollForwardWins ->
      {RollForwardLookup, RollForwardSource, Canonicalize, Dedup, Sort}
    [] c = NewViewFutureCanonicalFallback ->
      {RollForwardLookup, CanonicalRoundLookup, CanonicalRoundSource}
    [] c = NewViewFutureCanonicalEmpty ->
      {RollForwardLookup, CanonicalRoundLookup, ReturnEmpty}
    [] OTHER -> {}

ActualActions(c) ==
  CASE Bug = "skip_cache_for_past"
       /\ c = VoteCachedPastWins ->
      {CacheLookup, SidecarQuarantineGate, ExactPersistedLookup,
       CanonicalRoundLookup, CanonicalRoundSource, Canonicalize, Dedup, Sort}
    [] Bug = "cache_used_for_next_height"
       /\ c = VoteNextExactBeforeCache ->
      {CacheLookup, CacheNonEmpty, CachedSource, Canonicalize, Dedup, Sort}
    [] Bug = "block_sync_before_persisted"
       /\ c = VoteExactPersistedWins ->
      {SidecarQuarantineGate, ExactBlockSyncLookup, ExactSelectionSource,
       Canonicalize, Dedup, Sort}
    [] Bug = "empty_exact_roster_selected"
       /\ c = VoteEmptyExactIgnored ->
      {SidecarQuarantineGate, ExactPersistedLookup, ExactSelectionSource,
       Canonicalize, Dedup, Sort}
    [] Bug = "skip_sidecar_quarantine"
       /\ c = VoteExactPersistedWins ->
      {ExactPersistedLookup, ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] Bug = "skip_next_live"
       /\ c = VoteNextLiveBeforeCache ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, SidecarQuarantineGate,
       ExactPersistedLookup, ExactBlockSyncLookup, CanonicalRoundLookup,
       CanonicalRoundSource, Canonicalize, Dedup, Sort}
    [] Bug = "next_cached_before_live"
       /\ c = VoteNextLiveBeforeCache ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, CachedSource,
       Canonicalize, Dedup, Sort}
    [] Bug = "drop_cached_fallback"
       /\ c = VoteNextCachedFallbackWhenLiveEmpty ->
      {CacheLookup, CacheNonEmpty, NextHeightGuard, SidecarQuarantineGate,
       ExactPersistedLookup, ExactBlockSyncLookup, LiveRosterLookup,
       ReturnEmpty}
    [] Bug = "historical_uses_requested_height"
       /\ c = VoteHistoricalRemapsBlockHeightView ->
      {HistoricalCommittedGuard, SidecarQuarantineGate, HistoricalRosterLookup,
       ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] Bug = "historical_drops_block_view"
       /\ c = VoteHistoricalRemapsBlockHeightView ->
      {HistoricalCommittedGuard, HistoricalHeightLookup,
       HistoricalIncomingViewFallback, SidecarQuarantineGate,
       HistoricalRosterLookup, ExactSelectionSource, Canonicalize, Dedup, Sort}
    [] Bug = "historical_missing_view_no_fallback"
       /\ c = VoteHistoricalFallbackUsesIncomingView ->
      {HistoricalCommittedGuard, HistoricalHeightLookup,
       SidecarQuarantineGate, HistoricalRosterLookup, ReturnEmpty}
    [] Bug = "committed_skips_prev"
       /\ c = VoteCommittedPrevTopologyWins ->
      {HistoricalCommittedGuard, HistoricalRosterLookup,
       CanonicalRoundLookup, CanonicalRoundSource, Canonicalize, Dedup, Sort}
    [] Bug = "prev_empty_returns_empty"
       /\ c = VoteCommittedPrevEmptyUsesCanonical ->
      {HistoricalCommittedGuard, HistoricalRosterLookup, PrevTopologyEmpty,
       ReturnEmpty}
    [] Bug = "past_falls_through_canonical"
       /\ c = VotePastNoSelectionEmpty ->
      {HistoricalCommittedGuard, HistoricalRosterLookup, CanonicalRoundLookup,
       CanonicalRoundSource, Canonicalize, Dedup, Sort}
    [] Bug = "pending_roll_forward_uses_block_hash"
       /\ c = VoteFuturePendingRollForwardWins ->
      {PendingBlockLookup, PendingHeightCheck, RollForwardLookup,
       RollForwardSource, Canonicalize, Dedup, Sort}
    [] Bug = "future_parent_miss_uses_canonical"
       /\ c = VoteFuturePendingRollForwardMissBeyondNextEmpty ->
      {PendingBlockLookup, PendingHeightCheck, ParentHashSource,
       RollForwardLookup, CanonicalRoundLookup, CanonicalRoundSource,
       Canonicalize, Dedup, Sort}
    [] Bug = "skip_final_canonical"
       /\ c = VoteFutureNoPendingUsesCanonical ->
      {PendingBlockLookup, ReturnEmpty}
    [] Bug = "skip_canonicalize"
       /\ c = VoteCanonicalized ->
      {ExactSelectionSource, Dedup, Sort}
    [] Bug = "preserve_duplicates"
       /\ c = VoteCanonicalized ->
      {ExactSelectionSource, Canonicalize, Sort}
    [] Bug = "preserve_order"
       /\ c = VoteCanonicalized ->
      {ExactSelectionSource, Canonicalize, Dedup}
    [] Bug = "validation_cache_ignores_height"
       /\ c = ValidationCacheHeightMismatchObservesSidecar ->
      {CacheLookup, CacheNonEmpty, CachedSource, Canonicalize, Dedup, Sort}
    [] Bug = "validation_empty_cache_accepted"
       /\ c = ValidationEmptyCacheObservesSidecar ->
      {CacheLookup, CachedSource, Canonicalize, Dedup, Sort}
    [] Bug = "validation_no_sidecar_observe"
       /\ c = ValidationCacheHeightMismatchObservesSidecar ->
      {CacheLookup, CacheNonEmpty, CacheHeightCheck, DelegateVoteRoster}
    [] Bug = "new_view_past_uses_live"
       /\ c = NewViewPastDelegatesVote ->
      {HistoricalCommittedGuard, LiveRosterLookup, LiveSource}
    [] Bug = "new_view_next_uses_vote_selector"
       /\ c = NewViewNextUsesLive ->
      {NextHeightGuard, DelegateVoteRoster}
    [] Bug = "new_view_future_skips_roll_forward"
       /\ c = NewViewFutureRollForwardWins ->
      {CanonicalRoundLookup, CanonicalRoundSource}
    [] Bug = "new_view_future_drops_canonical"
       /\ c = NewViewFutureCanonicalFallback ->
      {RollForwardLookup, CanonicalRoundLookup, ReturnEmpty}
    [] OTHER -> SpecActions(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

VoteRosterSelectionCoreSafety ==
  \A c \in Cases: ActualActions(c) = SpecActions(c)

SafetyFast ==
  VoteRosterSelectionCoreSafety

BugSkipCacheForPast ==
  ActualActions(VoteCachedPastWins) = SpecActions(VoteCachedPastWins)

BugCacheUsedForNextHeight ==
  ActualActions(VoteNextExactBeforeCache) =
    SpecActions(VoteNextExactBeforeCache)

BugBlockSyncBeforePersisted ==
  ActualActions(VoteExactPersistedWins) = SpecActions(VoteExactPersistedWins)

BugEmptyExactRosterSelected ==
  ActualActions(VoteEmptyExactIgnored) = SpecActions(VoteEmptyExactIgnored)

BugSkipSidecarQuarantine ==
  ActualActions(VoteExactPersistedWins) = SpecActions(VoteExactPersistedWins)

BugSkipNextLive ==
  ActualActions(VoteNextLiveBeforeCache) =
    SpecActions(VoteNextLiveBeforeCache)

BugNextCachedBeforeLive ==
  ActualActions(VoteNextLiveBeforeCache) =
    SpecActions(VoteNextLiveBeforeCache)

BugDropCachedFallback ==
  ActualActions(VoteNextCachedFallbackWhenLiveEmpty) =
    SpecActions(VoteNextCachedFallbackWhenLiveEmpty)

BugHistoricalUsesRequestedHeight ==
  ActualActions(VoteHistoricalRemapsBlockHeightView) =
    SpecActions(VoteHistoricalRemapsBlockHeightView)

BugHistoricalDropsBlockView ==
  ActualActions(VoteHistoricalRemapsBlockHeightView) =
    SpecActions(VoteHistoricalRemapsBlockHeightView)

BugHistoricalMissingViewNoFallback ==
  ActualActions(VoteHistoricalFallbackUsesIncomingView) =
    SpecActions(VoteHistoricalFallbackUsesIncomingView)

BugCommittedSkipsPrev ==
  ActualActions(VoteCommittedPrevTopologyWins) =
    SpecActions(VoteCommittedPrevTopologyWins)

BugPrevEmptyReturnsEmpty ==
  ActualActions(VoteCommittedPrevEmptyUsesCanonical) =
    SpecActions(VoteCommittedPrevEmptyUsesCanonical)

BugPastFallsThroughCanonical ==
  ActualActions(VotePastNoSelectionEmpty) =
    SpecActions(VotePastNoSelectionEmpty)

BugPendingRollForwardUsesBlockHash ==
  ActualActions(VoteFuturePendingRollForwardWins) =
    SpecActions(VoteFuturePendingRollForwardWins)

BugFutureParentMissUsesCanonical ==
  ActualActions(VoteFuturePendingRollForwardMissBeyondNextEmpty) =
    SpecActions(VoteFuturePendingRollForwardMissBeyondNextEmpty)

BugSkipFinalCanonical ==
  ActualActions(VoteFutureNoPendingUsesCanonical) =
    SpecActions(VoteFutureNoPendingUsesCanonical)

BugSkipCanonicalize ==
  ActualActions(VoteCanonicalized) = SpecActions(VoteCanonicalized)

BugPreserveDuplicates ==
  ActualActions(VoteCanonicalized) = SpecActions(VoteCanonicalized)

BugPreserveOrder ==
  ActualActions(VoteCanonicalized) = SpecActions(VoteCanonicalized)

BugValidationCacheIgnoresHeight ==
  ActualActions(ValidationCacheHeightMismatchObservesSidecar) =
    SpecActions(ValidationCacheHeightMismatchObservesSidecar)

BugValidationEmptyCacheAccepted ==
  ActualActions(ValidationEmptyCacheObservesSidecar) =
    SpecActions(ValidationEmptyCacheObservesSidecar)

BugValidationNoSidecarObserve ==
  ActualActions(ValidationCacheHeightMismatchObservesSidecar) =
    SpecActions(ValidationCacheHeightMismatchObservesSidecar)

BugNewViewPastUsesLive ==
  ActualActions(NewViewPastDelegatesVote) = SpecActions(NewViewPastDelegatesVote)

BugNewViewNextUsesVoteSelector ==
  ActualActions(NewViewNextUsesLive) = SpecActions(NewViewNextUsesLive)

BugNewViewFutureSkipsRollForward ==
  ActualActions(NewViewFutureRollForwardWins) =
    SpecActions(NewViewFutureRollForwardWins)

BugNewViewFutureDropsCanonical ==
  ActualActions(NewViewFutureCanonicalFallback) =
    SpecActions(NewViewFutureCanonicalFallback)

====
