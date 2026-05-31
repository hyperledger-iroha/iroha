---- MODULE SumeragiBlockSyncSnapshotRosterGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the commit-roster snapshot selection branch in
`handle_block_sync_update(...)`.

After known-block snapshot hints are filtered, the live path turns a local
commit-roster snapshot into a `BlockSyncRosterSelection` before consulting
persisted sidecars, the block-sync roster cache, or fresh sidecar validation.
The snapshot-derived selection:

* is produced only when the snapshot commit QC has a nonempty validator set;
* uses `CommitRosterJournal` as its source;
* takes the roster, commit QC, and validator checkpoint from the local snapshot;
* includes the local stake snapshot only when it matches the selected roster;
* inserts the selection into the block-sync roster cache only when a cache key
  can be derived from the snapshot hints.

When a snapshot selection exists, it preempts persisted, cached, and freshly
validated fallback roster sources. When it does not exist, the fallback order is
persisted roster, then block-sync roster cache, then fresh sidecar validation.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "snapshot_matching_stake",
  "snapshot_no_stake",
  "snapshot_wrong_stake",
  "snapshot_no_key",
  "snapshot_preempts_persisted",
  "snapshot_preempts_cache",
  "snapshot_preempts_fresh",
  "snapshot_empty_persisted",
  "snapshot_empty_none",
  "no_snapshot_persisted_allowed",
  "no_snapshot_persisted_quarantined",
  "no_snapshot_persisted_and_cache",
  "no_snapshot_cache_hit",
  "no_snapshot_cache_and_fresh",
  "no_snapshot_fresh_qc",
  "no_snapshot_fresh_checkpoint",
  "no_snapshot_fresh_uncertified",
  "no_snapshot_fresh_no_key",
  "no_snapshot_none"
}

HasSnapshot(c) ==
  c \in {
    "snapshot_matching_stake",
    "snapshot_no_stake",
    "snapshot_wrong_stake",
    "snapshot_no_key",
    "snapshot_preempts_persisted",
    "snapshot_preempts_cache",
    "snapshot_preempts_fresh",
    "snapshot_empty_persisted",
    "snapshot_empty_none"
  }

SnapshotRosterNonempty(c) ==
  HasSnapshot(c)
    /\ c \notin {"snapshot_empty_persisted", "snapshot_empty_none"}

SnapshotStakePresent(c) ==
  c \in {
    "snapshot_matching_stake",
    "snapshot_wrong_stake",
    "snapshot_preempts_persisted",
    "snapshot_preempts_cache",
    "snapshot_preempts_fresh"
  }

SnapshotStakeMatchesRoster(c) ==
  c \in {
    "snapshot_matching_stake",
    "snapshot_preempts_persisted",
    "snapshot_preempts_cache",
    "snapshot_preempts_fresh"
  }

SnapshotCacheKey(c) ==
  c # "snapshot_no_key"

PersistedAvailable(c) ==
  c \in {
    "snapshot_preempts_persisted",
    "snapshot_empty_persisted",
    "no_snapshot_persisted_allowed",
    "no_snapshot_persisted_quarantined",
    "no_snapshot_persisted_and_cache"
  }

CacheHit(c) ==
  c \in {
    "snapshot_preempts_cache",
    "no_snapshot_persisted_and_cache",
    "no_snapshot_cache_hit",
    "no_snapshot_cache_and_fresh"
  }

FreshAvailable(c) ==
  c \in {
    "snapshot_preempts_fresh",
    "no_snapshot_cache_and_fresh",
    "no_snapshot_fresh_qc",
    "no_snapshot_fresh_checkpoint",
    "no_snapshot_fresh_uncertified",
    "no_snapshot_fresh_no_key"
  }

FreshCertified(c) ==
  c \in {
    "no_snapshot_cache_and_fresh",
    "no_snapshot_fresh_qc",
    "no_snapshot_fresh_checkpoint",
    "no_snapshot_fresh_no_key"
  }

FallbackCacheKey(c) ==
  c # "no_snapshot_fresh_no_key"

SidecarQuarantined(c) ==
  c = "no_snapshot_persisted_quarantined"

SpecSnapshotSelection(c) ==
  HasSnapshot(c) /\ SnapshotRosterNonempty(c)

SpecSelectedSource(c) ==
  IF SpecSnapshotSelection(c) THEN "CommitRosterJournal"
  ELSE IF PersistedAvailable(c) THEN "Persisted"
  ELSE IF CacheHit(c) /\ FallbackCacheKey(c) THEN "Cache"
  ELSE IF FreshAvailable(c) THEN "Fresh"
  ELSE "none"

SpecSnapshotRosterOrigin(c) ==
  IF SpecSelectedSource(c) = "CommitRosterJournal"
  THEN "snapshot_commit_qc_validator_set"
  ELSE "not_snapshot"

SpecSnapshotCommitQcIncluded(c) ==
  SpecSelectedSource(c) = "CommitRosterJournal"

SpecSnapshotCheckpointIncluded(c) ==
  SpecSelectedSource(c) = "CommitRosterJournal"

