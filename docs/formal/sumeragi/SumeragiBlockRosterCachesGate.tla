---- MODULE SumeragiBlockRosterCachesGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the block roster cache helpers.

This slice captures `BlockSyncRosterCacheKey::from_hints(...)`,
`BlockSignerCacheKey::new(...)`, `BlockSignerCache`, and
`BlockSyncRosterSelectionCache` from `main_loop.rs`. Hashes, block identities,
rosters, and cache values are collapsed into representative actions while
preserving observable contracts: roster-selection cache keys require at least
one roster artifact and, in NPoS mode, a stake snapshot; Permissioned keys may
omit the stake snapshot; keys retain block identity, consensus mode, artifact
validator-set hashes, and stake-snapshot hash while intentionally ignoring the
block view. Signer cache keys reject empty rosters, hash the mode-canonical
roster, and include the PRF seed. Both caches start empty, clear entries and
recency order together, return cloned hits while touching recency, leave misses
untouched, make zero-capacity inserts no-ops, update existing values without
duplicate recency entries, evict oldest live keys while skipping stale order
entries, and the signer cache removes all entries/order items for a completed
block without disturbing other blocks.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

RosterKeyRejectsNoArtifacts == 1
RosterKeyRejectsNposMissingStake == 2
RosterKeyAllowsPermissionedMissingStake == 3
RosterKeyIncludesBlockHash == 4
RosterKeyIncludesHeight == 5
RosterKeyIncludesMode == 6
RosterKeyUsesValidatorSetHashes == 7
RosterKeyIncludesStakeHash == 8
RosterKeyIgnoresBlockView == 9
SignerKeyRejectsEmptyRoster == 10
SignerKeyCanonicalizesRoster == 11
SignerKeyIncludesModeAndSeed == 12
SignerCacheNewEmptyCapacity == 13
SignerCacheClearClearsBoth == 14
SignerCacheGetMissNoTouch == 15
SignerCacheGetHitTouches == 16
SignerCacheInsertZeroNoop == 17
SignerCacheInsertUpdatesDedups == 18
SignerCacheEvictsOldest == 19
SignerCacheEvictSkipsStaleOrder == 20
SignerCacheRemoveBlockClearsMatching == 21
RosterCacheNewEmptyCapacity == 22
RosterCacheClearClearsBoth == 23
RosterCacheGetMissNoTouch == 24
RosterCacheGetHitTouches == 25
RosterCacheInsertZeroNoop == 26
RosterCacheInsertUpdatesDedups == 27
RosterCacheEvictsOldest == 28
RosterCacheEvictSkipsStaleOrder == 29

Candidates == 1..29

RosterCacheKeyCases == {
  RosterKeyRejectsNoArtifacts,
  RosterKeyRejectsNposMissingStake,
  RosterKeyAllowsPermissionedMissingStake,
  RosterKeyIncludesBlockHash,
  RosterKeyIncludesHeight,
  RosterKeyIncludesMode,
  RosterKeyUsesValidatorSetHashes,
  RosterKeyIncludesStakeHash,
  RosterKeyIgnoresBlockView
}

SignerCacheKeyCases == {
  SignerKeyRejectsEmptyRoster,
  SignerKeyCanonicalizesRoster,
  SignerKeyIncludesModeAndSeed
}

SignerCacheLifecycleCases == {
  SignerCacheNewEmptyCapacity,
  SignerCacheClearClearsBoth
}

SignerCacheLookupCases == {
  SignerCacheGetMissNoTouch,
  SignerCacheGetHitTouches
}

SignerCacheInsertEvictCases == {
  SignerCacheInsertZeroNoop,
  SignerCacheInsertUpdatesDedups,
  SignerCacheEvictsOldest,
  SignerCacheEvictSkipsStaleOrder
}

SignerCacheRemovalCases == {
  SignerCacheRemoveBlockClearsMatching
}

