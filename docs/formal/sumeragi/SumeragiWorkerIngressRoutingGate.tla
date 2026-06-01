---- MODULE SumeragiWorkerIngressRoutingGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi's worker ingress routing.

This slice models the consensus-facing contracts in `SumeragiHandle` enqueue
paths, `PriorityTier::queue_kind`, `PriorityTier::stage`, `run_parallel_worker`,
`spawn_queue_worker`, and `drain_queue_batch`. It deliberately abstracts away
message payload contents and actor-side consensus semantics, which are covered
by other models. The checked surface is the routing and execution envelope:
network/control/background ingress reaches the intended worker queue with
matching metadata and accounting; each queue worker enters the actor gate at
the intended priority, publishes the intended stage, invokes the intended
handler, polls worker results after each handled message, and records bounded
batch drains.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  messageQueue,
  \* @type: Str;
  metadataQueue,
  \* @type: Str;
  workerTier,
  \* @type: Str;
  workerStage,
  \* @type: Str;
  gatePriority,
  \* @type: Str;
  handlerKind,
  \* @type: Int;
  batchLimit,
  \* @type: Int;
  drainCount,
  \* @type: Bool;
  accepted,
  \* @type: Bool;
  wakeRecorded,
  \* @type: Bool;
  enqueueRecorded,
  \* @type: Bool;
  blockedRecorded,
  \* @type: Bool;
  dropRecorded,
  \* @type: Bool;
  gateEntered,
  \* @type: Bool;
  stageSetBeforeHandle,
  \* @type: Bool;
  actorHandled,
  \* @type: Bool;
  pollAfterEach,
  \* @type: Bool;
  drainRecorded,
  \* @type: Bool;
  batchBounded,
  \* @type: Bool;
  stopOnEmpty,
  \* @type: Bool;
  idleRestored

\* @type: <<Str, Str, Str, Str, Str, Str, Str, Int, Int, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    messageQueue,
    metadataQueue,
    workerTier,
    workerStage,
    gatePriority,
    handlerKind,
    batchLimit,
    drainCount,
    accepted,
    wakeRecorded,
    enqueueRecorded,
    blockedRecorded,
    dropRecorded,
    gateEntered,
    stageSetBeforeHandle,
    actorHandled,
    pollAfterEach,
    drainRecorded,
    batchBounded,
    stopOnEmpty,
    idleRestored>>

Queues == {
  "Votes",
  "BlockPayload",
  "RbcChunks",
  "Blocks",
  "Consensus",
  "LaneRelay",
  "Background",
  "None"
}

Stages == {
  "DrainVotes",
  "DrainRbcChunks",
  "DrainBlockPayloads",
  "DrainBlocks",
  "DrainConsensus",
  "DrainLaneRelay",
  "DrainBackground",
  "Idle",
  "None"
}

Priorities == {
  "Urgent",
  "AvailabilityCritical",
  "AvailabilityBody",
  "Regular",
  "None"
}

Handlers == {
  "block_message",
  "consensus_control",
  "lane_relay",
  "background",
  "None"
}

BatchLimits == {0, 1, 4, 8}
DrainCounts == {0, 1, 4, 8, 9}

VoteRouteCases == {
  "route_qc_vote",
  "route_proposal_hint",
  "route_vrf_commit",
  "route_vrf_reveal"
}

PayloadRouteCases == {
  "route_qc_cert",
  "route_block_created",
  "route_proposal"
}

RbcRouteCases == {
  "route_rbc_init",
  "route_rbc_chunk",
  "route_rbc_ready",
  "route_rbc_deliver",
  "route_block_sync_update"
}

BlockRouteCases == {
  "route_block_body_response",
  "route_fetch_pending_block",
  "route_fetch_block_body",
  "route_certified_fetch",
  "route_other_block"
}

LaneRouteCases == {
  "route_lane_relay",
  "route_merge_signature",
  "route_native_amx"
}

BackgroundRouteCases == {
  "route_background_post",
  "route_background_broadcast",
  "route_background_control_broadcast"
}

RouteCases ==
  VoteRouteCases
    \cup PayloadRouteCases
    \cup RbcRouteCases
    \cup BlockRouteCases
    \cup {"route_consensus_control"}
    \cup LaneRouteCases
    \cup BackgroundRouteCases

