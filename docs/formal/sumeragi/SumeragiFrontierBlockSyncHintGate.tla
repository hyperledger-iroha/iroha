---- MODULE SumeragiFrontierBlockSyncHintGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for `FrontierBlockSyncHint`.

This slice captures the externally visible contract around
`should_pause_latest_gossip(...)`, `sync_external_hints(...)`,
`record_direct_block_sync_response_permit(...)`,
`allow_direct_block_sync_response(...)`, and
`prune_direct_block_sync_response_permits(...)`. The model abstracts peers and
`Instant` values to finite cases while pinning the safety-critical behavior:
startup or frontier pressure pauses proactive latest-block gossip; actor sync
stores both frontier-pressure flags; direct block-sync responses require a
fresh peer-scoped permit; permits are consumed exactly once unless multiple
pending responses are authorized; the TTL boundary is inclusive; stale and
zero-pending permits are removed; and pruning drops only expired permits.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

DefaultReady == "default_ready"
StartupUninitialized == "startup_uninitialized"
PressureActive == "pressure_active"
LaneActive == "lane_active"
BothActive == "both_active"
SyncPressureOnly == "sync_pressure_only"
SyncLaneOnly == "sync_lane_only"
SyncClearStale == "sync_clear_stale"
SyncBoth == "sync_both"
AllowAbsentPeer == "allow_absent_peer"
RecordNewPermit == "record_new_permit"
RecordExistingIncrements == "record_existing_increments"
RecordExistingSaturates == "record_existing_saturates"
RecordPrunesExpired == "record_prunes_expired"
AllowFreshSingle == "allow_fresh_single"
AllowFreshMulti == "allow_fresh_multi"
AllowExactTtl == "allow_exact_ttl"
AllowExpired == "allow_expired"
AllowZeroPending == "allow_zero_pending"
AllowWrongPeer == "allow_wrong_peer"
PruneExpired == "prune_expired"
PruneFresh == "prune_fresh"

Cases == {
  DefaultReady,
  StartupUninitialized,
  PressureActive,
  LaneActive,
  BothActive,
  SyncPressureOnly,
  SyncLaneOnly,
  SyncClearStale,
  SyncBoth,
  AllowAbsentPeer,
  RecordNewPermit,
  RecordExistingIncrements,
  RecordExistingSaturates,
  RecordPrunesExpired,
  AllowFreshSingle,
  AllowFreshMulti,
  AllowExactTtl,
  AllowExpired,
  AllowZeroPending,
  AllowWrongPeer,
  PruneExpired,
  PruneFresh
}

Pause == 1
NoPause == 2
SyncPressureTrue == 3
SyncPressureFalse == 4
SyncLaneTrue == 5
SyncLaneFalse == 6
NoPermitReject == 7
RecordCreatesPermit == 8
RecordUpdatesLastRequest == 9
RecordIncrementsPending == 10
RecordCapsPending == 11
AllowFreshReturnsTrue == 12
AllowConsumesOne == 13
AllowRemovesWhenZero == 14
AllowRetainsPending == 15
AllowAtTtl == 16
ExpiredRejects == 17
ExpiredRemoves == 18
ZeroPendingRejects == 19
ZeroPendingRemoves == 20
WrongPeerRejects == 21
WrongPeerPreservesOwner == 22
PruneExpiredRemoves == 23
PruneFreshKeeps == 24

ActionUniverse == 1..24

