---- MODULE SumeragiVoteRosterCacheGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for vote roster cache/support helpers.

This slice captures `cache_vote_roster(...)`,
`cached_vote_verify_pops(...)`, `local_commit_vote_roster(...)`, and
`vote_emission_topology_for_height(...)`. Cached vote rosters must ignore empty
inputs, canonicalize before first insert, keep the original cached roster and
round metadata on later observations, and replay deferred votes after every
nonempty observation. Vote verification POP caches must return exact cache
hits, build misses only from roster peers, prefer roster-validation POPs before
trusted fallback POPs, omit missing/non-roster keys, deduplicate duplicate
roster keys through map semantics, clear only at the size cap, insert under the
given roster hash, and cache empty maps for empty rosters. Local vote emission
helpers must use live next-height rosters when nonempty and otherwise preserve
their supplied fallback topology.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CacheVoteEmptyIgnored == "cache_vote_empty_ignored"
CacheVoteVacantInsertCanonical == "cache_vote_vacant_insert_canonical"
CacheVoteOccupiedDifferentRosterKeepsCached ==
  "cache_vote_occupied_different_roster_keeps_cached"
CacheVoteOccupiedSameRosterDifferentRoundKeepsCached ==
  "cache_vote_occupied_same_roster_different_round_keeps_cached"
CacheVoteOccupiedSameKeepsCached == "cache_vote_occupied_same_keeps_cached"
CacheVoteReplayAfterInsert == "cache_vote_replay_after_insert"
CacheVoteReplayAfterOccupied == "cache_vote_replay_after_occupied"
PopCacheHitReturnsCached == "pop_cache_hit_returns_cached"
PopMissUsesValidationPop == "pop_miss_uses_validation_pop"
PopMissUsesTrustedFallback == "pop_miss_uses_trusted_fallback"
PopValidationOverridesTrusted == "pop_validation_overrides_trusted"
PopMissingOmitted == "pop_missing_omitted"
PopNonRosterOmitted == "pop_non_roster_omitted"
PopDuplicateRosterKeyDeduped == "pop_duplicate_roster_key_deduped"
PopBelowMaxKeepsExistingAndInserts ==
  "pop_below_max_keeps_existing_and_inserts"
PopAtMaxClearsThenInserts == "pop_at_max_clears_then_inserts"
PopEmptyRosterCachesEmpty == "pop_empty_roster_caches_empty"
LocalCommitNextLiveWins == "local_commit_next_live_wins"
LocalCommitNextEmptyFallback == "local_commit_next_empty_fallback"
LocalCommitPastNoLiveLookup == "local_commit_past_no_live_lookup"
EmissionNextLiveWins == "emission_next_live_wins"
EmissionNextEmptyFallback == "emission_next_empty_fallback"
EmissionPastFallbackClone == "emission_past_fallback_clone"

Cases == {
  CacheVoteEmptyIgnored,
  CacheVoteVacantInsertCanonical,
  CacheVoteOccupiedDifferentRosterKeepsCached,
  CacheVoteOccupiedSameRosterDifferentRoundKeepsCached,
  CacheVoteOccupiedSameKeepsCached,
  CacheVoteReplayAfterInsert,
  CacheVoteReplayAfterOccupied,
  PopCacheHitReturnsCached,
  PopMissUsesValidationPop,
  PopMissUsesTrustedFallback,
  PopValidationOverridesTrusted,
  PopMissingOmitted,
  PopNonRosterOmitted,
  PopDuplicateRosterKeyDeduped,
  PopBelowMaxKeepsExistingAndInserts,
  PopAtMaxClearsThenInserts,
  PopEmptyRosterCachesEmpty,
  LocalCommitNextLiveWins,
  LocalCommitNextEmptyFallback,
  LocalCommitPastNoLiveLookup,
  EmissionNextLiveWins,
  EmissionNextEmptyFallback,
  EmissionPastFallbackClone
}