EnqueueCases == {
  "enqueue_blocking_success",
  "enqueue_blocking_send_failure",
  "enqueue_nonblocking_success",
  "enqueue_nonblocking_full",
  "enqueue_nonblocking_disconnected"
}

WorkerCases == {
  "worker_votes",
  "worker_rbc_chunks",
  "worker_blocks",
  "worker_block_payload",
  "worker_consensus",
  "worker_lane_relay",
  "worker_background",
  "worker_batch_limit_floor",
  "worker_batch_limit_respected",
  "worker_batch_stops_on_empty",
  "worker_handler_error_keeps_drain",
  "worker_last_active_restores_idle",
  "worker_not_last_active_keeps_stage"
}

Cases == RouteCases \cup EnqueueCases \cup WorkerCases

SpecQueue(c) ==
  CASE c \in VoteRouteCases -> "Votes"
    [] c \in PayloadRouteCases -> "BlockPayload"
    [] c \in RbcRouteCases -> "RbcChunks"
    [] c \in BlockRouteCases -> "Blocks"
    [] c = "route_consensus_control" -> "Consensus"
    [] c \in LaneRouteCases -> "LaneRelay"
    [] c \in BackgroundRouteCases -> "Background"
    [] c \in {"enqueue_blocking_success", "enqueue_blocking_send_failure"} -> "Votes"
    [] c \in {"enqueue_nonblocking_success", "enqueue_nonblocking_full", "enqueue_nonblocking_disconnected"} -> "BlockPayload"
    [] c = "worker_votes" -> "Votes"
    [] c = "worker_rbc_chunks" -> "RbcChunks"
    [] c = "worker_blocks" -> "Blocks"
    [] c = "worker_block_payload" -> "BlockPayload"
    [] c = "worker_consensus" -> "Consensus"
    [] c = "worker_lane_relay" -> "LaneRelay"
    [] c = "worker_background" -> "Background"
    [] c = "worker_batch_limit_floor" -> "Background"
    [] c = "worker_batch_limit_respected" -> "Votes"
    [] c = "worker_batch_stops_on_empty" -> "Votes"
    [] c = "worker_handler_error_keeps_drain" -> "Blocks"
    [] c = "worker_last_active_restores_idle" -> "Background"
    [] c = "worker_not_last_active_keeps_stage" -> "RbcChunks"
    [] OTHER -> "None"

SpecMetadataQueue(c) ==
  CASE c \in (RouteCases \cup EnqueueCases) -> SpecQueue(c)
    [] OTHER -> "None"

SpecWorkerTier(c) ==
  CASE c \in WorkerCases -> SpecQueue(c)
    [] OTHER -> "None"

SpecStageForQueue(q) ==
  CASE q = "Votes" -> "DrainVotes"
    [] q = "RbcChunks" -> "DrainRbcChunks"
    [] q = "BlockPayload" -> "DrainBlockPayloads"
    [] q = "Blocks" -> "DrainBlocks"
    [] q = "Consensus" -> "DrainConsensus"
    [] q = "LaneRelay" -> "DrainLaneRelay"
    [] q = "Background" -> "DrainBackground"
    [] OTHER -> "None"

SpecWorkerStage(c) ==
  CASE c \in WorkerCases -> SpecStageForQueue(SpecQueue(c))
    [] OTHER -> "None"

SpecGatePriorityForQueue(q) ==
  CASE q \in {"Votes", "Consensus", "LaneRelay"} -> "Urgent"
    [] q \in {"RbcChunks", "Blocks"} -> "AvailabilityCritical"
    [] q = "BlockPayload" -> "AvailabilityBody"
    [] q = "Background" -> "Regular"
    [] OTHER -> "None"

SpecGatePriority(c) ==
  CASE c \in WorkerCases -> SpecGatePriorityForQueue(SpecQueue(c))
    [] OTHER -> "None"

SpecHandlerForQueue(q) ==
  CASE q \in {"Votes", "RbcChunks", "BlockPayload", "Blocks"} -> "block_message"
    [] q = "Consensus" -> "consensus_control"
    [] q = "LaneRelay" -> "lane_relay"
    [] q = "Background" -> "background"
    [] OTHER -> "None"

