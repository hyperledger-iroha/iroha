---- MODULE SumeragiQcVerifyAsyncGate ----

(***************************************************************************
A bounded abstract model for actor-side async QC aggregate verification.

This slice models ownership and fallback around consensus-QC and known-block
QC aggregate verification. It abstracts `handle_qc_with_aggregate(...)`,
`dispatch_known_block_qc_verify(...)`, `apply_known_block_qc_work(...)`, and
`poll_qc_verify_results(...)`. Cryptographic aggregate verification, signer
bitmap bounds, quorum arithmetic, and QC phase safety are covered by other
models. This gate checks that worker dispatch owns exactly one in-flight QC,
worker backlog or unavailable workers fall back to inline validation, duplicate
in-flight QCs do not add new owners, known-block stale-lock checks run before
worker dispatch, worker results apply only with matching ids, and a disconnected
result channel clears worker-owned state.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  inlineVerify,
  \* @type: Bool;
  deferWork,
  \* @type: Bool;
  addInflight,
  \* @type: Bool;
  removeInflight,
  \* @type: Bool;
  clearInflight,
  \* @type: Bool;
  clearWorkers,
  \* @type: Bool;
  clearResultRx,
  \* @type: Bool;
  keepResultRx,
  \* @type: Bool;
  duplicateDrop,
  \* @type: Bool;
  staleLockDrop,
  \* @type: Bool;
  useVerifiedCache,
  \* @type: Bool;
  aggregateResultProvided,
  \* @type: Bool;
  invokeConsensusHandler,
  \* @type: Bool;
  applyKnownBlockWork

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    inlineVerify,
    deferWork,
    addInflight,
    removeInflight,
    clearInflight,
    clearWorkers,
    clearResultRx,
    keepResultRx,
    duplicateDrop,
    staleLockDrop,
    useVerifiedCache,
    aggregateResultProvided,
    invokeConsensusHandler,
    applyKnownBlockWork>>

Cases == {
  "idle",
  "consensus_cached_verified",
  "consensus_no_workers_inline",
  "consensus_small_inline",
  "consensus_force_inline",
  "consensus_missing_inputs_inline",
  "consensus_send_success",
  "consensus_queue_full_inline",
  "consensus_disconnect_all_inline",
  "consensus_duplicate_inflight",
  "known_stale_lock_drop",
  "known_cached_tally",
  "known_no_workers_inline",
  "known_missing_inputs_inline",
  "known_send_success",
  "known_queue_full_inline",
  "known_disconnect_all_inline",
  "known_duplicate_inflight",
  "poll_no_inflight",
  "poll_id_mismatch",
  "poll_consensus_result",
  "poll_known_result",
  "poll_channel_disconnected"
}

ConsensusInlineCases == {
  "consensus_no_workers_inline",
  "consensus_small_inline",
  "consensus_force_inline",
  "consensus_missing_inputs_inline",
  "consensus_queue_full_inline",
  "consensus_disconnect_all_inline"
}

KnownInlineCases == {
  "known_no_workers_inline",
  "known_missing_inputs_inline",
  "known_queue_full_inline",
  "known_disconnect_all_inline"
}

DisconnectInlineCases == {
  "consensus_disconnect_all_inline",
  "known_disconnect_all_inline"
}

DuplicateCases == {
  "consensus_duplicate_inflight",
  "known_duplicate_inflight"
}

NormalPollCases == {
  "poll_no_inflight",
  "poll_id_mismatch",
  "poll_consensus_result",
  "poll_known_result"
}

SpecInlineVerify(c) ==
  c \in ConsensusInlineCases \/ c \in KnownInlineCases

SpecAddInflight(c) ==
  c \in {"consensus_send_success", "known_send_success"}

SpecDeferWork(c) ==
  SpecAddInflight(c)

SpecRemoveInflight(c) ==
  c \in {"poll_id_mismatch", "poll_consensus_result", "poll_known_result"}

SpecClearInflight(c) ==
  c \in DisconnectInlineCases \/ c = "poll_channel_disconnected"

SpecClearWorkers(c) ==
  c \in DisconnectInlineCases \/ c = "poll_channel_disconnected"

SpecClearResultRx(c) ==
  c \in DisconnectInlineCases \/ c = "poll_channel_disconnected"

SpecKeepResultRx(c) ==
  c \in NormalPollCases

SpecDuplicateDrop(c) ==
  c \in DuplicateCases

SpecStaleLockDrop(c) ==
  c = "known_stale_lock_drop"

SpecUseVerifiedCache(c) ==
  c = "consensus_cached_verified"

SpecAggregateResultProvided(c) ==
  c \in {"poll_consensus_result", "poll_known_result"}

SpecInvokeConsensusHandler(c) ==
  c = "consensus_cached_verified" \/ c \in ConsensusInlineCases \/ c = "poll_consensus_result"

SpecApplyKnownBlockWork(c) ==
  c = "known_cached_tally" \/ c \in KnownInlineCases \/ c = "poll_known_result"

ActualInlineVerify(c) ==
  CASE c \in ConsensusInlineCases /\ Bug = "consensus_inline_no_verify" -> FALSE
    [] c = "consensus_queue_full_inline" /\ Bug = "consensus_queue_full_no_inline" -> FALSE
    [] c = "consensus_disconnect_all_inline" /\ Bug = "consensus_disconnect_deferred" -> FALSE
    [] c \in KnownInlineCases /\ Bug = "known_inline_no_verify" -> FALSE
    [] c = "known_queue_full_inline" /\ Bug = "known_queue_full_drops" -> FALSE
    [] c = "known_disconnect_all_inline" /\ Bug = "known_disconnect_deferred" -> FALSE
    [] OTHER -> SpecInlineVerify(c)

ActualAddInflight(c) ==
  CASE c = "consensus_cached_verified" /\ Bug = "cache_deferred" -> TRUE
    [] c = "consensus_no_workers_inline" /\ Bug = "consensus_no_workers_deferred" -> TRUE
    [] c \in ConsensusInlineCases /\ Bug = "consensus_inline_deferred" -> TRUE
    [] c = "consensus_send_success" /\ Bug = "consensus_send_no_inflight" -> FALSE
    [] c = "consensus_queue_full_inline" /\ Bug = "consensus_queue_full_deferred" -> TRUE
    [] c = "consensus_duplicate_inflight" /\ Bug = "duplicate_queues" -> TRUE
    [] c = "known_cached_tally" /\ Bug = "known_cached_deferred" -> TRUE
    [] c \in KnownInlineCases /\ Bug = "known_inline_deferred" -> TRUE
    [] c = "known_send_success" /\ Bug = "known_send_no_inflight" -> FALSE
    [] c = "known_queue_full_inline" /\ Bug = "known_queue_full_deferred" -> TRUE
    [] c = "known_duplicate_inflight" /\ Bug = "duplicate_queues" -> TRUE
    [] OTHER -> SpecAddInflight(c)

ActualDeferWork(c) ==
  ActualAddInflight(c)

ActualRemoveInflight(c) ==
  CASE c = "poll_id_mismatch" /\ Bug = "poll_id_mismatch_keeps_inflight" -> FALSE
    [] OTHER -> SpecRemoveInflight(c)

ActualClearInflight(c) ==
  CASE c \in DisconnectInlineCases /\ Bug = "dispatch_disconnect_keeps_inflight" -> FALSE
    [] c = "poll_channel_disconnected" /\ Bug = "poll_disconnect_keeps_inflight" -> FALSE
    [] OTHER -> SpecClearInflight(c)

ActualClearWorkers(c) ==
  CASE c \in DisconnectInlineCases /\ Bug = "dispatch_disconnect_keeps_workers" -> FALSE
    [] c = "poll_channel_disconnected" /\ Bug = "poll_disconnect_keeps_workers" -> FALSE
    [] OTHER -> SpecClearWorkers(c)

ActualClearResultRx(c) ==
  CASE c \in DisconnectInlineCases /\ Bug = "dispatch_disconnect_keeps_rx" -> FALSE
    [] c = "poll_channel_disconnected" /\ Bug = "poll_disconnect_keeps_rx" -> FALSE
    [] OTHER -> SpecClearResultRx(c)

ActualKeepResultRx(c) ==
  CASE c \in NormalPollCases /\ Bug = "normal_poll_drops_rx" -> FALSE
    [] c = "poll_channel_disconnected" /\ Bug = "poll_disconnect_keeps_rx" -> TRUE
    [] OTHER -> SpecKeepResultRx(c)

ActualDuplicateDrop(c) ==
  CASE c \in DuplicateCases /\ Bug = "duplicate_not_dropped" -> FALSE
    [] OTHER -> SpecDuplicateDrop(c)

ActualStaleLockDrop(c) ==
  CASE c = "known_stale_lock_drop" /\ Bug \in {"known_stale_applies", "known_stale_dispatches"} -> FALSE
    [] OTHER -> SpecStaleLockDrop(c)

ActualUseVerifiedCache(c) ==
  CASE c = "consensus_cached_verified" /\ Bug = "cache_not_used" -> FALSE
    [] OTHER -> SpecUseVerifiedCache(c)

ActualAggregateResultProvided(c) ==
  CASE c \in {"poll_consensus_result", "poll_known_result"} /\ Bug = "poll_result_no_aggregate" -> FALSE
    [] OTHER -> SpecAggregateResultProvided(c)

ActualInvokeConsensusHandler(c) ==
  CASE c = "consensus_cached_verified" /\ Bug = "cache_not_used" -> FALSE
    [] c \in ConsensusInlineCases /\ Bug = "consensus_inline_no_handler" -> FALSE
    [] c = "consensus_send_success" /\ Bug = "consensus_send_applies" -> TRUE
    [] c = "consensus_duplicate_inflight" /\ Bug = "duplicate_applies" -> TRUE
    [] c = "poll_no_inflight" /\ Bug = "poll_no_inflight_applies" -> TRUE
    [] c = "poll_id_mismatch" /\ Bug = "poll_id_mismatch_applies" -> TRUE
    [] c = "poll_consensus_result" /\ Bug = "poll_consensus_no_handler" -> FALSE
    [] OTHER -> SpecInvokeConsensusHandler(c)

ActualApplyKnownBlockWork(c) ==
  CASE c = "known_stale_lock_drop" /\ Bug = "known_stale_applies" -> TRUE
    [] c = "known_cached_tally" /\ Bug = "known_cached_no_apply" -> FALSE
    [] c \in KnownInlineCases /\ Bug = "known_inline_no_apply" -> FALSE
    [] c = "known_send_success" /\ Bug = "known_send_applies" -> TRUE
    [] c = "known_duplicate_inflight" /\ Bug = "duplicate_applies" -> TRUE
    [] c = "poll_no_inflight" /\ Bug = "poll_no_inflight_applies" -> TRUE
    [] c = "poll_id_mismatch" /\ Bug = "poll_id_mismatch_applies" -> TRUE
    [] c = "poll_known_result" /\ Bug = "poll_known_no_apply" -> FALSE
    [] OTHER -> SpecApplyKnownBlockWork(c)

BugModes == {
  "none",
  "cache_not_used",
  "cache_deferred",
  "consensus_no_workers_deferred",
  "consensus_inline_deferred",
  "consensus_inline_no_verify",
  "consensus_inline_no_handler",
  "consensus_send_no_inflight",
  "consensus_send_applies",
  "consensus_queue_full_deferred",
  "consensus_queue_full_no_inline",
  "consensus_disconnect_deferred",
  "known_stale_applies",
  "known_stale_dispatches",
  "known_cached_deferred",
  "known_cached_no_apply",
  "known_inline_deferred",
  "known_inline_no_verify",
  "known_inline_no_apply",
  "known_send_no_inflight",
  "known_send_applies",
  "known_queue_full_deferred",
  "known_queue_full_drops",
  "known_disconnect_deferred",
  "duplicate_queues",
  "duplicate_applies",
  "duplicate_not_dropped",
  "dispatch_disconnect_keeps_inflight",
  "dispatch_disconnect_keeps_workers",
  "dispatch_disconnect_keeps_rx",
  "poll_no_inflight_applies",
  "poll_id_mismatch_applies",
  "poll_id_mismatch_keeps_inflight",
  "poll_consensus_no_handler",
  "poll_known_no_apply",
  "poll_result_no_aggregate",
  "poll_disconnect_keeps_inflight",
  "poll_disconnect_keeps_workers",
  "poll_disconnect_keeps_rx",
  "normal_poll_drops_rx"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ inlineVerify \in BOOLEAN
  /\ deferWork \in BOOLEAN
  /\ addInflight \in BOOLEAN
  /\ removeInflight \in BOOLEAN
  /\ clearInflight \in BOOLEAN
  /\ clearWorkers \in BOOLEAN
  /\ clearResultRx \in BOOLEAN
  /\ keepResultRx \in BOOLEAN
  /\ duplicateDrop \in BOOLEAN
  /\ staleLockDrop \in BOOLEAN
  /\ useVerifiedCache \in BOOLEAN
  /\ aggregateResultProvided \in BOOLEAN
  /\ invokeConsensusHandler \in BOOLEAN
  /\ applyKnownBlockWork \in BOOLEAN

Init ==
  /\ candidate = "idle"
  /\ inlineVerify = FALSE
  /\ deferWork = FALSE
  /\ addInflight = FALSE
  /\ removeInflight = FALSE
  /\ clearInflight = FALSE
  /\ clearWorkers = FALSE
  /\ clearResultRx = FALSE
  /\ keepResultRx = FALSE
  /\ duplicateDrop = FALSE
  /\ staleLockDrop = FALSE
  /\ useVerifiedCache = FALSE
  /\ aggregateResultProvided = FALSE
  /\ invokeConsensusHandler = FALSE
  /\ applyKnownBlockWork = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ inlineVerify' = ActualInlineVerify(c)
  /\ deferWork' = ActualDeferWork(c)
  /\ addInflight' = ActualAddInflight(c)
  /\ removeInflight' = ActualRemoveInflight(c)
  /\ clearInflight' = ActualClearInflight(c)
  /\ clearWorkers' = ActualClearWorkers(c)
  /\ clearResultRx' = ActualClearResultRx(c)
  /\ keepResultRx' = ActualKeepResultRx(c)
  /\ duplicateDrop' = ActualDuplicateDrop(c)
  /\ staleLockDrop' = ActualStaleLockDrop(c)
  /\ useVerifiedCache' = ActualUseVerifiedCache(c)
  /\ aggregateResultProvided' = ActualAggregateResultProvided(c)
  /\ invokeConsensusHandler' = ActualInvokeConsensusHandler(c)
  /\ applyKnownBlockWork' = ActualApplyKnownBlockWork(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ inlineVerify = SpecInlineVerify(candidate)
  /\ deferWork = SpecDeferWork(candidate)
  /\ addInflight = SpecAddInflight(candidate)
  /\ removeInflight = SpecRemoveInflight(candidate)
  /\ clearInflight = SpecClearInflight(candidate)
  /\ clearWorkers = SpecClearWorkers(candidate)
  /\ clearResultRx = SpecClearResultRx(candidate)
  /\ keepResultRx = SpecKeepResultRx(candidate)
  /\ duplicateDrop = SpecDuplicateDrop(candidate)
  /\ staleLockDrop = SpecStaleLockDrop(candidate)
  /\ useVerifiedCache = SpecUseVerifiedCache(candidate)
  /\ aggregateResultProvided = SpecAggregateResultProvided(candidate)
  /\ invokeConsensusHandler = SpecInvokeConsensusHandler(candidate)
  /\ applyKnownBlockWork = SpecApplyKnownBlockWork(candidate)

CachedConsensusDoesNotDispatch ==
  candidate = "consensus_cached_verified" =>
    /\ useVerifiedCache
    /\ invokeConsensusHandler
    /\ ~deferWork
    /\ ~addInflight

ConsensusInlineDoesNotOwnWorker ==
  candidate \in ConsensusInlineCases =>
    /\ inlineVerify
    /\ invokeConsensusHandler
    /\ ~addInflight

ConsensusDispatchOwnsInflight ==
  candidate = "consensus_send_success" =>
    /\ deferWork
    /\ addInflight
    /\ ~invokeConsensusHandler

KnownStaleLockDropsBeforeVerification ==
  candidate = "known_stale_lock_drop" =>
    /\ staleLockDrop
    /\ ~deferWork
    /\ ~addInflight
    /\ ~applyKnownBlockWork

KnownInlineAppliesWithoutWorkerOwner ==
  candidate \in KnownInlineCases \/ candidate = "known_cached_tally" =>
    /\ applyKnownBlockWork
    /\ ~addInflight

KnownDispatchOwnsInflight ==
  candidate = "known_send_success" =>
    /\ deferWork
    /\ addInflight
    /\ ~applyKnownBlockWork

DuplicatesDoNotQueueOrApply ==
  candidate \in DuplicateCases =>
    /\ duplicateDrop
    /\ ~addInflight
    /\ ~invokeConsensusHandler
    /\ ~applyKnownBlockWork

DispatchDisconnectClearsWorkerState ==
  candidate \in DisconnectInlineCases =>
    /\ inlineVerify
    /\ clearWorkers
    /\ clearInflight
    /\ clearResultRx
    /\ ~addInflight

PollResultRequiresInflight ==
  candidate = "poll_no_inflight" =>
    /\ ~removeInflight
    /\ ~invokeConsensusHandler
    /\ ~applyKnownBlockWork

IdMismatchRemovesOwnerOnly ==
  candidate = "poll_id_mismatch" =>
    /\ removeInflight
    /\ ~invokeConsensusHandler
    /\ ~applyKnownBlockWork

PollConsensusResultInvokesHandler ==
  candidate = "poll_consensus_result" =>
    /\ removeInflight
    /\ aggregateResultProvided
    /\ invokeConsensusHandler
    /\ ~applyKnownBlockWork

PollKnownResultAppliesWork ==
  candidate = "poll_known_result" =>
    /\ removeInflight
    /\ aggregateResultProvided
    /\ applyKnownBlockWork
    /\ ~invokeConsensusHandler

NormalPollKeepsResultReceiver ==
  candidate \in NormalPollCases => keepResultRx

PollDisconnectFailsClosed ==
  candidate = "poll_channel_disconnected" =>
    /\ clearWorkers
    /\ clearInflight
    /\ clearResultRx
    /\ ~keepResultRx
    /\ ~invokeConsensusHandler
    /\ ~applyKnownBlockWork

Safety ==
  /\ MatchesSpec
  /\ CachedConsensusDoesNotDispatch
  /\ ConsensusInlineDoesNotOwnWorker
  /\ ConsensusDispatchOwnsInflight
  /\ KnownStaleLockDropsBeforeVerification
  /\ KnownInlineAppliesWithoutWorkerOwner
  /\ KnownDispatchOwnsInflight
  /\ DuplicatesDoNotQueueOrApply
  /\ DispatchDisconnectClearsWorkerState
  /\ PollResultRequiresInflight
  /\ IdMismatchRemovesOwnerOnly
  /\ PollConsensusResultInvokesHandler
  /\ PollKnownResultAppliesWork
  /\ NormalPollKeepsResultReceiver
  /\ PollDisconnectFailsClosed

=============================================================================
