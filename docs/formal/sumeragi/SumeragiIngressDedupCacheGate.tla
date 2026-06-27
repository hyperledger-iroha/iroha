---- MODULE SumeragiIngressDedupCacheGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi ingress dedup caches.

This slice captures `DedupCache<T>` and `BlockPayloadDedupCache` in
`sumeragi/mod.rs`. It abstracts concrete keys and clocks to finite cases while
pinning the observable cache contract: cache capacity is floored at one,
duplicates refresh the LRU order without reporting insertion, expired entries
are evicted before admission, TTL zero disables expiry, the TTL boundary is
expired for ingress dedup caches, capacity eviction removes the oldest entry,
removal clears both map and LRU state, and block-payload dedup operations route
through independent per-kind buckets.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CapZeroClamp == "cap_zero_clamp"
CapPositivePreserved == "cap_positive_preserved"
InsertNewEmpty == "insert_new_empty"
InsertDuplicate == "insert_duplicate"
InsertExpiredThenNew == "insert_expired_then_new"
InsertTtlZeroNoExpire == "insert_ttl_zero_no_expire"
InsertAtTtlExpires == "insert_at_ttl_expires"
InsertCapacityFull == "insert_capacity_full"
InsertUnderCapNoEvict == "insert_under_cap_no_evict"
RemoveExisting == "remove_existing"
RemoveMissing == "remove_missing"
RouteBlockCreated == "route_block_created"
RouteFetchBlockBody == "route_fetch_block_body"
RouteBlockBodyResponse == "route_block_body_response"
RouteProposal == "route_proposal"
RouteRbcInit == "route_rbc_init"
RouteRbcReady == "route_rbc_ready"
RouteRbcDeliver == "route_rbc_deliver"
RouteBlockSyncUpdate == "route_block_sync_update"
RouteFetchPendingBlock == "route_fetch_pending_block"
RouteRbcChunk == "route_rbc_chunk"
KindsIndependent == "kinds_independent"
LenSumsBuckets == "len_sums_buckets"
LenForKeyRoutes == "len_for_key_routes"

Cases == {
  CapZeroClamp,
  CapPositivePreserved,
  InsertNewEmpty,
  InsertDuplicate,
  InsertExpiredThenNew,
  InsertTtlZeroNoExpire,
  InsertAtTtlExpires,
  InsertCapacityFull,
  InsertUnderCapNoEvict,
  RemoveExisting,
  RemoveMissing,
  RouteBlockCreated,
  RouteFetchBlockBody,
  RouteBlockBodyResponse,
  RouteProposal,
  RouteRbcInit,
  RouteRbcReady,
  RouteRbcDeliver,
  RouteBlockSyncUpdate,
  RouteFetchPendingBlock,
  RouteRbcChunk,
  KindsIndependent,
  LenSumsBuckets,
  LenForKeyRoutes
}

CapFloored == 1
CapPreserved == 2
InsertReturnsTrue == 3
InsertReturnsFalse == 4
EntryStored == 5
EntryPreserved == 6
LastSeenUpdated == 7
LruMovedBack == 8
ExpiredEvictedFirst == 9
NoExpiredEviction == 10
TtlBoundaryExpired == 11
CapacityEvicted == 12
OldestEntryEvicted == 13
NoCapacityEviction == 14
RemoveReturnsTrue == 15
RemoveReturnsFalse == 16
EntryRemoved == 17
LruRemoved == 18
MissingPreserved == 19
RouteBlockCreatedBucket == 20
RouteFetchBlockBodyBucket == 21
RouteBlockBodyResponseBucket == 22
RouteProposalBucket == 23
RouteRbcInitBucket == 24
RouteRbcReadyBucket == 25
RouteRbcDeliverBucket == 26
RouteBlockSyncUpdateBucket == 27
RouteFetchPendingBlockBucket == 28
RouteRbcChunkBucket == 29
OtherBucketsUnchanged == 30
LenIncludesAllBuckets == 31
LenForKeyUsesSelectedBucket == 32

ActionUniverse == 1..32