RosterSelectionCacheLifecycleCases == {
  RosterCacheNewEmptyCapacity,
  RosterCacheClearClearsBoth
}

RosterSelectionCacheLookupCases == {
  RosterCacheGetMissNoTouch,
  RosterCacheGetHitTouches
}

RosterSelectionCacheInsertEvictCases == {
  RosterCacheInsertZeroNoop,
  RosterCacheInsertUpdatesDedups,
  RosterCacheEvictsOldest,
  RosterCacheEvictSkipsStaleOrder
}

RejectKey == 1
ReturnKey == 2
HasBlockHash == 3
HasHeight == 4
HasMode == 5
HasCertValidatorSetHash == 6
HasCheckpointValidatorSetHash == 7
HasStakeSnapshotHash == 8
ViewIgnored == 9
RejectEmptyRoster == 10
HashCanonicalRoster == 11
HasPrfSeed == 12
EntriesEmpty == 13
OrderEmpty == 14
CapacityInput == 15
ClearEntries == 16
ClearOrder == 17
ReturnNone == 18
ReturnValue == 19
PreserveOrder == 20
TouchKey == 21
DedupOrder == 22
MoveKeyToBack == 23
NoInsert == 24
InsertEntry == 25
UpdateEntry == 26
UpdatedValue == 27
PreserveOldValue == 28
EvictOldest == 29
EvictNewest == 30
CapacityBound == 31
DropStaleOrder == 32
ContinueEvict == 33
RemoveMatchingBlockEntries == 34
RemoveMatchingBlockOrder == 35
PreserveOtherBlocks == 36

Actions == 1..36