SpecHandlerKind(c) ==
  CASE c \in WorkerCases -> SpecHandlerForQueue(SpecQueue(c))
    [] OTHER -> "None"

SpecBatchLimit(c) ==
  CASE c = "worker_votes" -> 8
    [] c = "worker_rbc_chunks" -> 4
    [] c = "worker_batch_limit_respected" -> 8
    [] c = "worker_batch_stops_on_empty" -> 8
    [] c = "worker_batch_limit_floor" -> 1
    [] c \in WorkerCases -> 1
    [] OTHER -> 0

SpecDrainCount(c) ==
  CASE c = "worker_batch_limit_respected" -> 8
    [] c = "worker_rbc_chunks" -> 4
    [] c \in WorkerCases -> 1
    [] OTHER -> 0

SpecAccepted(c) ==
  c \in RouteCases
    \/ c \in {"enqueue_blocking_success", "enqueue_nonblocking_success"}

SpecWakeRecorded(c) ==
  c \in RouteCases
    \/ c \in {"enqueue_blocking_success", "enqueue_blocking_send_failure", "enqueue_nonblocking_success"}

SpecEnqueueRecorded(c) ==
  SpecAccepted(c)

SpecBlockedRecorded(c) ==
  c = "enqueue_blocking_success"

SpecDropRecorded(c) ==
  c \in {"enqueue_blocking_send_failure", "enqueue_nonblocking_full", "enqueue_nonblocking_disconnected"}

SpecGateEntered(c) ==
  c \in WorkerCases

SpecStageSetBeforeHandle(c) ==
  c \in WorkerCases

SpecActorHandled(c) ==
  c \in WorkerCases

SpecPollAfterEach(c) ==
  c \in WorkerCases

SpecDrainRecorded(c) ==
  c \in WorkerCases

SpecBatchBounded(c) ==
  c \in WorkerCases

SpecStopOnEmpty(c) ==
  c = "worker_batch_stops_on_empty"

SpecIdleRestored(c) ==
  c = "worker_last_active_restores_idle"

ActualQueue(c) ==
  CASE c = "route_qc_vote" /\ Bug = "route_vote_to_payload" -> "BlockPayload"
    [] c = "route_qc_cert" /\ Bug = "route_qc_to_votes" -> "Votes"
    [] c \in RbcRouteCases /\ Bug = "route_rbc_to_payload" -> "BlockPayload"
    [] c = "route_block_sync_update" /\ Bug = "route_block_sync_to_payload" -> "BlockPayload"
    [] c = "route_block_created" /\ Bug = "route_block_created_to_blocks" -> "Blocks"
    [] c \in BlockRouteCases /\ Bug = "route_body_to_payload" -> "BlockPayload"
    [] c = "route_consensus_control" /\ Bug = "route_consensus_to_background" -> "Background"
    [] c \in LaneRouteCases /\ Bug = "route_lane_to_background" -> "Background"
    [] c \in BackgroundRouteCases /\ Bug = "route_background_to_consensus" -> "Consensus"
    [] OTHER -> SpecQueue(c)

ActualMetadataQueue(c) ==
  CASE c \in (RouteCases \cup EnqueueCases) /\ Bug = "missing_metadata" -> "None"
    [] c \in (RouteCases \cup EnqueueCases) -> ActualQueue(c)
    [] OTHER -> SpecMetadataQueue(c)

ActualWorkerTier(c) ==
  CASE c \in WorkerCases /\ Bug = "worker_wrong_stage" -> SpecWorkerTier(c)
    [] OTHER -> SpecWorkerTier(c)

ActualWorkerStage(c) ==
  CASE c \in WorkerCases /\ Bug = "worker_wrong_stage" -> "Idle"
    [] OTHER -> SpecWorkerStage(c)

ActualGatePriority(c) ==
  CASE c = "worker_votes" /\ Bug = "votes_not_urgent" -> "Regular"
    [] c = "worker_rbc_chunks" /\ Bug = "rbc_not_critical" -> "AvailabilityBody"
    [] c = "worker_blocks" /\ Bug = "blocks_not_critical" -> "Regular"
    [] c = "worker_block_payload" /\ Bug = "payload_not_body" -> "AvailabilityCritical"
    [] c \in {"worker_consensus", "worker_lane_relay"} /\ Bug = "control_not_urgent" -> "Regular"
    [] c = "worker_background" /\ Bug = "background_not_regular" -> "Urgent"
    [] OTHER -> SpecGatePriority(c)