RouteAction(c) ==
  CASE c = RouteBlockCreated -> RouteBlockCreatedBucket
    [] c = RouteFetchBlockBody -> RouteFetchBlockBodyBucket
    [] c = RouteBlockBodyResponse -> RouteBlockBodyResponseBucket
    [] c = RouteProposal -> RouteProposalBucket
    [] c = RouteRbcInit -> RouteRbcInitBucket
    [] c = RouteRbcReady -> RouteRbcReadyBucket
    [] c = RouteRbcDeliver -> RouteRbcDeliverBucket
    [] c = RouteBlockSyncUpdate -> RouteBlockSyncUpdateBucket
    [] c = RouteFetchPendingBlock -> RouteFetchPendingBlockBucket
    [] c = RouteRbcChunk -> RouteRbcChunkBucket
    [] OTHER -> 0

RouteCases == {
  RouteBlockCreated,
  RouteFetchBlockBody,
  RouteBlockBodyResponse,
  RouteProposal,
  RouteRbcInit,
  RouteRbcReady,
  RouteRbcDeliver,
  RouteBlockSyncUpdate,
  RouteFetchPendingBlock,
  RouteRbcChunk
}

SpecActions(c) ==
  CASE c = CapZeroClamp -> {CapFloored}
    [] c = CapPositivePreserved -> {CapPreserved}
    [] c = InsertNewEmpty ->
      {InsertReturnsTrue, EntryStored, NoExpiredEviction, NoCapacityEviction}
    [] c = InsertDuplicate ->
      {InsertReturnsFalse, EntryPreserved, LastSeenUpdated, LruMovedBack,
       NoCapacityEviction}
    [] c = InsertExpiredThenNew ->
      {ExpiredEvictedFirst, InsertReturnsTrue, EntryStored, NoCapacityEviction}
    [] c = InsertTtlZeroNoExpire ->
      {NoExpiredEviction, InsertReturnsTrue, EntryStored}
    [] c = InsertAtTtlExpires ->
      {ExpiredEvictedFirst, TtlBoundaryExpired, InsertReturnsTrue, EntryStored}
    [] c = InsertCapacityFull ->
      {InsertReturnsTrue, EntryStored, CapacityEvicted, OldestEntryEvicted}
    [] c = InsertUnderCapNoEvict ->
      {InsertReturnsTrue, EntryStored, NoCapacityEviction}
    [] c = RemoveExisting ->
      {RemoveReturnsTrue, EntryRemoved, LruRemoved}
    [] c = RemoveMissing ->
      {RemoveReturnsFalse, MissingPreserved}
    [] c \in RouteCases ->
      {RouteAction(c), OtherBucketsUnchanged}
    [] c = KindsIndependent ->
      {OtherBucketsUnchanged}
    [] c = LenSumsBuckets ->
      {LenIncludesAllBuckets}
    [] c = LenForKeyRoutes ->
      {LenForKeyUsesSelectedBucket}
    [] OTHER -> {}