SpecActions(candidate) ==
  CASE candidate = RosterKeyRejectsNoArtifacts ->
      {RejectKey}
    [] candidate = RosterKeyRejectsNposMissingStake ->
      {RejectKey}
    [] candidate = RosterKeyAllowsPermissionedMissingStake ->
      {ReturnKey, HasMode}
    [] candidate = RosterKeyIncludesBlockHash ->
      {ReturnKey, HasBlockHash}
    [] candidate = RosterKeyIncludesHeight ->
      {ReturnKey, HasHeight}
    [] candidate = RosterKeyIncludesMode ->
      {ReturnKey, HasMode}
    [] candidate = RosterKeyUsesValidatorSetHashes ->
      {ReturnKey, HasCertValidatorSetHash, HasCheckpointValidatorSetHash}
    [] candidate = RosterKeyIncludesStakeHash ->
      {ReturnKey, HasStakeSnapshotHash}
    [] candidate = RosterKeyIgnoresBlockView ->
      {ReturnKey, ViewIgnored}
    [] candidate = SignerKeyRejectsEmptyRoster ->
      {RejectKey, RejectEmptyRoster}
    [] candidate = SignerKeyCanonicalizesRoster ->
      {ReturnKey, HashCanonicalRoster}
    [] candidate = SignerKeyIncludesModeAndSeed ->
      {ReturnKey, HasMode, HasPrfSeed}
    [] candidate = SignerCacheNewEmptyCapacity ->
      {EntriesEmpty, OrderEmpty, CapacityInput}
    [] candidate = SignerCacheClearClearsBoth ->
      {ClearEntries, ClearOrder}
    [] candidate = SignerCacheGetMissNoTouch ->
      {ReturnNone, PreserveOrder}
    [] candidate = SignerCacheGetHitTouches ->
      {ReturnValue, TouchKey, DedupOrder, MoveKeyToBack}
    [] candidate = SignerCacheInsertZeroNoop ->
      {NoInsert, PreserveOrder}
    [] candidate = SignerCacheInsertUpdatesDedups ->
      {UpdateEntry, UpdatedValue, TouchKey, DedupOrder, MoveKeyToBack}
    [] candidate = SignerCacheEvictsOldest ->
      {InsertEntry, TouchKey, MoveKeyToBack, EvictOldest, CapacityBound}
    [] candidate = SignerCacheEvictSkipsStaleOrder ->
      {DropStaleOrder, ContinueEvict, EvictOldest, CapacityBound}
    [] candidate = SignerCacheRemoveBlockClearsMatching ->
      {RemoveMatchingBlockEntries, RemoveMatchingBlockOrder, PreserveOtherBlocks}
    [] candidate = RosterCacheNewEmptyCapacity ->
      {EntriesEmpty, OrderEmpty, CapacityInput}
    [] candidate = RosterCacheClearClearsBoth ->
      {ClearEntries, ClearOrder}
    [] candidate = RosterCacheGetMissNoTouch ->
      {ReturnNone, PreserveOrder}
    [] candidate = RosterCacheGetHitTouches ->
      {ReturnValue, TouchKey, DedupOrder, MoveKeyToBack}
    [] candidate = RosterCacheInsertZeroNoop ->
      {NoInsert, PreserveOrder}
    [] candidate = RosterCacheInsertUpdatesDedups ->
      {UpdateEntry, UpdatedValue, TouchKey, DedupOrder, MoveKeyToBack}
    [] candidate = RosterCacheEvictsOldest ->
      {InsertEntry, TouchKey, MoveKeyToBack, EvictOldest, CapacityBound}
    [] candidate = RosterCacheEvictSkipsStaleOrder ->
      {DropStaleOrder, ContinueEvict, EvictOldest, CapacityBound}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = RosterKeyRejectsNoArtifacts /\
          Bug = "roster_key_accepts_no_artifacts" ->
      (spec \ {RejectKey}) \cup {ReturnKey}
    [] candidate = RosterKeyRejectsNposMissingStake /\
          Bug = "roster_key_accepts_npos_missing_stake" ->
      (spec \ {RejectKey}) \cup {ReturnKey}
    [] candidate = RosterKeyAllowsPermissionedMissingStake /\
          Bug = "roster_key_rejects_permissioned_missing_stake" ->
      (spec \ {ReturnKey}) \cup {RejectKey}
    [] candidate = RosterKeyIncludesBlockHash /\
          Bug = "roster_key_drops_block_hash" ->
      spec \ {HasBlockHash}
    [] candidate = RosterKeyIncludesHeight /\
          Bug = "roster_key_drops_height" ->
      spec \ {HasHeight}
    [] candidate = RosterKeyIncludesMode /\
          Bug = "roster_key_drops_mode" ->
      spec \ {HasMode}
    [] candidate = RosterKeyUsesValidatorSetHashes /\
          Bug = "roster_key_uses_full_artifact_hash" ->
      spec \ {HasCertValidatorSetHash, HasCheckpointValidatorSetHash}
    [] candidate = RosterKeyIncludesStakeHash /\
          Bug = "roster_key_drops_stake_hash" ->
      spec \ {HasStakeSnapshotHash}
    [] candidate = RosterKeyIgnoresBlockView /\
          Bug = "roster_key_includes_view" ->
      spec \ {ViewIgnored}
    [] candidate = SignerKeyRejectsEmptyRoster /\
          Bug = "signer_key_accepts_empty_roster" ->
      (spec \ {RejectKey, RejectEmptyRoster}) \cup {ReturnKey}
    [] candidate = SignerKeyCanonicalizesRoster /\
          Bug = "signer_key_skips_canonicalization" ->
      spec \ {HashCanonicalRoster}
    [] candidate = SignerKeyIncludesModeAndSeed /\
          Bug = "signer_key_drops_seed" ->
      spec \ {HasPrfSeed}
    [] candidate = SignerCacheNewEmptyCapacity /\
          Bug = "signer_cache_new_not_empty" ->
      (spec \ {EntriesEmpty, OrderEmpty}) \cup {InsertEntry}
    [] candidate = SignerCacheClearClearsBoth /\
          Bug = "signer_cache_clear_keeps_order" ->
      spec \ {ClearOrder}
    [] candidate = SignerCacheGetMissNoTouch /\
          Bug = "signer_cache_get_miss_touches" ->
      (spec \ {PreserveOrder}) \cup {TouchKey}
    [] candidate = SignerCacheGetHitTouches /\
          Bug = "signer_cache_get_hit_skips_touch" ->
      (spec \ {TouchKey, DedupOrder, MoveKeyToBack}) \cup {PreserveOrder}
    [] candidate = SignerCacheInsertZeroNoop /\
          Bug = "signer_cache_insert_zero_inserts" ->
      (spec \ {NoInsert, PreserveOrder}) \cup {InsertEntry, TouchKey}
    [] candidate = SignerCacheInsertUpdatesDedups /\
          Bug = "signer_cache_update_duplicates_order" ->
      spec \ {DedupOrder}
    [] candidate = SignerCacheEvictsOldest /\
          Bug = "signer_cache_evict_drops_newest" ->
      (spec \ {EvictOldest}) \cup {EvictNewest}
    [] candidate = SignerCacheEvictSkipsStaleOrder /\
          Bug = "signer_cache_evict_stops_on_stale_order" ->
      (spec \ {ContinueEvict, EvictOldest, CapacityBound}) \cup {NoInsert}
    [] candidate = SignerCacheRemoveBlockClearsMatching /\
          Bug = "signer_cache_remove_block_keeps_entry" ->
      spec \ {RemoveMatchingBlockEntries}
    [] candidate = SignerCacheRemoveBlockClearsMatching /\
          Bug = "signer_cache_remove_block_keeps_order" ->
      spec \ {RemoveMatchingBlockOrder}
    [] candidate = RosterCacheNewEmptyCapacity /\
          Bug = "roster_cache_new_not_empty" ->
      (spec \ {EntriesEmpty, OrderEmpty}) \cup {InsertEntry}
    [] candidate = RosterCacheClearClearsBoth /\
          Bug = "roster_cache_clear_keeps_entries" ->
      spec \ {ClearEntries}
    [] candidate = RosterCacheGetMissNoTouch /\
          Bug = "roster_cache_get_miss_touches" ->
      (spec \ {PreserveOrder}) \cup {TouchKey}
    [] candidate = RosterCacheGetHitTouches /\
          Bug = "roster_cache_get_hit_skips_touch" ->
      (spec \ {TouchKey, DedupOrder, MoveKeyToBack}) \cup {PreserveOrder}
    [] candidate = RosterCacheInsertZeroNoop /\
          Bug = "roster_cache_insert_zero_inserts" ->
      (spec \ {NoInsert, PreserveOrder}) \cup {InsertEntry, TouchKey}
    [] candidate = RosterCacheInsertUpdatesDedups /\
          Bug = "roster_cache_update_keeps_old_value" ->
      (spec \ {UpdatedValue}) \cup {PreserveOldValue}
    [] candidate = RosterCacheEvictsOldest /\
          Bug = "roster_cache_evict_over_capacity" ->
      spec \ {CapacityBound}
    [] candidate = RosterCacheEvictSkipsStaleOrder /\
          Bug = "roster_cache_evict_stops_on_stale_order" ->
      (spec \ {ContinueEvict, EvictOldest, CapacityBound}) \cup {NoInsert}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "roster_key_accepts_no_artifacts",
       "roster_key_accepts_npos_missing_stake",
       "roster_key_rejects_permissioned_missing_stake",
       "roster_key_drops_block_hash",
       "roster_key_drops_height",
       "roster_key_drops_mode",
       "roster_key_uses_full_artifact_hash",
       "roster_key_drops_stake_hash",
       "roster_key_includes_view",
       "signer_key_accepts_empty_roster",
       "signer_key_skips_canonicalization",
       "signer_key_drops_seed",
       "signer_cache_new_not_empty",
       "signer_cache_clear_keeps_order",
       "signer_cache_get_miss_touches",
       "signer_cache_get_hit_skips_touch",
       "signer_cache_insert_zero_inserts",
       "signer_cache_update_duplicates_order",
       "signer_cache_evict_drops_newest",
       "signer_cache_evict_stops_on_stale_order",
       "signer_cache_remove_block_keeps_entry",
       "signer_cache_remove_block_keeps_order",
       "roster_cache_new_not_empty",
       "roster_cache_clear_keeps_entries",
       "roster_cache_get_miss_touches",
       "roster_cache_get_hit_skips_touch",
       "roster_cache_insert_zero_inserts",
       "roster_cache_update_keeps_old_value",
       "roster_cache_evict_over_capacity",
       "roster_cache_evict_stops_on_stale_order"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheRosterKeyExact ==
  \A c \in RosterCacheKeyCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSignerKeyExact ==
  \A c \in SignerCacheKeyCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSignerLifecycleExact ==
  \A c \in SignerCacheLifecycleCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSignerLookupExact ==
  \A c \in SignerCacheLookupCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSignerInsertEvictExact ==
  \A c \in SignerCacheInsertEvictCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSignerRemovalExact ==
  \A c \in SignerCacheRemovalCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSelectionLifecycleExact ==
  \A c \in RosterSelectionCacheLifecycleCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSelectionLookupExact ==
  \A c \in RosterSelectionCacheLookupCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheSelectionInsertEvictExact ==
  \A c \in RosterSelectionCacheInsertEvictCases:
    ImplementationActions(c) = SpecActions(c)