SpecActions(c) ==
  CASE c = DefaultReady -> {NoPause}
    [] c = StartupUninitialized -> {Pause}
    [] c = PressureActive -> {Pause}
    [] c = LaneActive -> {Pause}
    [] c = BothActive -> {Pause}
    [] c = SyncPressureOnly -> {SyncPressureTrue, SyncLaneFalse}
    [] c = SyncLaneOnly -> {SyncPressureFalse, SyncLaneTrue}
    [] c = SyncClearStale -> {SyncPressureFalse, SyncLaneFalse}
    [] c = SyncBoth -> {SyncPressureTrue, SyncLaneTrue}
    [] c = AllowAbsentPeer -> {NoPermitReject}
    [] c = RecordNewPermit ->
      {RecordCreatesPermit, RecordUpdatesLastRequest, RecordIncrementsPending}
    [] c = RecordExistingIncrements ->
      {RecordUpdatesLastRequest, RecordIncrementsPending}
    [] c = RecordExistingSaturates ->
      {RecordUpdatesLastRequest, RecordCapsPending}
    [] c = RecordPrunesExpired ->
      {PruneExpiredRemoves, RecordCreatesPermit, RecordUpdatesLastRequest,
       RecordIncrementsPending}
    [] c = AllowFreshSingle ->
      {AllowFreshReturnsTrue, AllowConsumesOne, AllowRemovesWhenZero}
    [] c = AllowFreshMulti ->
      {AllowFreshReturnsTrue, AllowConsumesOne, AllowRetainsPending}
    [] c = AllowExactTtl ->
      {AllowFreshReturnsTrue, AllowConsumesOne, AllowAtTtl,
       AllowRemovesWhenZero}
    [] c = AllowExpired -> {ExpiredRejects, ExpiredRemoves}
    [] c = AllowZeroPending -> {ZeroPendingRejects, ZeroPendingRemoves}
    [] c = AllowWrongPeer -> {WrongPeerRejects, WrongPeerPreservesOwner}
    [] c = PruneExpired -> {PruneExpiredRemoves}
    [] c = PruneFresh -> {PruneFreshKeeps}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "default_pauses" /\ c = DefaultReady ->
      (spec \ {NoPause}) \cup {Pause}
    [] Bug = "startup_not_paused" /\ c = StartupUninitialized ->
      (spec \ {Pause}) \cup {NoPause}
    [] Bug = "pressure_not_paused" /\ c = PressureActive ->
      (spec \ {Pause}) \cup {NoPause}
    [] Bug = "lane_not_paused" /\ c = LaneActive ->
      (spec \ {Pause}) \cup {NoPause}
    [] Bug = "sync_skips_pressure_true"
       /\ c \in {SyncPressureOnly, SyncBoth} ->
      (spec \ {SyncPressureTrue}) \cup {SyncPressureFalse}
    [] Bug = "sync_skips_pressure_false" /\ c = SyncClearStale ->
      (spec \ {SyncPressureFalse}) \cup {SyncPressureTrue}
    [] Bug = "sync_skips_lane_true"
       /\ c \in {SyncLaneOnly, SyncBoth} ->
      (spec \ {SyncLaneTrue}) \cup {SyncLaneFalse}
    [] Bug = "sync_skips_lane_false" /\ c = SyncClearStale ->
      (spec \ {SyncLaneFalse}) \cup {SyncLaneTrue}
    [] Bug = "allow_absent_peer" /\ c = AllowAbsentPeer ->
      (spec \ {NoPermitReject}) \cup {AllowFreshReturnsTrue}
    [] Bug = "record_skips_create"
       /\ c \in {RecordNewPermit, RecordPrunesExpired} ->
      spec \ {RecordCreatesPermit}
    [] Bug = "record_skips_last_request"
       /\ c \in {RecordNewPermit, RecordExistingIncrements,
                 RecordExistingSaturates, RecordPrunesExpired} ->
      spec \ {RecordUpdatesLastRequest}
    [] Bug = "record_skips_increment"
       /\ c \in {RecordNewPermit, RecordExistingIncrements,
                 RecordPrunesExpired} ->
      spec \ {RecordIncrementsPending}
    [] Bug = "record_overflows_pending" /\ c = RecordExistingSaturates ->
      (spec \ {RecordCapsPending}) \cup {RecordIncrementsPending}
    [] Bug = "record_skips_prune" /\ c = RecordPrunesExpired ->
      spec \ {PruneExpiredRemoves}
    [] Bug = "allow_fresh_rejects" /\ c = AllowFreshSingle ->
      (spec \ {AllowFreshReturnsTrue}) \cup {NoPermitReject}
    [] Bug = "allow_fresh_not_consumed"
       /\ c \in {AllowFreshSingle, AllowFreshMulti, AllowExactTtl} ->
      spec \ {AllowConsumesOne}
    [] Bug = "allow_single_not_removed"
       /\ c \in {AllowFreshSingle, AllowExactTtl} ->
      (spec \ {AllowRemovesWhenZero}) \cup {AllowRetainsPending}
    [] Bug = "allow_multi_removed" /\ c = AllowFreshMulti ->
      (spec \ {AllowRetainsPending}) \cup {AllowRemovesWhenZero}
    [] Bug = "ttl_boundary_rejected" /\ c = AllowExactTtl ->
      (spec \ {AllowFreshReturnsTrue, AllowAtTtl}) \cup {ExpiredRejects}
    [] Bug = "expired_allowed" /\ c = AllowExpired ->
      (spec \ {ExpiredRejects}) \cup {AllowFreshReturnsTrue}
    [] Bug = "expired_kept" /\ c = AllowExpired ->
      spec \ {ExpiredRemoves}
    [] Bug = "zero_pending_allowed" /\ c = AllowZeroPending ->
      (spec \ {ZeroPendingRejects}) \cup {AllowFreshReturnsTrue}
    [] Bug = "zero_pending_kept" /\ c = AllowZeroPending ->
      spec \ {ZeroPendingRemoves}
    [] Bug = "wrong_peer_allowed" /\ c = AllowWrongPeer ->
      (spec \ {WrongPeerRejects}) \cup {AllowFreshReturnsTrue}
    [] Bug = "wrong_peer_consumes_owner" /\ c = AllowWrongPeer ->
      (spec \ {WrongPeerPreservesOwner}) \cup {AllowConsumesOne}
    [] Bug = "prune_keeps_expired" /\ c = PruneExpired ->
      spec \ {PruneExpiredRemoves}
    [] Bug = "prune_drops_fresh" /\ c = PruneFresh ->
      (spec \ {PruneFreshKeeps}) \cup {PruneExpiredRemoves}
    [] OTHER -> spec