InputEmptyGuard == 1
ReturnWithoutInsert == 2
ConsensusModeLookup == 3
Canonicalize == 4
Dedup == 5
Sort == 6
VoteCacheLookup == 7
VacantEntry == 8
OccupiedEntry == 9
CacheInsert == 10
KeepExisting == 11
RosterMismatchWarn == 12
RoundMismatchWarn == 13
ReplayDeferred == 14
PopCacheLookup == 15
ReturnCachedPops == 16
BuildPops == 17
RosterValidationPop == 18
TrustedPop == 19
ValidationPreferred == 20
MissingPopOmitted == 21
NonRosterPopOmitted == 22
DedupPopKeys == 23
PopCacheSizeCheck == 24
PreserveExistingPopCache == 25
ClearPopCache == 26
PopCacheInsert == 27
InsertUnderRosterHash == 28
EmptyPopMap == 29
NextHeightGuard == 30
LiveRosterLookup == 31
LiveSource == 32
FallbackCommitTopology == 33
FallbackTopologyClone == 34

SpecActions(c) ==
  CASE c = CacheVoteEmptyIgnored ->
      {InputEmptyGuard, ReturnWithoutInsert}
    [] c = CacheVoteVacantInsertCanonical ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, VacantEntry, CacheInsert, ReplayDeferred}
    [] c = CacheVoteOccupiedDifferentRosterKeepsCached ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, KeepExisting, RosterMismatchWarn,
       ReplayDeferred}
    [] c = CacheVoteOccupiedSameRosterDifferentRoundKeepsCached ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, KeepExisting, RoundMismatchWarn,
       ReplayDeferred}
    [] c = CacheVoteOccupiedSameKeepsCached ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, KeepExisting, ReplayDeferred}
    [] c = CacheVoteReplayAfterInsert ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, VacantEntry, CacheInsert, ReplayDeferred}
    [] c = CacheVoteReplayAfterOccupied ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, KeepExisting, ReplayDeferred}
    [] c = PopCacheHitReturnsCached ->
      {PopCacheLookup, ReturnCachedPops}
    [] c = PopMissUsesValidationPop ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = PopMissUsesTrustedFallback ->
      {PopCacheLookup, BuildPops, TrustedPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = PopValidationOverridesTrusted ->
      {PopCacheLookup, BuildPops, RosterValidationPop, ValidationPreferred,
       PopCacheSizeCheck, PreserveExistingPopCache, PopCacheInsert,
       InsertUnderRosterHash}
    [] c = PopMissingOmitted ->
      {PopCacheLookup, BuildPops, MissingPopOmitted, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = PopNonRosterOmitted ->
      {PopCacheLookup, BuildPops, NonRosterPopOmitted, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = PopDuplicateRosterKeyDeduped ->
      {PopCacheLookup, BuildPops, RosterValidationPop, DedupPopKeys,
       PopCacheSizeCheck, PreserveExistingPopCache, PopCacheInsert,
       InsertUnderRosterHash}
    [] c = PopBelowMaxKeepsExistingAndInserts ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = PopAtMaxClearsThenInserts ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       ClearPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = PopEmptyRosterCachesEmpty ->
      {PopCacheLookup, BuildPops, EmptyPopMap, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] c = LocalCommitNextLiveWins ->
      {NextHeightGuard, LiveRosterLookup, LiveSource}
    [] c = LocalCommitNextEmptyFallback ->
      {NextHeightGuard, LiveRosterLookup, FallbackCommitTopology}
    [] c = LocalCommitPastNoLiveLookup ->
      {FallbackCommitTopology}
    [] c = EmissionNextLiveWins ->
      {NextHeightGuard, LiveRosterLookup, LiveSource}
    [] c = EmissionNextEmptyFallback ->
      {NextHeightGuard, LiveRosterLookup, FallbackTopologyClone}
    [] c = EmissionPastFallbackClone ->
      {FallbackTopologyClone}
    [] OTHER -> {}

ActualActions(c) ==
  CASE Bug = "cache_empty_inserts"
       /\ c = CacheVoteEmptyIgnored ->
      {InputEmptyGuard, ConsensusModeLookup, VoteCacheLookup, VacantEntry,
       CacheInsert, ReplayDeferred}
    [] Bug = "cache_vacant_skip_canonicalize"
       /\ c = CacheVoteVacantInsertCanonical ->
      {InputEmptyGuard, ConsensusModeLookup, VoteCacheLookup, VacantEntry,
       CacheInsert, ReplayDeferred}
    [] Bug = "cache_vacant_no_insert"
       /\ c = CacheVoteVacantInsertCanonical ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, VacantEntry, ReplayDeferred}
    [] Bug = "cache_occupied_overwrites_roster"
       /\ c = CacheVoteOccupiedDifferentRosterKeepsCached ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, CacheInsert, RosterMismatchWarn,
       ReplayDeferred}
    [] Bug = "cache_occupied_overwrites_round"
       /\ c = CacheVoteOccupiedSameRosterDifferentRoundKeepsCached ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, CacheInsert, RoundMismatchWarn,
       ReplayDeferred}
    [] Bug = "cache_skip_replay_after_insert"
       /\ c = CacheVoteReplayAfterInsert ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, VacantEntry, CacheInsert}
    [] Bug = "cache_skip_replay_after_occupied"
       /\ c = CacheVoteReplayAfterOccupied ->
      {InputEmptyGuard, ConsensusModeLookup, Canonicalize, Dedup, Sort,
       VoteCacheLookup, OccupiedEntry, KeepExisting}
    [] Bug = "pop_ignore_cache_hit"
       /\ c = PopCacheHitReturnsCached ->
      {PopCacheLookup, BuildPops, PopCacheSizeCheck, PopCacheInsert,
       InsertUnderRosterHash}
    [] Bug = "pop_rebuild_on_cache_hit"
       /\ c = PopCacheHitReturnsCached ->
      {PopCacheLookup, ReturnCachedPops, BuildPops, PopCacheInsert,
       InsertUnderRosterHash}
    [] Bug = "pop_skip_validation_pop"
       /\ c = PopMissUsesValidationPop ->
      {PopCacheLookup, BuildPops, MissingPopOmitted, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_skip_trusted_fallback"
       /\ c = PopMissUsesTrustedFallback ->
      {PopCacheLookup, BuildPops, MissingPopOmitted, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_trusted_overrides_validation"
       /\ c = PopValidationOverridesTrusted ->
      {PopCacheLookup, BuildPops, TrustedPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_include_missing"
       /\ c = PopMissingOmitted ->
      {PopCacheLookup, BuildPops, TrustedPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_include_non_roster"
       /\ c = PopNonRosterOmitted ->
      {PopCacheLookup, BuildPops, NonRosterPopOmitted, TrustedPop,
       PopCacheSizeCheck, PreserveExistingPopCache, PopCacheInsert,
       InsertUnderRosterHash}
    [] Bug = "pop_preserve_duplicate_keys"
       /\ c = PopDuplicateRosterKeyDeduped ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_skip_cache_insert"
       /\ c = PopMissUsesValidationPop ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       PreserveExistingPopCache}
    [] Bug = "pop_clear_below_max"
       /\ c = PopBelowMaxKeepsExistingAndInserts ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       ClearPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_skip_clear_at_max"
       /\ c = PopAtMaxClearsThenInserts ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert, InsertUnderRosterHash}
    [] Bug = "pop_insert_wrong_hash"
       /\ c = PopMissUsesValidationPop ->
      {PopCacheLookup, BuildPops, RosterValidationPop, PopCacheSizeCheck,
       PreserveExistingPopCache, PopCacheInsert}
    [] Bug = "pop_drop_empty_roster"
       /\ c = PopEmptyRosterCachesEmpty ->
      {PopCacheLookup, BuildPops, EmptyPopMap}
    [] Bug = "local_skip_next_live"
       /\ c = LocalCommitNextLiveWins ->
      {NextHeightGuard, LiveRosterLookup, FallbackCommitTopology}
    [] Bug = "local_empty_live_returns_empty"
       /\ c = LocalCommitNextEmptyFallback ->
      {NextHeightGuard, LiveRosterLookup}
    [] Bug = "local_past_uses_live"
       /\ c = LocalCommitPastNoLiveLookup ->
      {LiveRosterLookup, LiveSource}
    [] Bug = "emission_skip_next_live"
       /\ c = EmissionNextLiveWins ->
      {NextHeightGuard, LiveRosterLookup, FallbackTopologyClone}
    [] Bug = "emission_empty_live_returns_empty"
       /\ c = EmissionNextEmptyFallback ->
      {NextHeightGuard, LiveRosterLookup}
    [] Bug = "emission_past_uses_live"
       /\ c = EmissionPastFallbackClone ->
      {LiveRosterLookup, LiveSource}
    [] OTHER -> SpecActions(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

SafetyFast ==
  \A c \in Cases: ActualActions(c) = SpecActions(c)

BugCacheEmptyInserts ==
  ActualActions(CacheVoteEmptyIgnored) = SpecActions(CacheVoteEmptyIgnored)

BugCacheVacantSkipCanonicalize ==
  ActualActions(CacheVoteVacantInsertCanonical) =
    SpecActions(CacheVoteVacantInsertCanonical)

BugCacheVacantNoInsert ==
  ActualActions(CacheVoteVacantInsertCanonical) =
    SpecActions(CacheVoteVacantInsertCanonical)

BugCacheOccupiedOverwritesRoster ==
  ActualActions(CacheVoteOccupiedDifferentRosterKeepsCached) =
    SpecActions(CacheVoteOccupiedDifferentRosterKeepsCached)

BugCacheOccupiedOverwritesRound ==
  ActualActions(CacheVoteOccupiedSameRosterDifferentRoundKeepsCached) =
    SpecActions(CacheVoteOccupiedSameRosterDifferentRoundKeepsCached)

BugCacheSkipReplayAfterInsert ==
  ActualActions(CacheVoteReplayAfterInsert) =
    SpecActions(CacheVoteReplayAfterInsert)

BugCacheSkipReplayAfterOccupied ==
  ActualActions(CacheVoteReplayAfterOccupied) =
    SpecActions(CacheVoteReplayAfterOccupied)

BugPopIgnoreCacheHit ==
  ActualActions(PopCacheHitReturnsCached) = SpecActions(PopCacheHitReturnsCached)

BugPopRebuildOnCacheHit ==
  ActualActions(PopCacheHitReturnsCached) = SpecActions(PopCacheHitReturnsCached)

BugPopSkipValidationPop ==
  ActualActions(PopMissUsesValidationPop) = SpecActions(PopMissUsesValidationPop)

BugPopSkipTrustedFallback ==
  ActualActions(PopMissUsesTrustedFallback) =
    SpecActions(PopMissUsesTrustedFallback)

BugPopTrustedOverridesValidation ==
  ActualActions(PopValidationOverridesTrusted) =
    SpecActions(PopValidationOverridesTrusted)

BugPopIncludeMissing ==
  ActualActions(PopMissingOmitted) = SpecActions(PopMissingOmitted)

BugPopIncludeNonRoster ==
  ActualActions(PopNonRosterOmitted) = SpecActions(PopNonRosterOmitted)

BugPopPreserveDuplicateKeys ==
  ActualActions(PopDuplicateRosterKeyDeduped) =
    SpecActions(PopDuplicateRosterKeyDeduped)

BugPopSkipCacheInsert ==
  ActualActions(PopMissUsesValidationPop) = SpecActions(PopMissUsesValidationPop)

BugPopClearBelowMax ==
  ActualActions(PopBelowMaxKeepsExistingAndInserts) =
    SpecActions(PopBelowMaxKeepsExistingAndInserts)

BugPopSkipClearAtMax ==
  ActualActions(PopAtMaxClearsThenInserts) =
    SpecActions(PopAtMaxClearsThenInserts)

BugPopInsertWrongHash ==
  ActualActions(PopMissUsesValidationPop) = SpecActions(PopMissUsesValidationPop)

BugPopDropEmptyRoster ==
  ActualActions(PopEmptyRosterCachesEmpty) =
    SpecActions(PopEmptyRosterCachesEmpty)

BugLocalSkipNextLive ==
  ActualActions(LocalCommitNextLiveWins) = SpecActions(LocalCommitNextLiveWins)

BugLocalEmptyLiveReturnsEmpty ==
  ActualActions(LocalCommitNextEmptyFallback) =
    SpecActions(LocalCommitNextEmptyFallback)

BugLocalPastUsesLive ==
  ActualActions(LocalCommitPastNoLiveLookup) =
    SpecActions(LocalCommitPastNoLiveLookup)

BugEmissionSkipNextLive ==
  ActualActions(EmissionNextLiveWins) = SpecActions(EmissionNextLiveWins)

BugEmissionEmptyLiveReturnsEmpty ==
  ActualActions(EmissionNextEmptyFallback) =
    SpecActions(EmissionNextEmptyFallback)

BugEmissionPastUsesLive ==
  ActualActions(EmissionPastFallbackClone) = SpecActions(EmissionPastFallbackClone)

====