BlockRosterCacheExactness ==
  /\ BlockRosterCacheRosterKeyExact
  /\ BlockRosterCacheSignerKeyExact
  /\ BlockRosterCacheSignerLifecycleExact
  /\ BlockRosterCacheSignerLookupExact
  /\ BlockRosterCacheSignerInsertEvictExact
  /\ BlockRosterCacheSignerRemovalExact
  /\ BlockRosterCacheSelectionLifecycleExact
  /\ BlockRosterCacheSelectionLookupExact
  /\ BlockRosterCacheSelectionInsertEvictExact

BugRosterKeyAcceptsNoArtifacts ==
  ImplementationActions(RosterKeyRejectsNoArtifacts) =
    SpecActions(RosterKeyRejectsNoArtifacts)

BugRosterKeyAcceptsNposMissingStake ==
  ImplementationActions(RosterKeyRejectsNposMissingStake) =
    SpecActions(RosterKeyRejectsNposMissingStake)

BugRosterKeyRejectsPermissionedMissingStake ==
  ImplementationActions(RosterKeyAllowsPermissionedMissingStake) =
    SpecActions(RosterKeyAllowsPermissionedMissingStake)

BugRosterKeyDropsBlockHash ==
  ImplementationActions(RosterKeyIncludesBlockHash) =
    SpecActions(RosterKeyIncludesBlockHash)