Bugs == {
  "none",
  "default_pauses",
  "startup_not_paused",
  "pressure_not_paused",
  "lane_not_paused",
  "sync_skips_pressure_true",
  "sync_skips_pressure_false",
  "sync_skips_lane_true",
  "sync_skips_lane_false",
  "allow_absent_peer",
  "record_skips_create",
  "record_skips_last_request",
  "record_skips_increment",
  "record_overflows_pending",
  "record_skips_prune",
  "allow_fresh_rejects",
  "allow_fresh_not_consumed",
  "allow_single_not_removed",
  "allow_multi_removed",
  "ttl_boundary_rejected",
  "expired_allowed",
  "expired_kept",
  "zero_pending_allowed",
  "zero_pending_kept",
  "wrong_peer_allowed",
  "wrong_peer_consumes_owner",
  "prune_keeps_expired",
  "prune_drops_fresh"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

LatestGossipPauseMatchesHints ==
  /\ Pause \notin ImplementationActions(DefaultReady)
  /\ Pause \in ImplementationActions(StartupUninitialized)
  /\ Pause \in ImplementationActions(PressureActive)
  /\ Pause \in ImplementationActions(LaneActive)
  /\ Pause \in ImplementationActions(BothActive)

SyncExternalHintsStoresBothFlags ==
  /\ ImplementationActions(SyncPressureOnly) =
       {SyncPressureTrue, SyncLaneFalse}
  /\ ImplementationActions(SyncLaneOnly) =
       {SyncPressureFalse, SyncLaneTrue}
  /\ ImplementationActions(SyncClearStale) =
       {SyncPressureFalse, SyncLaneFalse}
  /\ ImplementationActions(SyncBoth) =
       {SyncPressureTrue, SyncLaneTrue}

DirectResponsePermitAdmission ==
  /\ NoPermitReject \in ImplementationActions(AllowAbsentPeer)
  /\ AllowFreshReturnsTrue \in ImplementationActions(AllowFreshSingle)
  /\ AllowFreshReturnsTrue \in ImplementationActions(AllowFreshMulti)
  /\ AllowAtTtl \in ImplementationActions(AllowExactTtl)
  /\ ExpiredRejects \in ImplementationActions(AllowExpired)
  /\ ZeroPendingRejects \in ImplementationActions(AllowZeroPending)
  /\ WrongPeerRejects \in ImplementationActions(AllowWrongPeer)

DirectResponsePermitAccounting ==
  /\ RecordCreatesPermit \in ImplementationActions(RecordNewPermit)
  /\ RecordUpdatesLastRequest \in ImplementationActions(RecordNewPermit)
  /\ RecordIncrementsPending \in ImplementationActions(RecordNewPermit)
  /\ RecordIncrementsPending \in
       ImplementationActions(RecordExistingIncrements)
  /\ RecordCapsPending \in ImplementationActions(RecordExistingSaturates)
  /\ AllowConsumesOne \in ImplementationActions(AllowFreshSingle)
  /\ AllowRemovesWhenZero \in ImplementationActions(AllowFreshSingle)
  /\ AllowRetainsPending \in ImplementationActions(AllowFreshMulti)
  /\ WrongPeerPreservesOwner \in ImplementationActions(AllowWrongPeer)

DirectResponsePermitPruning ==
  /\ PruneExpiredRemoves \in ImplementationActions(RecordPrunesExpired)
  /\ ExpiredRemoves \in ImplementationActions(AllowExpired)
  /\ ZeroPendingRemoves \in ImplementationActions(AllowZeroPending)
  /\ PruneExpiredRemoves \in ImplementationActions(PruneExpired)
  /\ PruneFreshKeeps \in ImplementationActions(PruneFresh)

SafetyFast ==
  /\ ActionsMatchSpec
  /\ LatestGossipPauseMatchesHints
  /\ SyncExternalHintsStoresBothFlags
  /\ DirectResponsePermitAdmission
  /\ DirectResponsePermitAccounting
  /\ DirectResponsePermitPruning

FrontierBlockSyncHintExactness ==
  /\ ActionsMatchSpec
  /\ LatestGossipPauseMatchesHints
  /\ SyncExternalHintsStoresBothFlags
  /\ DirectResponsePermitAdmission
  /\ DirectResponsePermitAccounting
  /\ DirectResponsePermitPruning

FrontierBlockSyncHintCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ FrontierBlockSyncHintExactness

====
