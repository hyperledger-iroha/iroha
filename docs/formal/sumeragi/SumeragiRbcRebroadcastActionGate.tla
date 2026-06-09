---- MODULE SumeragiRbcRebroadcastActionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the non-delivered per-session action branch in
`rebroadcast_stalled_rbc_payloads(...)`.

This slice starts after session cursor selection.  It pins the observable
contract for one selected non-delivered session: pending RBC is flushed before
repair work; local READY is retried only for live unsent sessions and reports
progress only when it flips `sent_ready`; missing or unauthoritative rosters
skip repair; progress observations are synchronized before invalid sessions
are skipped; authoritative complete sessions with no READY evidence do not
start payload repair; non-attempted chunk repair clears stale repair state;
payload repair requests report progress while broad payload rebroadcast still
requires hot repair, fallback/not-needed repair status, backpressure/cooldown
gates, rebroadcaster eligibility, and a buildable bundle; READY rebroadcast
requires hot repair, READY evidence, and cooldown; targeted READY rescue is
hot-repair gated; and progress is the disjunction of successful action side
effects.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FlushOnly == "flush_only"
ReadyReattemptEmits == "ready_reattempt_emits"
ReadyReattemptNoSend == "ready_reattempt_no_send"
RosterMissing == "roster_missing"
RosterUnauthoritative == "roster_unauthoritative"
InvalidAfterSync == "invalid_after_sync"
AuthoritativeCompleteNoReady == "authoritative_complete_no_ready"
ClearRepairWhenAuthoritative == "clear_repair_when_authoritative"
PayloadRepairRequested == "payload_repair_requested"
PayloadFallbackBroadcast == "payload_fallback_broadcast"
RelayBackpressureBlocksPayload == "relay_backpressure_blocks_payload"
QueueBackpressureBlocksPayload == "queue_backpressure_blocks_payload"
QueueExemptAllowsPayload == "queue_exempt_allows_payload"
NonRebroadcasterSkipsPayload == "non_rebroadcaster_skips_payload"
PayloadCooldownBlocks == "payload_cooldown_blocks"
ReadyBroadcast == "ready_broadcast"
ReadyZeroSkips == "ready_zero_skips"
ReadyCooldownBlocks == "ready_cooldown_blocks"
ExactFrontierChunkRepairOnly == "exact_frontier_chunk_repair_only"
HotSuppressedClearsRepair == "hot_suppressed_clears_repair"
TargetedRescue == "targeted_rescue"
CombinedActions == "combined_actions"

Cases == {
  FlushOnly,
  ReadyReattemptEmits,
  ReadyReattemptNoSend,
  RosterMissing,
  RosterUnauthoritative,
  InvalidAfterSync,
  AuthoritativeCompleteNoReady,
  ClearRepairWhenAuthoritative,
  PayloadRepairRequested,
  PayloadFallbackBroadcast,
  RelayBackpressureBlocksPayload,
  QueueBackpressureBlocksPayload,
  QueueExemptAllowsPayload,
  NonRebroadcasterSkipsPayload,
  PayloadCooldownBlocks,
  ReadyBroadcast,
  ReadyZeroSkips,
  ReadyCooldownBlocks,
  ExactFrontierChunkRepairOnly,
  HotSuppressedClearsRepair,
  TargetedRescue,
  CombinedActions
}

ActionResult(
  progress,
  flush_pending,
  ready_attempt,
  local_ready_emitted,
  sync_observations,
  payload_repair_requested,
  chunk_repair_cleared,
  payload_broadcast,
  ready_broadcast,
  targeted_rescue
) ==
  [
    progress |-> progress,
    flush_pending |-> flush_pending,
    ready_attempt |-> ready_attempt,
    local_ready_emitted |-> local_ready_emitted,
    sync_observations |-> sync_observations,
    payload_repair_requested |-> payload_repair_requested,
    chunk_repair_cleared |-> chunk_repair_cleared,
    payload_broadcast |-> payload_broadcast,
    ready_broadcast |-> ready_broadcast,
    targeted_rescue |-> targeted_rescue
  ]