WrongRoute(c) ==
  CASE c = RouteBlockCreated -> RouteProposalBucket
    [] c = RouteFetchBlockBody -> RouteFetchPendingBlockBucket
    [] c = RouteBlockBodyResponse -> RouteBlockSyncUpdateBucket
    [] c = RouteProposal -> RouteBlockCreatedBucket
    [] c = RouteRbcInit -> RouteRbcChunkBucket
    [] c = RouteRbcReady -> RouteRbcDeliverBucket
    [] c = RouteRbcDeliver -> RouteRbcReadyBucket
    [] c = RouteBlockSyncUpdate -> RouteBlockBodyResponseBucket
    [] c = RouteFetchPendingBlock -> RouteFetchBlockBodyBucket
    [] c = RouteRbcChunk -> RouteRbcInitBucket
    [] OTHER -> 0

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "cap_zero_not_clamped" /\ c = CapZeroClamp ->
      spec \ {CapFloored}
    [] Bug = "cap_positive_clamped" /\ c = CapPositivePreserved ->
      (spec \ {CapPreserved}) \cup {CapFloored}
    [] Bug = "insert_new_reports_duplicate" /\ c = InsertNewEmpty ->
      (spec \ {InsertReturnsTrue}) \cup {InsertReturnsFalse}
    [] Bug = "insert_new_skips_entry" /\ c = InsertNewEmpty ->
      spec \ {EntryStored}
    [] Bug = "duplicate_reports_inserted" /\ c = InsertDuplicate ->
      (spec \ {InsertReturnsFalse}) \cup {InsertReturnsTrue}
    [] Bug = "duplicate_skips_refresh" /\ c = InsertDuplicate ->
      spec \ {LastSeenUpdated, LruMovedBack}
    [] Bug = "duplicate_counts_capacity" /\ c = InsertDuplicate ->
      (spec \ {NoCapacityEviction}) \cup {CapacityEvicted}
    [] Bug = "expired_not_evicted_first" /\ c = InsertExpiredThenNew ->
      spec \ {ExpiredEvictedFirst}
    [] Bug = "ttl_zero_evicts" /\ c = InsertTtlZeroNoExpire ->
      (spec \ {NoExpiredEviction}) \cup {ExpiredEvictedFirst}
    [] Bug = "ttl_boundary_kept" /\ c = InsertAtTtlExpires ->
      spec \ {ExpiredEvictedFirst, TtlBoundaryExpired}
    [] Bug = "capacity_skips_evict" /\ c = InsertCapacityFull ->
      spec \ {CapacityEvicted, OldestEntryEvicted}
    [] Bug = "capacity_evicts_newest" /\ c = InsertCapacityFull ->
      spec \ {OldestEntryEvicted}
    [] Bug = "under_cap_evicts" /\ c = InsertUnderCapNoEvict ->
      (spec \ {NoCapacityEviction}) \cup {CapacityEvicted}
    [] Bug = "remove_existing_returns_false" /\ c = RemoveExisting ->
      (spec \ {RemoveReturnsTrue}) \cup {RemoveReturnsFalse}
    [] Bug = "remove_existing_keeps_entry" /\ c = RemoveExisting ->
      spec \ {EntryRemoved}
    [] Bug = "remove_existing_keeps_lru" /\ c = RemoveExisting ->
      spec \ {LruRemoved}
    [] Bug = "remove_missing_returns_true" /\ c = RemoveMissing ->
      (spec \ {RemoveReturnsFalse}) \cup {RemoveReturnsTrue}
    [] Bug = "route_block_created_wrong" /\ c = RouteBlockCreated ->
      (spec \ {RouteBlockCreatedBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_fetch_block_body_wrong" /\ c = RouteFetchBlockBody ->
      (spec \ {RouteFetchBlockBodyBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_block_body_response_wrong" /\ c = RouteBlockBodyResponse ->
      (spec \ {RouteBlockBodyResponseBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_proposal_wrong" /\ c = RouteProposal ->
      (spec \ {RouteProposalBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_rbc_init_wrong" /\ c = RouteRbcInit ->
      (spec \ {RouteRbcInitBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_rbc_ready_wrong" /\ c = RouteRbcReady ->
      (spec \ {RouteRbcReadyBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_rbc_deliver_wrong" /\ c = RouteRbcDeliver ->
      (spec \ {RouteRbcDeliverBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_block_sync_update_wrong" /\ c = RouteBlockSyncUpdate ->
      (spec \ {RouteBlockSyncUpdateBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_fetch_pending_block_wrong" /\ c = RouteFetchPendingBlock ->
      (spec \ {RouteFetchPendingBlockBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_rbc_chunk_wrong" /\ c = RouteRbcChunk ->
      (spec \ {RouteRbcChunkBucket}) \cup {WrongRoute(c)}
    [] Bug = "route_mutates_other_buckets" /\ c \in RouteCases ->
      spec \ {OtherBucketsUnchanged}
    [] Bug = "kind_buckets_collide" /\ c = KindsIndependent ->
      spec \ {OtherBucketsUnchanged}
    [] Bug = "len_omits_bucket" /\ c = LenSumsBuckets ->
      spec \ {LenIncludesAllBuckets}
    [] Bug = "len_for_key_wrong_bucket" /\ c = LenForKeyRoutes ->
      spec \ {LenForKeyUsesSelectedBucket}
    [] OTHER -> spec

Bugs == {
  "none",
  "cap_zero_not_clamped",
  "cap_positive_clamped",
  "insert_new_reports_duplicate",
  "insert_new_skips_entry",
  "duplicate_reports_inserted",
  "duplicate_skips_refresh",
  "duplicate_counts_capacity",
  "expired_not_evicted_first",
  "ttl_zero_evicts",
  "ttl_boundary_kept",
  "capacity_skips_evict",
  "capacity_evicts_newest",
  "under_cap_evicts",
  "remove_existing_returns_false",
  "remove_existing_keeps_entry",
  "remove_existing_keeps_lru",
  "remove_missing_returns_true",
  "route_block_created_wrong",
  "route_fetch_block_body_wrong",
  "route_block_body_response_wrong",
  "route_proposal_wrong",
  "route_rbc_init_wrong",
  "route_rbc_ready_wrong",
  "route_rbc_deliver_wrong",
  "route_block_sync_update_wrong",
  "route_fetch_pending_block_wrong",
  "route_rbc_chunk_wrong",
  "route_mutates_other_buckets",
  "kind_buckets_collide",
  "len_omits_bucket",
  "len_for_key_wrong_bucket"
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

GenericDedupCacheMatchesContract ==
  /\ CapFloored \in ImplementationActions(CapZeroClamp)
  /\ CapPreserved \in ImplementationActions(CapPositivePreserved)
  /\ InsertReturnsTrue \in ImplementationActions(InsertNewEmpty)
  /\ EntryStored \in ImplementationActions(InsertNewEmpty)
  /\ InsertReturnsFalse \in ImplementationActions(InsertDuplicate)
  /\ LastSeenUpdated \in ImplementationActions(InsertDuplicate)
  /\ LruMovedBack \in ImplementationActions(InsertDuplicate)
  /\ ExpiredEvictedFirst \in ImplementationActions(InsertExpiredThenNew)
  /\ NoExpiredEviction \in ImplementationActions(InsertTtlZeroNoExpire)
  /\ TtlBoundaryExpired \in ImplementationActions(InsertAtTtlExpires)
  /\ CapacityEvicted \in ImplementationActions(InsertCapacityFull)
  /\ OldestEntryEvicted \in ImplementationActions(InsertCapacityFull)
  /\ NoCapacityEviction \in ImplementationActions(InsertUnderCapNoEvict)
  /\ RemoveReturnsTrue \in ImplementationActions(RemoveExisting)
  /\ EntryRemoved \in ImplementationActions(RemoveExisting)
  /\ LruRemoved \in ImplementationActions(RemoveExisting)
  /\ RemoveReturnsFalse \in ImplementationActions(RemoveMissing)

BlockPayloadDedupRoutesByKind ==
  /\ RouteBlockCreatedBucket \in ImplementationActions(RouteBlockCreated)
  /\ RouteFetchBlockBodyBucket \in ImplementationActions(RouteFetchBlockBody)
  /\ RouteBlockBodyResponseBucket \in
       ImplementationActions(RouteBlockBodyResponse)
  /\ RouteProposalBucket \in ImplementationActions(RouteProposal)
  /\ RouteRbcInitBucket \in ImplementationActions(RouteRbcInit)
  /\ RouteRbcReadyBucket \in ImplementationActions(RouteRbcReady)
  /\ RouteRbcDeliverBucket \in ImplementationActions(RouteRbcDeliver)
  /\ RouteBlockSyncUpdateBucket \in ImplementationActions(RouteBlockSyncUpdate)
  /\ RouteFetchPendingBlockBucket \in
       ImplementationActions(RouteFetchPendingBlock)
  /\ RouteRbcChunkBucket \in ImplementationActions(RouteRbcChunk)
  /\ \A c \in RouteCases:
       OtherBucketsUnchanged \in ImplementationActions(c)

BlockPayloadDedupAccountingMatchesBuckets ==
  /\ OtherBucketsUnchanged \in ImplementationActions(KindsIndependent)
  /\ LenIncludesAllBuckets \in ImplementationActions(LenSumsBuckets)
  /\ LenForKeyUsesSelectedBucket \in ImplementationActions(LenForKeyRoutes)

IngressDedupCacheExactness ==
  /\ ActionsMatchSpec
  /\ GenericDedupCacheMatchesContract
  /\ BlockPayloadDedupRoutesByKind
  /\ BlockPayloadDedupAccountingMatchesBuckets

IngressDedupCacheCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ IngressDedupCacheExactness

SafetyFast == IngressDedupCacheExactness

====
