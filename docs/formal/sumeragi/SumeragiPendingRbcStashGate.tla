---- MODULE SumeragiPendingRbcStashGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the pending-RBC stash.

`PendingRbcMessages` and `Actor::pending_rbc_slot(...)` temporarily retain
RBC chunks, READY messages, and DELIVER messages that arrive before INIT or
before roster/session context is available. This model captures the observable
contract: per-session chunk/byte caps, drop accounting, last-seen TTL refresh,
active-session protection, session-limit eviction of only inactive stashes,
dedup/metric/repair side effects on eviction, and replay of only retained
frames when INIT later arrives.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  frame_inserted,
  \* @type: Bool;
  frame_dropped,
  \* @type: Bool;
  chunk_evicted,
  \* @type: Bool;
  pending_bounded,
  \* @type: Bool;
  drop_counted,
  \* @type: Bool;
  ready_drop_counted,
  \* @type: Bool;
  deliver_drop_counted,
  \* @type: Bool;
  last_seen_touched,
  \* @type: Bool;
  ttl_evicted,
  \* @type: Bool;
  session_cap_evicted,
  \* @type: Bool;
  active_retained,
  \* @type: Bool;
  new_slot_available,
  \* @type: Bool;
  new_slot_rejected,
  \* @type: Bool;
  oldest_inactive_evicted,
  \* @type: Bool;
  flush_replayed_chunks,
  \* @type: Bool;
  flush_replayed_ready,
  \* @type: Bool;
  flush_replayed_deliver,
  \* @type: Bool;
  flush_removed_pending,
  \* @type: Bool;
  dedup_released,
  \* @type: Bool;
  metrics_recorded,
  \* @type: Bool;
  missing_repair_requested,
  \* @type: Bool;
  backlog_published,
  \* @type: Bool;
  evicted_frame_replayed

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, frame_inserted, frame_dropped, chunk_evicted,
  pending_bounded, drop_counted, ready_drop_counted, deliver_drop_counted,
  last_seen_touched, ttl_evicted, session_cap_evicted, active_retained,
  new_slot_available, new_slot_rejected, oldest_inactive_evicted,
  flush_replayed_chunks, flush_replayed_ready, flush_replayed_deliver,
  flush_removed_pending, dedup_released, metrics_recorded,
  missing_repair_requested, backlog_published, evicted_frame_replayed>>

Cases == {
  "chunk_insert_empty",
  "chunk_insert_at_cap",
  "chunk_insert_evict_oldest",
  "chunk_drop_zero_cap",
  "chunk_drop_oversize",
  "ready_accept",
  "ready_drop_over_cap",
  "deliver_accept",
  "deliver_drop_over_cap",
  "ttl_live",
  "ttl_expired_inactive",
  "ttl_expired_active",
  "ttl_disabled",
  "session_cap_under_limit",
  "session_cap_evict_oldest_inactive",
  "session_cap_all_active_reject_new",
  "session_cap_existing_key_allowed",
  "flush_after_init",
  "eviction_cleanup",
  "cap_drop_counts_only",
  "touch_extends_ttl"
}

InsertCases == {
  "chunk_insert_empty",
  "chunk_insert_at_cap",
  "chunk_insert_evict_oldest",
  "ready_accept",
  "deliver_accept"
}

DropCases == {
  "chunk_drop_zero_cap",
  "chunk_drop_oversize",
  "ready_drop_over_cap",
  "deliver_drop_over_cap"
}

ChunkEvictCases == {"chunk_insert_evict_oldest", "chunk_drop_oversize"}

StashAccountingCases == InsertCases \union DropCases \union {"cap_drop_counts_only"}

ChunkDropCountCases == {
  "chunk_insert_evict_oldest",
  "chunk_drop_zero_cap",
  "chunk_drop_oversize",
  "cap_drop_counts_only"
}

TrafficTouchCases ==
  InsertCases \union DropCases \union {
    "session_cap_under_limit",
    "session_cap_evict_oldest_inactive",
    "session_cap_existing_key_allowed",
    "cap_drop_counts_only",
    "touch_extends_ttl"
  }

TtlEvictionCases == {"ttl_expired_inactive"}