SpecSnapshotStakeIncluded(c) ==
  /\ SpecSelectedSource(c) = "CommitRosterJournal"
  /\ SnapshotStakePresent(c)
  /\ SnapshotStakeMatchesRoster(c)

SpecSnapshotCacheInsert(c) ==
  SpecSnapshotSelection(c) /\ SnapshotCacheKey(c)

SpecPersistedLookupCalled(c) ==
  ~SpecSnapshotSelection(c)

SpecAllowSidecarArg(c) ==
  IF ~SpecPersistedLookupCalled(c) THEN "not_called"
  ELSE IF SidecarQuarantined(c) THEN "blocked"
  ELSE "allowed"

SpecCacheLookupCalled(c) ==
  /\ ~SpecSnapshotSelection(c)
  /\ ~PersistedAvailable(c)
  /\ FallbackCacheKey(c)

SpecFreshSelectorCalled(c) ==
  /\ ~SpecSnapshotSelection(c)
  /\ ~PersistedAvailable(c)
  /\ ~(CacheHit(c) /\ FallbackCacheKey(c))

SpecFreshCacheInsert(c) ==
  /\ SpecSelectedSource(c) = "Fresh"
  /\ FallbackCacheKey(c)
  /\ FreshCertified(c)

ActualSelectedSource(c) ==
  IF Bug = "empty_snapshot_roster_selected"
     /\ c \in {"snapshot_empty_persisted", "snapshot_empty_none"}
  THEN "CommitRosterJournal"
  ELSE IF Bug = "snapshot_roster_dropped"
          /\ c = "snapshot_matching_stake" THEN "none"
  ELSE IF Bug = "snapshot_source_not_journal"
          /\ c = "snapshot_matching_stake" THEN "Persisted"
  ELSE IF Bug = "snapshot_does_not_preempt_persisted"
          /\ c = "snapshot_preempts_persisted" THEN "Persisted"
  ELSE IF Bug = "snapshot_does_not_preempt_cache"
          /\ c = "snapshot_preempts_cache" THEN "Cache"
  ELSE IF Bug = "snapshot_does_not_preempt_fresh"
          /\ c = "snapshot_preempts_fresh" THEN "Fresh"
  ELSE IF Bug = "no_snapshot_skips_persisted"
          /\ c = "no_snapshot_persisted_allowed" THEN "none"
  ELSE IF Bug = "cache_before_persisted"
          /\ c = "no_snapshot_persisted_and_cache" THEN "Cache"
  ELSE IF Bug = "fresh_before_cache"
          /\ c = "no_snapshot_cache_and_fresh" THEN "Fresh"
  ELSE IF Bug = "no_selection_returns_snapshot"
          /\ c = "no_snapshot_none" THEN "CommitRosterJournal"
  ELSE SpecSelectedSource(c)

ActualSnapshotRosterOrigin(c) ==
  IF ActualSelectedSource(c) = "CommitRosterJournal" THEN
    IF Bug = "snapshot_roster_from_incoming"
       /\ c = "snapshot_matching_stake"
    THEN "incoming_update"
    ELSE "snapshot_commit_qc_validator_set"
  ELSE "not_snapshot"

ActualSnapshotCommitQcIncluded(c) ==
  IF Bug = "snapshot_commit_qc_dropped"
     /\ c = "snapshot_matching_stake"
  THEN FALSE
  ELSE ActualSelectedSource(c) = "CommitRosterJournal"

ActualSnapshotCheckpointIncluded(c) ==
  IF Bug = "snapshot_checkpoint_dropped"
     /\ c = "snapshot_matching_stake"
  THEN FALSE
  ELSE ActualSelectedSource(c) = "CommitRosterJournal"

ActualSnapshotStakeIncluded(c) ==
  IF Bug = "snapshot_stake_ignored"
     /\ c = "snapshot_matching_stake"
  THEN FALSE
  ELSE IF Bug = "snapshot_stake_wrong_roster_kept"
          /\ c = "snapshot_wrong_stake" THEN TRUE
  ELSE
    /\ ActualSelectedSource(c) = "CommitRosterJournal"
    /\ SnapshotStakePresent(c)
    /\ SnapshotStakeMatchesRoster(c)

ActualSnapshotCacheInsert(c) ==
  IF Bug = "snapshot_cache_not_inserted"
     /\ c = "snapshot_matching_stake"
  THEN FALSE
  ELSE IF Bug = "snapshot_cache_insert_without_key"
          /\ c = "snapshot_no_key" THEN TRUE
  ELSE
    /\ HasSnapshot(c)
    /\ ActualSelectedSource(c) = "CommitRosterJournal"
    /\ SnapshotCacheKey(c)

ActualPersistedLookupCalled(c) ==
  IF Bug = "snapshot_persisted_lookup_called"
     /\ c = "snapshot_matching_stake"
  THEN TRUE
  ELSE ActualSelectedSource(c) # "CommitRosterJournal"