ActualHandlerKind(c) ==
  CASE c \in WorkerCases /\ Bug = "worker_wrong_handler" -> "background"
    [] OTHER -> SpecHandlerKind(c)

ActualBatchLimit(c) ==
  CASE c = "worker_votes" /\ Bug = "vote_batch_limit_one" -> 1
    [] c = "worker_rbc_chunks" /\ Bug = "rbc_batch_limit_one" -> 1
    [] c = "worker_batch_limit_floor" /\ Bug = "batch_limit_zero_not_floored" -> 0
    [] OTHER -> SpecBatchLimit(c)

ActualDrainCount(c) ==
  CASE c = "worker_batch_limit_respected" /\ Bug = "batch_ignores_limit" -> 9
    [] c = "worker_batch_stops_on_empty" /\ Bug = "batch_continues_on_empty" -> 4
    [] OTHER -> SpecDrainCount(c)

ActualAccepted(c) ==
  SpecAccepted(c)

ActualWakeRecorded(c) ==
  CASE SpecAccepted(c) /\ Bug = "accepted_missing_wake" -> FALSE
    [] c \in {"enqueue_nonblocking_full", "enqueue_nonblocking_disconnected"} /\ Bug = "nonblocking_failure_wakes" -> TRUE
    [] OTHER -> SpecWakeRecorded(c)

ActualEnqueueRecorded(c) ==
  CASE SpecAccepted(c) /\ Bug = "missing_enqueue_record" -> FALSE
    [] OTHER -> SpecEnqueueRecorded(c)

ActualBlockedRecorded(c) ==
  CASE c = "enqueue_blocking_success" /\ Bug = "blocking_missing_blocked_record" -> FALSE
    [] OTHER -> SpecBlockedRecorded(c)

ActualDropRecorded(c) ==
  CASE ~SpecAccepted(c) /\ c \in EnqueueCases /\ Bug = "failed_missing_drop" -> FALSE
    [] SpecAccepted(c) /\ Bug = "accepted_records_drop" -> TRUE
    [] OTHER -> SpecDropRecorded(c)

ActualGateEntered(c) ==
  CASE c \in WorkerCases /\ Bug = "actor_without_gate" -> FALSE
    [] OTHER -> SpecGateEntered(c)

ActualStageSetBeforeHandle(c) ==
  CASE c \in WorkerCases /\ Bug = "stage_after_handle" -> FALSE
    [] OTHER -> SpecStageSetBeforeHandle(c)

ActualActorHandled(c) ==
  SpecActorHandled(c)

ActualPollAfterEach(c) ==
  CASE c \in WorkerCases /\ Bug = "missing_poll_after_handle" -> FALSE
    [] OTHER -> SpecPollAfterEach(c)

ActualDrainRecorded(c) ==
  CASE c \in WorkerCases /\ Bug = "missing_drain_record" -> FALSE
    [] OTHER -> SpecDrainRecorded(c)

ActualBatchBounded(c) ==
  CASE c = "worker_batch_limit_respected" /\ Bug = "batch_ignores_limit" -> FALSE
    [] OTHER -> SpecBatchBounded(c)

ActualStopOnEmpty(c) ==
  CASE c = "worker_batch_stops_on_empty" /\ Bug = "batch_continues_on_empty" -> FALSE
    [] OTHER -> SpecStopOnEmpty(c)

ActualIdleRestored(c) ==
  CASE c = "worker_last_active_restores_idle" /\ Bug = "last_active_no_idle" -> FALSE
    [] OTHER -> SpecIdleRestored(c)