SessionCapEvictionCases == {"session_cap_evict_oldest_inactive"}

ActiveRetainedCases == {
  "ttl_expired_active",
  "session_cap_all_active_reject_new"
}

NewSlotAvailableCases == {
  "session_cap_under_limit",
  "session_cap_evict_oldest_inactive",
  "session_cap_existing_key_allowed"
}

NewSlotRejectedCases == {"session_cap_all_active_reject_new"}

FlushCases == {"flush_after_init"}

EvictionCleanupCases == {
  "ttl_expired_inactive",
  "session_cap_evict_oldest_inactive",
  "eviction_cleanup"
}

SpecFrameInserted(c) == c \in InsertCases

ActualFrameInserted(c) ==
  \/ /\ c \in {"chunk_insert_empty", "chunk_insert_at_cap", "chunk_insert_evict_oldest"}
     /\ Bug # "drop_insertable_chunk"
  \/ /\ c = "ready_accept"
     /\ Bug # "drop_ready_with_capacity"
  \/ /\ c = "deliver_accept"
     /\ Bug # "drop_deliver_with_capacity"
  \/ /\ c = "chunk_drop_zero_cap"
     /\ Bug = "accept_zero_cap_chunk"
  \/ /\ c = "chunk_drop_oversize"
     /\ Bug = "accept_oversize_chunk"
  \/ /\ c = "ready_drop_over_cap"
     /\ Bug = "ignore_ready_byte_cap"
  \/ /\ c = "deliver_drop_over_cap"
     /\ Bug = "ignore_deliver_byte_cap"

SpecFrameDropped(c) == c \in DropCases

ActualFrameDropped(c) ==
  IF ActualFrameInserted(c)
  THEN FALSE
  ELSE
    \/ c \in DropCases
    \/ /\ c \in {"chunk_insert_empty", "chunk_insert_at_cap", "chunk_insert_evict_oldest"}
       /\ Bug = "drop_insertable_chunk"
    \/ /\ c = "ready_accept"
       /\ Bug = "drop_ready_with_capacity"
    \/ /\ c = "deliver_accept"
       /\ Bug = "drop_deliver_with_capacity"

SpecChunkEvicted(c) == c \in ChunkEvictCases

ActualChunkEvicted(c) ==
  IF c = "chunk_insert_evict_oldest"
  THEN Bug # "skip_eviction_for_capped_insert"
  ELSE IF c = "chunk_drop_oversize" /\ ~ActualFrameInserted(c)
  THEN TRUE
  ELSE Bug = "evict_when_not_needed"

SpecPendingBounded(c) == c \in StashAccountingCases

ActualPendingBounded(c) ==
  IF c \in StashAccountingCases
  THEN Bug # "skip_pending_bound"
  ELSE FALSE

SpecDropCounted(c) == c \in ChunkDropCountCases

ActualDropCounted(c) ==
  IF c \in ChunkDropCountCases
  THEN Bug # "cap_drop_skips_counter"
  ELSE Bug = "count_clean_insert_as_drop"

SpecReadyDropCounted(c) == c = "ready_drop_over_cap"

ActualReadyDropCounted(c) ==
  IF c = "ready_drop_over_cap"
  THEN Bug # "skip_ready_drop_counter"
  ELSE Bug = "ready_counter_on_chunk_drop"

SpecDeliverDropCounted(c) == c = "deliver_drop_over_cap"

ActualDeliverDropCounted(c) ==
  IF c = "deliver_drop_over_cap"
  THEN Bug # "skip_deliver_drop_counter"
  ELSE Bug = "deliver_counter_on_chunk_drop"

SpecLastSeenTouched(c) == c \in TrafficTouchCases

ActualLastSeenTouched(c) ==
  IF c \in InsertCases
  THEN Bug # "no_touch_on_insert"
  ELSE IF c \in DropCases \union {"cap_drop_counts_only"}
  THEN Bug # "no_touch_on_drop"
  ELSE IF c \in {
    "session_cap_under_limit",
    "session_cap_evict_oldest_inactive",
    "session_cap_existing_key_allowed",
    "touch_extends_ttl"
  }
  THEN Bug # "no_touch_on_slot"
  ELSE Bug = "spurious_touch_without_traffic"