ActualAllowSidecarArg(c) ==
  IF ~ActualPersistedLookupCalled(c) THEN "not_called"
  ELSE IF Bug = "sidecar_quarantine_ignored"
          /\ c = "no_snapshot_persisted_quarantined" THEN "allowed"
  ELSE IF SidecarQuarantined(c) THEN "blocked"
  ELSE "allowed"

ActualCacheLookupCalled(c) ==
  IF Bug = "cache_before_persisted"
     /\ c = "no_snapshot_persisted_and_cache"
  THEN TRUE
  ELSE
    /\ ActualPersistedLookupCalled(c)
    /\ ~PersistedAvailable(c)
    /\ FallbackCacheKey(c)

ActualFreshSelectorCalled(c) ==
  IF Bug = "fresh_before_cache"
     /\ c = "no_snapshot_cache_and_fresh"
  THEN TRUE
  ELSE
    /\ ActualPersistedLookupCalled(c)
    /\ ~PersistedAvailable(c)
    /\ ~(CacheHit(c) /\ FallbackCacheKey(c))

ActualFreshCacheInsert(c) ==
  IF Bug = "fresh_cert_not_cached"
     /\ c = "no_snapshot_fresh_qc"
  THEN FALSE
  ELSE IF Bug = "fresh_uncertified_cached"
          /\ c = "no_snapshot_fresh_uncertified" THEN TRUE
  ELSE IF Bug = "fresh_cache_without_key"
          /\ c = "no_snapshot_fresh_no_key" THEN TRUE
  ELSE
    /\ ActualSelectedSource(c) = "Fresh"
    /\ FallbackCacheKey(c)
    /\ FreshCertified(c)

Matches(c) ==
  /\ ActualSelectedSource(c) = SpecSelectedSource(c)
  /\ ActualSnapshotRosterOrigin(c) = SpecSnapshotRosterOrigin(c)
  /\ ActualSnapshotCommitQcIncluded(c) = SpecSnapshotCommitQcIncluded(c)
  /\ ActualSnapshotCheckpointIncluded(c) = SpecSnapshotCheckpointIncluded(c)
  /\ ActualSnapshotStakeIncluded(c) = SpecSnapshotStakeIncluded(c)
  /\ ActualSnapshotCacheInsert(c) = SpecSnapshotCacheInsert(c)
  /\ ActualPersistedLookupCalled(c) = SpecPersistedLookupCalled(c)
  /\ ActualAllowSidecarArg(c) = SpecAllowSidecarArg(c)
  /\ ActualCacheLookupCalled(c) = SpecCacheLookupCalled(c)
  /\ ActualFreshSelectorCalled(c) = SpecFreshSelectorCalled(c)
  /\ ActualFreshCacheInsert(c) = SpecFreshCacheInsert(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_snapshot_roster_selected",
       "snapshot_roster_dropped",
       "snapshot_source_not_journal",
       "snapshot_roster_from_incoming",
       "snapshot_commit_qc_dropped",
       "snapshot_checkpoint_dropped",
       "snapshot_stake_ignored",
       "snapshot_stake_wrong_roster_kept",
       "snapshot_cache_not_inserted",
       "snapshot_cache_insert_without_key",
       "snapshot_persisted_lookup_called",
       "snapshot_does_not_preempt_persisted",
       "snapshot_does_not_preempt_cache",
       "snapshot_does_not_preempt_fresh",
       "no_snapshot_skips_persisted",
       "cache_before_persisted",
       "fresh_before_cache",
       "fresh_cert_not_cached",
       "fresh_uncertified_cached",
       "fresh_cache_without_key",
       "no_selection_returns_snapshot",
       "sidecar_quarantine_ignored"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

SnapshotSelectionUsesJournal ==
  Matches("snapshot_matching_stake")

SnapshotSelectionOmitsAbsentStake ==
  Matches("snapshot_no_stake")

SnapshotSelectionDropsWrongStake ==
  Matches("snapshot_wrong_stake")

SnapshotSelectionCacheKeyGate ==
  Matches("snapshot_no_key")

SnapshotPreemptsPersisted ==
  Matches("snapshot_preempts_persisted")

SnapshotPreemptsCache ==
  Matches("snapshot_preempts_cache")

SnapshotPreemptsFresh ==
  Matches("snapshot_preempts_fresh")

EmptySnapshotRosterFallsBack ==
  Matches("snapshot_empty_persisted") /\ Matches("snapshot_empty_none")

PersistedPrecedesCacheAndFresh ==
  Matches("no_snapshot_persisted_allowed")
    /\ Matches("no_snapshot_persisted_quarantined")
    /\ Matches("no_snapshot_persisted_and_cache")

CachePrecedesFresh ==
  Matches("no_snapshot_cache_hit") /\ Matches("no_snapshot_cache_and_fresh")

FreshSelectionCacheGate ==
  Matches("no_snapshot_fresh_qc")
    /\ Matches("no_snapshot_fresh_checkpoint")
    /\ Matches("no_snapshot_fresh_uncertified")
    /\ Matches("no_snapshot_fresh_no_key")

NoRosterSourceStaysNone ==
  Matches("no_snapshot_none")

=============================================================================