NoAction ==
  ActionResult(FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)

SpecAction(c) ==
  CASE c = FlushOnly ->
      ActionResult(TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = ReadyReattemptEmits ->
      ActionResult(TRUE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = ReadyReattemptNoSend ->
      ActionResult(FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c \in {RosterMissing, RosterUnauthoritative} ->
      NoAction
    [] c = InvalidAfterSync ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = AuthoritativeCompleteNoReady ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = ClearRepairWhenAuthoritative ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, FALSE)
    [] c = PayloadRepairRequested ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE)
    [] c = PayloadFallbackBroadcast ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] c \in {RelayBackpressureBlocksPayload, QueueBackpressureBlocksPayload,
        NonRebroadcasterSkipsPayload, PayloadCooldownBlocks} ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = QueueExemptAllowsPayload ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] c = ReadyBroadcast ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, TRUE, FALSE)
    [] c = ReadyZeroSkips ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] c = ReadyCooldownBlocks ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, FALSE)
    [] c = ExactFrontierChunkRepairOnly ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE)
    [] c = HotSuppressedClearsRepair ->
      ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, FALSE)
    [] c = TargetedRescue ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, TRUE)
    [] c = CombinedActions ->
      ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, TRUE, TRUE)
    [] OTHER -> NoAction

ActualAction(c) ==
  CASE Bug = "flush_no_progress"
       /\ c = FlushOnly ->
         ActionResult(FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "ready_emit_no_progress"
       /\ c = ReadyReattemptEmits ->
         ActionResult(FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "ready_not_attempted"
       /\ c = ReadyReattemptNoSend ->
         NoAction
    [] Bug = "roster_missing_syncs"
       /\ c = RosterMissing ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "unauthoritative_syncs"
       /\ c = RosterUnauthoritative ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "invalid_runs_payload"
       /\ c = InvalidAfterSync ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "auth_complete_no_ready_repairs"
       /\ c = AuthoritativeCompleteNoReady ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "authoritative_skip_clear_repair"
       /\ c = ClearRepairWhenAuthoritative ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "repair_requested_no_progress"
       /\ c = PayloadRepairRequested ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "repair_requested_also_broadcasts"
       /\ c = PayloadRepairRequested ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "fallback_payload_skipped"
       /\ c = PayloadFallbackBroadcast ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "relay_backpressure_broadcasts"
       /\ c = RelayBackpressureBlocksPayload ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "queue_backpressure_broadcasts"
       /\ c = QueueBackpressureBlocksPayload ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "queue_exempt_blocked"
       /\ c = QueueExemptAllowsPayload ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "non_rebroadcaster_broadcasts"
       /\ c = NonRebroadcasterSkipsPayload ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "payload_cooldown_ignored"
       /\ c = PayloadCooldownBlocks ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "ready_broadcast_no_progress"
       /\ c = ReadyBroadcast ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "ready_zero_broadcasts"
       /\ c = ReadyZeroSkips ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, TRUE, FALSE)
    [] Bug = "ready_cooldown_ignored"
       /\ c = ReadyCooldownBlocks ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, TRUE, FALSE)
    [] Bug = "exact_frontier_broad_payload"
       /\ c = ExactFrontierChunkRepairOnly ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, TRUE, FALSE, FALSE)
    [] Bug = "hot_suppressed_keeps_repair"
       /\ c = HotSuppressedClearsRepair ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
    [] Bug = "hot_suppressed_rescues"
       /\ c = HotSuppressedClearsRepair ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, TRUE)
    [] Bug = "rescue_no_progress"
       /\ c = TargetedRescue ->
         ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, TRUE)
    [] Bug = "combined_skips_ready"
       /\ c = CombinedActions ->
         ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, TRUE)
    [] OTHER -> SpecAction(c)