SpecTtlEvicted(c) == c \in TtlEvictionCases

ActualTtlEvicted(c) ==
  IF c = "ttl_expired_inactive"
  THEN Bug # "skip_ttl_eviction"
  ELSE IF c = "touch_extends_ttl"
  THEN Bug = "ttl_uses_first_seen"
  ELSE IF c = "ttl_disabled"
  THEN Bug = "ttl_disabled_evicts"
  ELSE IF c = "ttl_expired_active"
  THEN Bug = "ttl_evicts_active_session"
  ELSE FALSE

SpecSessionCapEvicted(c) == c \in SessionCapEvictionCases

ActualSessionCapEvicted(c) ==
  IF c = "session_cap_evict_oldest_inactive"
  THEN Bug # "skip_session_cap_eviction"
  ELSE IF c = "session_cap_all_active_reject_new"
  THEN Bug = "session_cap_evicts_active"
  ELSE FALSE

SpecActiveRetained(c) == c \in ActiveRetainedCases

ActualActiveRetained(c) ==
  IF c = "ttl_expired_active"
  THEN Bug # "ttl_evicts_active_session"
  ELSE IF c = "session_cap_all_active_reject_new"
  THEN Bug # "session_cap_evicts_active"
  ELSE FALSE

SpecNewSlotAvailable(c) == c \in NewSlotAvailableCases

ActualNewSlotAvailable(c) ==
  IF c = "session_cap_evict_oldest_inactive"
  THEN Bug # "session_cap_rejects_after_inactive_evict"
  ELSE IF c = "session_cap_existing_key_allowed"
  THEN Bug # "session_cap_rejects_existing_key"
  ELSE IF c = "session_cap_under_limit"
  THEN Bug # "session_cap_ignores_under_limit"
  ELSE Bug = "session_cap_ignores_limit"

SpecNewSlotRejected(c) == c \in NewSlotRejectedCases

ActualNewSlotRejected(c) ==
  IF c = "session_cap_all_active_reject_new"
  THEN Bug # "session_cap_ignores_limit"
  ELSE Bug = "reject_available_slot"

SpecOldestInactiveEvicted(c) == c = "session_cap_evict_oldest_inactive"

ActualOldestInactiveEvicted(c) ==
  IF c = "session_cap_evict_oldest_inactive"
  THEN Bug # "evict_newest_instead_of_oldest"
  ELSE Bug = "oldest_marker_without_eviction"

SpecFlushReplayedChunks(c) == c \in FlushCases

ActualFlushReplayedChunks(c) ==
  IF c \in FlushCases
  THEN Bug # "flush_drops_chunks"
  ELSE Bug = "replay_on_non_flush"

SpecFlushReplayedReady(c) == c \in FlushCases

ActualFlushReplayedReady(c) ==
  IF c \in FlushCases
  THEN Bug # "flush_drops_ready"
  ELSE Bug = "replay_on_non_flush"

SpecFlushReplayedDeliver(c) == c \in FlushCases

ActualFlushReplayedDeliver(c) ==
  IF c \in FlushCases
  THEN Bug # "flush_drops_deliver"
  ELSE Bug = "replay_on_non_flush"

SpecFlushRemovedPending(c) == c \in FlushCases

ActualFlushRemovedPending(c) ==
  IF c \in FlushCases
  THEN Bug # "flush_keeps_pending"
  ELSE FALSE

SpecDedupReleased(c) == c \in EvictionCleanupCases

ActualDedupReleased(c) ==
  IF c \in EvictionCleanupCases
  THEN Bug # "eviction_keeps_dedup"
  ELSE FALSE

SpecMetricsRecorded(c) == c \in EvictionCleanupCases

ActualMetricsRecorded(c) ==
  IF c \in EvictionCleanupCases
  THEN Bug # "eviction_skips_metrics"
  ELSE FALSE

SpecMissingRepairRequested(c) == c \in EvictionCleanupCases

ActualMissingRepairRequested(c) ==
  IF c \in EvictionCleanupCases
  THEN Bug # "eviction_skips_repair"
  ELSE FALSE