BugRosterKeyDropsHeight ==
  ImplementationActions(RosterKeyIncludesHeight) =
    SpecActions(RosterKeyIncludesHeight)

BugRosterKeyDropsMode ==
  ImplementationActions(RosterKeyIncludesMode) =
    SpecActions(RosterKeyIncludesMode)

BugRosterKeyUsesFullArtifactHash ==
  ImplementationActions(RosterKeyUsesValidatorSetHashes) =
    SpecActions(RosterKeyUsesValidatorSetHashes)

BugRosterKeyDropsStakeHash ==
  ImplementationActions(RosterKeyIncludesStakeHash) =
    SpecActions(RosterKeyIncludesStakeHash)

BugRosterKeyIncludesView ==
  ImplementationActions(RosterKeyIgnoresBlockView) =
    SpecActions(RosterKeyIgnoresBlockView)

BugSignerKeyAcceptsEmptyRoster ==
  ImplementationActions(SignerKeyRejectsEmptyRoster) =
    SpecActions(SignerKeyRejectsEmptyRoster)

BugSignerKeySkipsCanonicalization ==
  ImplementationActions(SignerKeyCanonicalizesRoster) =
    SpecActions(SignerKeyCanonicalizesRoster)

BugSignerKeyDropsSeed ==
  ImplementationActions(SignerKeyIncludesModeAndSeed) =
    SpecActions(SignerKeyIncludesModeAndSeed)