BugSet == {
  "none",
  "flush_no_progress",
  "ready_emit_no_progress",
  "ready_not_attempted",
  "roster_missing_syncs",
  "unauthoritative_syncs",
  "invalid_runs_payload",
  "auth_complete_no_ready_repairs",
  "authoritative_skip_clear_repair",
  "repair_requested_no_progress",
  "repair_requested_also_broadcasts",
  "fallback_payload_skipped",
  "relay_backpressure_broadcasts",
  "queue_backpressure_broadcasts",
  "queue_exempt_blocked",
  "non_rebroadcaster_broadcasts",
  "payload_cooldown_ignored",
  "ready_broadcast_no_progress",
  "ready_zero_broadcasts",
  "ready_cooldown_ignored",
  "exact_frontier_broad_payload",
  "hot_suppressed_keeps_repair",
  "hot_suppressed_rescues",
  "rescue_no_progress",
  "combined_skips_ready"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in Cases:
       /\ ActualAction(c).progress \in BOOLEAN
       /\ ActualAction(c).flush_pending \in BOOLEAN
       /\ ActualAction(c).ready_attempt \in BOOLEAN
       /\ ActualAction(c).local_ready_emitted \in BOOLEAN
       /\ ActualAction(c).sync_observations \in BOOLEAN
       /\ ActualAction(c).payload_repair_requested \in BOOLEAN
       /\ ActualAction(c).chunk_repair_cleared \in BOOLEAN
       /\ ActualAction(c).payload_broadcast \in BOOLEAN
       /\ ActualAction(c).ready_broadcast \in BOOLEAN
       /\ ActualAction(c).targeted_rescue \in BOOLEAN

ActionExact ==
  \A c \in Cases:
    ActualAction(c) = SpecAction(c)

ProgressMatchesActions ==
  \A c \in Cases:
    ActualAction(c).progress =
      (ActualAction(c).flush_pending
        \/ ActualAction(c).local_ready_emitted
        \/ ActualAction(c).payload_repair_requested
        \/ ActualAction(c).payload_broadcast
        \/ ActualAction(c).ready_broadcast
        \/ ActualAction(c).targeted_rescue)

StableActions ==
  /\ ActualAction(FlushOnly) =
       ActionResult(TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(ReadyReattemptEmits) =
       ActionResult(TRUE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(ReadyReattemptNoSend) =
       ActionResult(FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(RosterMissing) = NoAction
  /\ ActualAction(RosterUnauthoritative) = NoAction
  /\ ActualAction(InvalidAfterSync) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(AuthoritativeCompleteNoReady) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(ClearRepairWhenAuthoritative) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, FALSE)
  /\ ActualAction(PayloadRepairRequested) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(PayloadFallbackBroadcast) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
  /\ ActualAction(RelayBackpressureBlocksPayload) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(QueueBackpressureBlocksPayload) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(QueueExemptAllowsPayload) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, FALSE, FALSE)
  /\ ActualAction(NonRebroadcasterSkipsPayload) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(PayloadCooldownBlocks) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(ReadyBroadcast) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, TRUE, FALSE)
  /\ ActualAction(ReadyZeroSkips) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(ReadyCooldownBlocks) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, FALSE)
  /\ ActualAction(ExactFrontierChunkRepairOnly) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, TRUE, FALSE, FALSE, FALSE, FALSE)
  /\ ActualAction(HotSuppressedClearsRepair) =
       ActionResult(FALSE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, FALSE)
  /\ ActualAction(TargetedRescue) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, TRUE, FALSE, FALSE, TRUE)
  /\ ActualAction(CombinedActions) =
       ActionResult(TRUE, FALSE, FALSE, FALSE, TRUE, FALSE, FALSE, TRUE, TRUE, TRUE)

RbcRebroadcastActionCoreSafety ==
  /\ ActionExact
  /\ ProgressMatchesActions
  /\ StableActions

SafetyFast ==
  RbcRebroadcastActionCoreSafety

====