SpecBacklogPublished(c) == c \in EvictionCleanupCases

ActualBacklogPublished(c) ==
  IF c \in EvictionCleanupCases
  THEN Bug # "eviction_skips_backlog_publish"
  ELSE FALSE

SpecEvictedFrameReplayed(c) == FALSE

ActualEvictedFrameReplayed(c) ==
  \/ /\ c \in ChunkEvictCases
     /\ Bug = "replay_evicted_chunk"
  \/ /\ c = "flush_after_init"
     /\ Bug = "flush_replays_dropped_frame"

BugModes == {
  "none",
  "drop_insertable_chunk",
  "accept_zero_cap_chunk",
  "accept_oversize_chunk",
  "skip_eviction_for_capped_insert",
  "evict_when_not_needed",
  "skip_pending_bound",
  "cap_drop_skips_counter",
  "count_clean_insert_as_drop",
  "ignore_ready_byte_cap",
  "drop_ready_with_capacity",
  "skip_ready_drop_counter",
  "ready_counter_on_chunk_drop",
  "ignore_deliver_byte_cap",
  "drop_deliver_with_capacity",
  "skip_deliver_drop_counter",
  "deliver_counter_on_chunk_drop",
  "no_touch_on_insert",
  "no_touch_on_drop",
  "no_touch_on_slot",
  "spurious_touch_without_traffic",
  "skip_ttl_eviction",
  "ttl_uses_first_seen",
  "ttl_disabled_evicts",
  "ttl_evicts_active_session",
  "skip_session_cap_eviction",
  "session_cap_evicts_active",
  "session_cap_rejects_existing_key",
  "session_cap_rejects_after_inactive_evict",
  "session_cap_ignores_under_limit",
  "session_cap_ignores_limit",
  "reject_available_slot",
  "evict_newest_instead_of_oldest",
  "oldest_marker_without_eviction",
  "flush_drops_chunks",
  "flush_drops_ready",
  "flush_drops_deliver",
  "flush_keeps_pending",
  "replay_on_non_flush",
  "eviction_keeps_dedup",
  "eviction_skips_metrics",
  "eviction_skips_repair",
  "eviction_skips_backlog_publish",
  "replay_evicted_chunk",
  "flush_replays_dropped_frame"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ frame_inserted \in BOOLEAN
  /\ frame_dropped \in BOOLEAN
  /\ chunk_evicted \in BOOLEAN
  /\ pending_bounded \in BOOLEAN
  /\ drop_counted \in BOOLEAN
  /\ ready_drop_counted \in BOOLEAN
  /\ deliver_drop_counted \in BOOLEAN
  /\ last_seen_touched \in BOOLEAN
  /\ ttl_evicted \in BOOLEAN
  /\ session_cap_evicted \in BOOLEAN
  /\ active_retained \in BOOLEAN
  /\ new_slot_available \in BOOLEAN
  /\ new_slot_rejected \in BOOLEAN
  /\ oldest_inactive_evicted \in BOOLEAN
  /\ flush_replayed_chunks \in BOOLEAN
  /\ flush_replayed_ready \in BOOLEAN
  /\ flush_replayed_deliver \in BOOLEAN
  /\ flush_removed_pending \in BOOLEAN
  /\ dedup_released \in BOOLEAN
  /\ metrics_recorded \in BOOLEAN
  /\ missing_repair_requested \in BOOLEAN
  /\ backlog_published \in BOOLEAN
  /\ evicted_frame_replayed \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ frame_inserted = FALSE
  /\ frame_dropped = FALSE
  /\ chunk_evicted = FALSE
  /\ pending_bounded = FALSE
  /\ drop_counted = FALSE
  /\ ready_drop_counted = FALSE
  /\ deliver_drop_counted = FALSE
  /\ last_seen_touched = FALSE
  /\ ttl_evicted = FALSE
  /\ session_cap_evicted = FALSE
  /\ active_retained = FALSE
  /\ new_slot_available = FALSE
  /\ new_slot_rejected = FALSE
  /\ oldest_inactive_evicted = FALSE
  /\ flush_replayed_chunks = FALSE
  /\ flush_replayed_ready = FALSE
  /\ flush_replayed_deliver = FALSE
  /\ flush_removed_pending = FALSE
  /\ dedup_released = FALSE
  /\ metrics_recorded = FALSE
  /\ missing_repair_requested = FALSE
  /\ backlog_published = FALSE
  /\ evicted_frame_replayed = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ frame_inserted' = ActualFrameInserted(c)
  /\ frame_dropped' = ActualFrameDropped(c)
  /\ chunk_evicted' = ActualChunkEvicted(c)
  /\ pending_bounded' = ActualPendingBounded(c)
  /\ drop_counted' = ActualDropCounted(c)
  /\ ready_drop_counted' = ActualReadyDropCounted(c)
  /\ deliver_drop_counted' = ActualDeliverDropCounted(c)
  /\ last_seen_touched' = ActualLastSeenTouched(c)
  /\ ttl_evicted' = ActualTtlEvicted(c)
  /\ session_cap_evicted' = ActualSessionCapEvicted(c)
  /\ active_retained' = ActualActiveRetained(c)
  /\ new_slot_available' = ActualNewSlotAvailable(c)
  /\ new_slot_rejected' = ActualNewSlotRejected(c)
  /\ oldest_inactive_evicted' = ActualOldestInactiveEvicted(c)
  /\ flush_replayed_chunks' = ActualFlushReplayedChunks(c)
  /\ flush_replayed_ready' = ActualFlushReplayedReady(c)
  /\ flush_replayed_deliver' = ActualFlushReplayedDeliver(c)
  /\ flush_removed_pending' = ActualFlushRemovedPending(c)
  /\ dedup_released' = ActualDedupReleased(c)
  /\ metrics_recorded' = ActualMetricsRecorded(c)
  /\ missing_repair_requested' = ActualMissingRepairRequested(c)
  /\ backlog_published' = ActualBacklogPublished(c)
  /\ evicted_frame_replayed' = ActualEvictedFrameReplayed(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

FrameInsertedMatchesSpec ==
  candidate = "none" \/ frame_inserted = SpecFrameInserted(candidate)

FrameDroppedMatchesSpec ==
  candidate = "none" \/ frame_dropped = SpecFrameDropped(candidate)

ChunkEvictionMatchesSpec ==
  candidate = "none" \/ chunk_evicted = SpecChunkEvicted(candidate)

PendingBoundMatchesSpec ==
  candidate = "none" \/ pending_bounded = SpecPendingBounded(candidate)

DropCountMatchesSpec ==
  candidate = "none" \/ drop_counted = SpecDropCounted(candidate)

ReadyDropCountMatchesSpec ==
  candidate = "none" \/ ready_drop_counted = SpecReadyDropCounted(candidate)

DeliverDropCountMatchesSpec ==
  candidate = "none" \/ deliver_drop_counted = SpecDeliverDropCounted(candidate)

LastSeenTouchMatchesSpec ==
  candidate = "none" \/ last_seen_touched = SpecLastSeenTouched(candidate)

TtlEvictionMatchesSpec ==
  candidate = "none" \/ ttl_evicted = SpecTtlEvicted(candidate)

SessionCapEvictionMatchesSpec ==
  candidate = "none" \/ session_cap_evicted = SpecSessionCapEvicted(candidate)

ActiveRetentionMatchesSpec ==
  candidate = "none" \/ active_retained = SpecActiveRetained(candidate)

NewSlotAvailableMatchesSpec ==
  candidate = "none" \/ new_slot_available = SpecNewSlotAvailable(candidate)

NewSlotRejectedMatchesSpec ==
  candidate = "none" \/ new_slot_rejected = SpecNewSlotRejected(candidate)

OldestInactiveEvictedMatchesSpec ==
  candidate = "none" \/ oldest_inactive_evicted = SpecOldestInactiveEvicted(candidate)

FlushChunksMatchesSpec ==
  candidate = "none" \/ flush_replayed_chunks = SpecFlushReplayedChunks(candidate)

FlushReadyMatchesSpec ==
  candidate = "none" \/ flush_replayed_ready = SpecFlushReplayedReady(candidate)

FlushDeliverMatchesSpec ==
  candidate = "none" \/ flush_replayed_deliver = SpecFlushReplayedDeliver(candidate)

FlushRemovedPendingMatchesSpec ==
  candidate = "none" \/ flush_removed_pending = SpecFlushRemovedPending(candidate)

DedupReleaseMatchesSpec ==
  candidate = "none" \/ dedup_released = SpecDedupReleased(candidate)

MetricsRecordedMatchesSpec ==
  candidate = "none" \/ metrics_recorded = SpecMetricsRecorded(candidate)

MissingRepairMatchesSpec ==
  candidate = "none" \/ missing_repair_requested = SpecMissingRepairRequested(candidate)

BacklogPublishedMatchesSpec ==
  candidate = "none" \/ backlog_published = SpecBacklogPublished(candidate)

EvictedReplayMatchesSpec ==
  candidate = "none" \/ evicted_frame_replayed = SpecEvictedFrameReplayed(candidate)

BoundedStashNeverExceedsCaps ==
  candidate \in StashAccountingCases => pending_bounded

DroppedTrafficIsAccounted ==
  candidate \in DropCases =>
    \/ drop_counted
    \/ ready_drop_counted
    \/ deliver_drop_counted

TrafficRefreshesLastSeen ==
  candidate \in TrafficTouchCases => last_seen_touched

TtlOnlyEvictsInactiveExpired ==
  ttl_evicted => candidate = "ttl_expired_inactive"

SessionCapEvictsOnlyOldestInactive ==
  session_cap_evicted =>
    /\ candidate = "session_cap_evict_oldest_inactive"
    /\ oldest_inactive_evicted

ActiveSessionsProtectPendingStash ==
  candidate \in ActiveRetainedCases =>
    /\ active_retained
    /\ ~ttl_evicted
    /\ ~session_cap_evicted

RejectedNewSlotOnlyWhenActiveCapFull ==
  new_slot_rejected => candidate = "session_cap_all_active_reject_new"

FlushReplaysOnlyRetainedFrames ==
  candidate = "flush_after_init" =>
    /\ flush_replayed_chunks
    /\ flush_replayed_ready
    /\ flush_replayed_deliver
    /\ flush_removed_pending
    /\ ~evicted_frame_replayed

EvictionsReleaseDedupRecordAndRepair ==
  candidate \in EvictionCleanupCases =>
    /\ dedup_released
    /\ metrics_recorded
    /\ missing_repair_requested
    /\ backlog_published

NoEvictedFrameReplay ==
  ~evicted_frame_replayed

Safety ==
  /\ FrameInsertedMatchesSpec
  /\ FrameDroppedMatchesSpec
  /\ ChunkEvictionMatchesSpec
  /\ PendingBoundMatchesSpec
  /\ DropCountMatchesSpec
  /\ ReadyDropCountMatchesSpec
  /\ DeliverDropCountMatchesSpec
  /\ LastSeenTouchMatchesSpec
  /\ TtlEvictionMatchesSpec
  /\ SessionCapEvictionMatchesSpec
  /\ ActiveRetentionMatchesSpec
  /\ NewSlotAvailableMatchesSpec
  /\ NewSlotRejectedMatchesSpec
  /\ OldestInactiveEvictedMatchesSpec
  /\ FlushChunksMatchesSpec
  /\ FlushReadyMatchesSpec
  /\ FlushDeliverMatchesSpec
  /\ FlushRemovedPendingMatchesSpec
  /\ DedupReleaseMatchesSpec
  /\ MetricsRecordedMatchesSpec
  /\ MissingRepairMatchesSpec
  /\ BacklogPublishedMatchesSpec
  /\ EvictedReplayMatchesSpec
  /\ BoundedStashNeverExceedsCaps
  /\ DroppedTrafficIsAccounted
  /\ TrafficRefreshesLastSeen
  /\ TtlOnlyEvictsInactiveExpired
  /\ SessionCapEvictsOnlyOldestInactive
  /\ ActiveSessionsProtectPendingStash
  /\ RejectedNewSlotOnlyWhenActiveCapFull
  /\ FlushReplaysOnlyRetainedFrames
  /\ EvictionsReleaseDedupRecordAndRepair
  /\ NoEvictedFrameReplay

====