BugModes == {
  "none",
  "route_vote_to_payload",
  "route_qc_to_votes",
  "route_rbc_to_payload",
  "route_block_sync_to_payload",
  "route_block_created_to_blocks",
  "route_body_to_payload",
  "route_consensus_to_background",
  "route_lane_to_background",
  "route_background_to_consensus",
  "missing_metadata",
  "missing_enqueue_record",
  "accepted_records_drop",
  "failed_missing_drop",
  "blocking_missing_blocked_record",
  "accepted_missing_wake",
  "nonblocking_failure_wakes",
  "votes_not_urgent",
  "rbc_not_critical",
  "blocks_not_critical",
  "payload_not_body",
  "control_not_urgent",
  "background_not_regular",
  "worker_wrong_stage",
  "worker_wrong_handler",
  "vote_batch_limit_one",
  "rbc_batch_limit_one",
  "batch_limit_zero_not_floored",
  "batch_ignores_limit",
  "batch_continues_on_empty",
  "actor_without_gate",
  "stage_after_handle",
  "missing_poll_after_handle",
  "missing_drain_record",
  "last_active_no_idle"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ messageQueue \in Queues
  /\ metadataQueue \in Queues
  /\ workerTier \in Queues
  /\ workerStage \in Stages
  /\ gatePriority \in Priorities
  /\ handlerKind \in Handlers
  /\ batchLimit \in BatchLimits
  /\ drainCount \in DrainCounts
  /\ accepted \in BOOLEAN
  /\ wakeRecorded \in BOOLEAN
  /\ enqueueRecorded \in BOOLEAN
  /\ blockedRecorded \in BOOLEAN
  /\ dropRecorded \in BOOLEAN
  /\ gateEntered \in BOOLEAN
  /\ stageSetBeforeHandle \in BOOLEAN
  /\ actorHandled \in BOOLEAN
  /\ pollAfterEach \in BOOLEAN
  /\ drainRecorded \in BOOLEAN
  /\ batchBounded \in BOOLEAN
  /\ stopOnEmpty \in BOOLEAN
  /\ idleRestored \in BOOLEAN

Init ==
  /\ candidate = "route_qc_vote"
  /\ messageQueue = "Votes"
  /\ metadataQueue = "Votes"
  /\ workerTier = "None"
  /\ workerStage = "None"
  /\ gatePriority = "None"
  /\ handlerKind = "None"
  /\ batchLimit = 0
  /\ drainCount = 0
  /\ accepted = TRUE
  /\ wakeRecorded = TRUE
  /\ enqueueRecorded = TRUE
  /\ blockedRecorded = FALSE
  /\ dropRecorded = FALSE
  /\ gateEntered = FALSE
  /\ stageSetBeforeHandle = FALSE
  /\ actorHandled = FALSE
  /\ pollAfterEach = FALSE
  /\ drainRecorded = FALSE
  /\ batchBounded = FALSE
  /\ stopOnEmpty = FALSE
  /\ idleRestored = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ messageQueue' = ActualQueue(c)
  /\ metadataQueue' = ActualMetadataQueue(c)
  /\ workerTier' = ActualWorkerTier(c)
  /\ workerStage' = ActualWorkerStage(c)
  /\ gatePriority' = ActualGatePriority(c)
  /\ handlerKind' = ActualHandlerKind(c)
  /\ batchLimit' = ActualBatchLimit(c)
  /\ drainCount' = ActualDrainCount(c)
  /\ accepted' = ActualAccepted(c)
  /\ wakeRecorded' = ActualWakeRecorded(c)
  /\ enqueueRecorded' = ActualEnqueueRecorded(c)
  /\ blockedRecorded' = ActualBlockedRecorded(c)
  /\ dropRecorded' = ActualDropRecorded(c)
  /\ gateEntered' = ActualGateEntered(c)
  /\ stageSetBeforeHandle' = ActualStageSetBeforeHandle(c)
  /\ actorHandled' = ActualActorHandled(c)
  /\ pollAfterEach' = ActualPollAfterEach(c)
  /\ drainRecorded' = ActualDrainRecorded(c)
  /\ batchBounded' = ActualBatchBounded(c)
  /\ stopOnEmpty' = ActualStopOnEmpty(c)
  /\ idleRestored' = ActualIdleRestored(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ messageQueue = SpecQueue(candidate)
  /\ metadataQueue = SpecMetadataQueue(candidate)
  /\ workerTier = SpecWorkerTier(candidate)
  /\ workerStage = SpecWorkerStage(candidate)
  /\ gatePriority = SpecGatePriority(candidate)
  /\ handlerKind = SpecHandlerKind(candidate)
  /\ batchLimit = SpecBatchLimit(candidate)
  /\ drainCount = SpecDrainCount(candidate)
  /\ accepted = SpecAccepted(candidate)
  /\ wakeRecorded = SpecWakeRecorded(candidate)
  /\ enqueueRecorded = SpecEnqueueRecorded(candidate)
  /\ blockedRecorded = SpecBlockedRecorded(candidate)
  /\ dropRecorded = SpecDropRecorded(candidate)
  /\ gateEntered = SpecGateEntered(candidate)
  /\ stageSetBeforeHandle = SpecStageSetBeforeHandle(candidate)
  /\ actorHandled = SpecActorHandled(candidate)
  /\ pollAfterEach = SpecPollAfterEach(candidate)
  /\ drainRecorded = SpecDrainRecorded(candidate)
  /\ batchBounded = SpecBatchBounded(candidate)
  /\ stopOnEmpty = SpecStopOnEmpty(candidate)
  /\ idleRestored = SpecIdleRestored(candidate)

RouteMetadataMatchesQueue ==
  candidate \in RouteCases =>
    /\ metadataQueue = messageQueue
    /\ accepted
    /\ enqueueRecorded
    /\ wakeRecorded
    /\ ~dropRecorded

AcceptedIngressRecordsQueue ==
  candidate \in (RouteCases \cup EnqueueCases) /\ accepted =>
    /\ enqueueRecorded
    /\ metadataQueue = messageQueue
    /\ ~dropRecorded

FailedIngressRecordsDrop ==
  candidate \in EnqueueCases /\ ~accepted =>
    /\ dropRecorded
    /\ ~enqueueRecorded

BlockingIngressAccounting ==
  /\ candidate = "enqueue_blocking_success" =>
       /\ wakeRecorded
       /\ blockedRecorded
  /\ candidate = "enqueue_blocking_send_failure" =>
       /\ wakeRecorded
       /\ dropRecorded

NonblockingFailureDoesNotWake ==
  candidate \in {"enqueue_nonblocking_full", "enqueue_nonblocking_disconnected"} =>
    /\ dropRecorded
    /\ ~wakeRecorded

WorkerQueueMatchesTier ==
  candidate \in WorkerCases =>
    /\ workerTier = messageQueue
    /\ workerStage = SpecStageForQueue(messageQueue)

WorkerGatePriorityMatchesQueue ==
  candidate \in WorkerCases =>
    gatePriority = SpecGatePriorityForQueue(messageQueue)

WorkerHandlerMatchesQueue ==
  candidate \in WorkerCases =>
    handlerKind = SpecHandlerForQueue(messageQueue)

WorkerBatchLimits ==
  /\ candidate = "worker_votes" => batchLimit = 8
  /\ candidate = "worker_rbc_chunks" => batchLimit = 4
  /\ candidate \in {"worker_blocks", "worker_block_payload", "worker_consensus", "worker_lane_relay", "worker_background"} => batchLimit = 1
  /\ candidate = "worker_batch_limit_floor" => batchLimit = 1
  /\ candidate = "worker_batch_limit_respected" =>
       /\ batchBounded
       /\ drainCount <= batchLimit

WorkerDrainSequencing ==
  candidate \in WorkerCases =>
    /\ gateEntered
    /\ stageSetBeforeHandle
    /\ actorHandled
    /\ pollAfterEach
    /\ drainRecorded
    /\ batchBounded
    /\ drainCount >= 1

WorkerBatchStopsOnEmpty ==
  candidate = "worker_batch_stops_on_empty" =>
    /\ stopOnEmpty
    /\ drainCount = 1

WorkerLastActiveRestoresIdle ==
  /\ candidate = "worker_last_active_restores_idle" => idleRestored
  /\ candidate = "worker_not_last_active_keeps_stage" => ~idleRestored

Safety ==
  /\ MatchesSpec
  /\ RouteMetadataMatchesQueue
  /\ AcceptedIngressRecordsQueue
  /\ FailedIngressRecordsDrop
  /\ BlockingIngressAccounting
  /\ NonblockingFailureDoesNotWake
  /\ WorkerQueueMatchesTier
  /\ WorkerGatePriorityMatchesQueue
  /\ WorkerHandlerMatchesQueue
  /\ WorkerBatchLimits
  /\ WorkerDrainSequencing
  /\ WorkerBatchStopsOnEmpty
  /\ WorkerLastActiveRestoresIdle

=============================================================================
====