BugSignerCacheNewNotEmpty ==
  ImplementationActions(SignerCacheNewEmptyCapacity) =
    SpecActions(SignerCacheNewEmptyCapacity)

BugSignerCacheClearKeepsOrder ==
  ImplementationActions(SignerCacheClearClearsBoth) =
    SpecActions(SignerCacheClearClearsBoth)

BugSignerCacheGetMissTouches ==
  ImplementationActions(SignerCacheGetMissNoTouch) =
    SpecActions(SignerCacheGetMissNoTouch)

BugSignerCacheGetHitSkipsTouch ==
  ImplementationActions(SignerCacheGetHitTouches) =
    SpecActions(SignerCacheGetHitTouches)

BugSignerCacheInsertZeroInserts ==
  ImplementationActions(SignerCacheInsertZeroNoop) =
    SpecActions(SignerCacheInsertZeroNoop)

BugSignerCacheUpdateDuplicatesOrder ==
  ImplementationActions(SignerCacheInsertUpdatesDedups) =
    SpecActions(SignerCacheInsertUpdatesDedups)

BugSignerCacheEvictDropsNewest ==
  ImplementationActions(SignerCacheEvictsOldest) =
    SpecActions(SignerCacheEvictsOldest)

BugSignerCacheEvictStopsOnStaleOrder ==
  ImplementationActions(SignerCacheEvictSkipsStaleOrder) =
    SpecActions(SignerCacheEvictSkipsStaleOrder)

BugSignerCacheRemoveBlockKeepsEntry ==
  ImplementationActions(SignerCacheRemoveBlockClearsMatching) =
    SpecActions(SignerCacheRemoveBlockClearsMatching)

BugSignerCacheRemoveBlockKeepsOrder ==
  ImplementationActions(SignerCacheRemoveBlockClearsMatching) =
    SpecActions(SignerCacheRemoveBlockClearsMatching)

BugRosterCacheNewNotEmpty ==
  ImplementationActions(RosterCacheNewEmptyCapacity) =
    SpecActions(RosterCacheNewEmptyCapacity)

BugRosterCacheClearKeepsEntries ==
  ImplementationActions(RosterCacheClearClearsBoth) =
    SpecActions(RosterCacheClearClearsBoth)

BugRosterCacheGetMissTouches ==
  ImplementationActions(RosterCacheGetMissNoTouch) =
    SpecActions(RosterCacheGetMissNoTouch)

BugRosterCacheGetHitSkipsTouch ==
  ImplementationActions(RosterCacheGetHitTouches) =
    SpecActions(RosterCacheGetHitTouches)

BugRosterCacheInsertZeroInserts ==
  ImplementationActions(RosterCacheInsertZeroNoop) =
    SpecActions(RosterCacheInsertZeroNoop)

BugRosterCacheUpdateKeepsOldValue ==
  ImplementationActions(RosterCacheInsertUpdatesDedups) =
    SpecActions(RosterCacheInsertUpdatesDedups)

BugRosterCacheEvictOverCapacity ==
  ImplementationActions(RosterCacheEvictsOldest) =
    SpecActions(RosterCacheEvictsOldest)

BugRosterCacheEvictStopsOnStaleOrder ==
  ImplementationActions(RosterCacheEvictSkipsStaleOrder) =
    SpecActions(RosterCacheEvictSkipsStaleOrder)

====
