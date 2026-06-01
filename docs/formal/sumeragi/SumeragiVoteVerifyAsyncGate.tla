---- MODULE SumeragiVoteVerifyAsyncGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the actor-side async vote verification gate.

This slice models the state effects around `try_dispatch_vote_verification`,
`dispatch_pending_vote_verifications`, and `poll_vote_verify_results`. It does
not model cryptographic signature math; helper models cover signature and
quorum predicates. Instead it proves that the actor either verifies inline
when workers are unavailable, defers exactly one owned vote to worker state,
retains backpressured work for a later retry, ignores unmatched worker results,
drops stale/locked/penalized votes before consensus mutation, rejects invalid
signature results, and fails closed when the worker result channel is gone.
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
  addPending,
  \* @type: Bool;
  retainPending,
  \* @type: Bool;
  removeInflight,
  \* @type: Bool;
  clearInflight,
  \* @type: Bool;
  clearPending,
  \* @type: Bool;
  clearWorkers,
  \* @type: Bool;
  keepResultRx,
  \* @type: Bool;
  applyVote,
  \* @type: Bool;
  dropVote,
  \* @type: Bool;
  duplicateDrop,
  \* @type: Bool;
  invalidRejected

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    inlineVerify,
    deferWork,
    addInflight,
    addPending,
    retainPending,
    removeInflight,
    clearInflight,
    clearPending,
    clearWorkers,
    keepResultRx,
    applyVote,
    dropVote,
    duplicateDrop,
    invalidRejected>>

Cases == {
  "idle",
  "dispatch_no_workers_inline",
  "dispatch_duplicate_inflight",
  "dispatch_duplicate_pending",
  "dispatch_send_success",
  "dispatch_queue_full",
  "dispatch_disconnect_all_inline",
  "pending_no_workers",
  "pending_success",
  "pending_queue_full_keep",
  "poll_no_inflight",
  "poll_id_mismatch",
  "poll_stale_height_view",
  "poll_locked_precommit",
  "poll_penalized",
  "poll_invalid_signature",
  "poll_valid_signature",
  "poll_channel_disconnected",
  "poll_no_rx_dispatch_pending"
}

DispatchCases == {
  "dispatch_no_workers_inline",
  "dispatch_duplicate_inflight",
  "dispatch_duplicate_pending",
  "dispatch_send_success",
  "dispatch_queue_full",
  "dispatch_disconnect_all_inline"
}

DuplicateCases == {
  "dispatch_duplicate_inflight",
  "dispatch_duplicate_pending"
}

PendingDispatchCases == {
  "pending_no_workers",
  "pending_success",
  "pending_queue_full_keep"
}

NormalPollCases == {
  "poll_no_inflight",
  "poll_id_mismatch",
  "poll_stale_height_view",
  "poll_locked_precommit",
  "poll_penalized",
  "poll_invalid_signature",
  "poll_valid_signature"
}

PollResultWithInflightCases == {
  "poll_id_mismatch",
  "poll_stale_height_view",
  "poll_locked_precommit",
  "poll_penalized",
  "poll_invalid_signature",
  "poll_valid_signature"
}

RejectedPollCases == {
  "poll_stale_height_view",
  "poll_locked_precommit",
  "poll_penalized"
}

SpecInlineVerify(c) ==
  c \in {"dispatch_no_workers_inline", "dispatch_disconnect_all_inline"}

SpecAddInflight(c) ==
  c \in {"dispatch_send_success", "pending_success", "poll_no_rx_dispatch_pending"}

SpecAddPending(c) ==
  c = "dispatch_queue_full"

SpecRetainPending(c) ==
  c \in {"pending_no_workers", "pending_queue_full_keep"}

SpecDeferWork(c) ==
  SpecAddInflight(c) \/ SpecAddPending(c) \/ SpecRetainPending(c)

SpecRemoveInflight(c) ==
  c \in PollResultWithInflightCases

SpecClearInflight(c) ==
  c = "poll_channel_disconnected"

SpecClearPending(c) ==
  c = "poll_channel_disconnected"

SpecClearWorkers(c) ==
  c \in {"dispatch_disconnect_all_inline", "poll_channel_disconnected"}

SpecKeepResultRx(c) ==
  c \in NormalPollCases

SpecApplyVote(c) ==
  c = "poll_valid_signature"

SpecDropVote(c) ==
  c \in RejectedPollCases

SpecDuplicateDrop(c) ==
  c \in DuplicateCases

SpecInvalidRejected(c) ==
  c = "poll_invalid_signature"

ActualInlineVerify(c) ==
  CASE c = "dispatch_no_workers_inline" /\ Bug = "no_workers_no_inline" -> FALSE
    [] c = "dispatch_disconnect_all_inline" /\ Bug = "disconnect_all_no_inline" -> FALSE
    [] OTHER -> SpecInlineVerify(c)

ActualAddInflight(c) ==
  CASE c = "dispatch_no_workers_inline" /\ Bug = "no_workers_deferred" -> TRUE
    [] c = "dispatch_duplicate_inflight" /\ Bug = "duplicate_inflight_queues" -> TRUE
    [] c = "dispatch_duplicate_pending" /\ Bug = "duplicate_pending_queues" -> TRUE
    [] c = "dispatch_send_success" /\ Bug = "send_success_no_inflight" -> FALSE
    [] c = "dispatch_queue_full" /\ Bug = "queue_full_adds_inflight" -> TRUE
    [] c = "pending_success" /\ Bug = "pending_success_no_inflight" -> FALSE
    [] c = "poll_no_rx_dispatch_pending" /\ Bug = "poll_no_rx_skips_dispatch" -> FALSE
    [] OTHER -> SpecAddInflight(c)

ActualAddPending(c) ==
  CASE c = "dispatch_no_workers_inline" /\ Bug = "no_workers_deferred" -> TRUE
    [] c = "dispatch_duplicate_inflight" /\ Bug = "duplicate_inflight_queues" -> TRUE
    [] c = "dispatch_duplicate_pending" /\ Bug = "duplicate_pending_queues" -> TRUE
    [] c = "dispatch_queue_full" /\ Bug = "queue_full_drops" -> FALSE
    [] OTHER -> SpecAddPending(c)

ActualRetainPending(c) ==
  CASE c = "pending_no_workers" /\ Bug = "pending_no_workers_drops" -> FALSE
    [] c = "pending_success" /\ Bug = "pending_success_keeps_pending" -> TRUE
    [] c = "pending_queue_full_keep" /\ Bug = "pending_queue_full_drops" -> FALSE
    [] OTHER -> SpecRetainPending(c)

ActualDeferWork(c) ==
  ActualAddInflight(c) \/ ActualAddPending(c) \/ ActualRetainPending(c)

ActualRemoveInflight(c) ==
  CASE c = "poll_id_mismatch" /\ Bug = "poll_id_mismatch_keeps_inflight" -> FALSE
    [] OTHER -> SpecRemoveInflight(c)

ActualClearInflight(c) ==
  CASE c = "poll_channel_disconnected" /\ Bug = "poll_channel_disconnect_keeps_workers" -> FALSE
    [] OTHER -> SpecClearInflight(c)

ActualClearPending(c) ==
  CASE c = "poll_channel_disconnected" /\ Bug = "poll_channel_disconnect_keeps_pending" -> FALSE
    [] OTHER -> SpecClearPending(c)

ActualClearWorkers(c) ==
  CASE c = "dispatch_disconnect_all_inline" /\ Bug = "disconnect_all_keeps_workers" -> FALSE
    [] c = "poll_channel_disconnected" /\ Bug = "poll_channel_disconnect_keeps_workers" -> FALSE
    [] OTHER -> SpecClearWorkers(c)

ActualKeepResultRx(c) ==
  CASE c = "poll_channel_disconnected" /\ Bug = "poll_channel_disconnect_keeps_rx" -> TRUE
    [] c \in NormalPollCases /\ Bug = "normal_poll_drops_rx" -> FALSE
    [] OTHER -> SpecKeepResultRx(c)

ActualApplyVote(c) ==
  CASE c = "dispatch_send_success" /\ Bug = "send_success_applies" -> TRUE
    [] c = "poll_no_inflight" /\ Bug = "poll_no_inflight_applies" -> TRUE
    [] c = "poll_id_mismatch" /\ Bug = "poll_id_mismatch_applies" -> TRUE
    [] c = "poll_stale_height_view" /\ Bug = "poll_stale_applies" -> TRUE
    [] c = "poll_locked_precommit" /\ Bug = "poll_locked_applies" -> TRUE
    [] c = "poll_penalized" /\ Bug = "poll_penalized_applies" -> TRUE
    [] c = "poll_invalid_signature" /\ Bug = "poll_invalid_applies" -> TRUE
    [] c = "poll_valid_signature" /\ Bug = "poll_valid_no_apply" -> FALSE
    [] OTHER -> SpecApplyVote(c)

ActualDropVote(c) ==
  CASE c \in RejectedPollCases /\ Bug = "poll_drop_missing" -> FALSE
    [] OTHER -> SpecDropVote(c)

ActualDuplicateDrop(c) ==
  CASE c \in DuplicateCases /\ Bug = "duplicate_not_dropped" -> FALSE
    [] OTHER -> SpecDuplicateDrop(c)

ActualInvalidRejected(c) ==
  CASE c = "poll_invalid_signature" /\ Bug = "poll_invalid_not_rejected" -> FALSE
    [] OTHER -> SpecInvalidRejected(c)

BugModes == {
  "none",
  "no_workers_deferred",
  "no_workers_no_inline",
  "duplicate_inflight_queues",
  "duplicate_pending_queues",
  "duplicate_not_dropped",
  "send_success_no_inflight",
  "send_success_applies",
  "queue_full_drops",
  "queue_full_adds_inflight",
  "pending_no_workers_drops",
  "pending_success_no_inflight",
  "pending_success_keeps_pending",
  "pending_queue_full_drops",
  "disconnect_all_keeps_workers",
  "disconnect_all_no_inline",
  "poll_no_inflight_applies",
  "poll_id_mismatch_applies",
  "poll_id_mismatch_keeps_inflight",
  "poll_stale_applies",
  "poll_locked_applies",
  "poll_penalized_applies",
  "poll_invalid_applies",
  "poll_invalid_not_rejected",
  "poll_valid_no_apply",
  "poll_channel_disconnect_keeps_rx",
  "poll_channel_disconnect_keeps_workers",
  "poll_channel_disconnect_keeps_pending",
  "poll_no_rx_skips_dispatch",
  "normal_poll_drops_rx",
  "poll_drop_missing"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ inlineVerify \in BOOLEAN
  /\ deferWork \in BOOLEAN
  /\ addInflight \in BOOLEAN
  /\ addPending \in BOOLEAN
  /\ retainPending \in BOOLEAN
  /\ removeInflight \in BOOLEAN
  /\ clearInflight \in BOOLEAN
  /\ clearPending \in BOOLEAN
  /\ clearWorkers \in BOOLEAN
  /\ keepResultRx \in BOOLEAN
  /\ applyVote \in BOOLEAN
  /\ dropVote \in BOOLEAN
  /\ duplicateDrop \in BOOLEAN
  /\ invalidRejected \in BOOLEAN

Init ==
  /\ candidate = "idle"
  /\ inlineVerify = FALSE
  /\ deferWork = FALSE
  /\ addInflight = FALSE
  /\ addPending = FALSE
  /\ retainPending = FALSE
  /\ removeInflight = FALSE
  /\ clearInflight = FALSE
  /\ clearPending = FALSE
  /\ clearWorkers = FALSE
  /\ keepResultRx = FALSE
  /\ applyVote = FALSE
  /\ dropVote = FALSE
  /\ duplicateDrop = FALSE
  /\ invalidRejected = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ inlineVerify' = ActualInlineVerify(c)
  /\ deferWork' = ActualDeferWork(c)
  /\ addInflight' = ActualAddInflight(c)
  /\ addPending' = ActualAddPending(c)
  /\ retainPending' = ActualRetainPending(c)
  /\ removeInflight' = ActualRemoveInflight(c)
  /\ clearInflight' = ActualClearInflight(c)
  /\ clearPending' = ActualClearPending(c)
  /\ clearWorkers' = ActualClearWorkers(c)
  /\ keepResultRx' = ActualKeepResultRx(c)
  /\ applyVote' = ActualApplyVote(c)
  /\ dropVote' = ActualDropVote(c)
  /\ duplicateDrop' = ActualDuplicateDrop(c)
  /\ invalidRejected' = ActualInvalidRejected(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ inlineVerify = SpecInlineVerify(candidate)
  /\ deferWork = SpecDeferWork(candidate)
  /\ addInflight = SpecAddInflight(candidate)
  /\ addPending = SpecAddPending(candidate)
  /\ retainPending = SpecRetainPending(candidate)
  /\ removeInflight = SpecRemoveInflight(candidate)
  /\ clearInflight = SpecClearInflight(candidate)
  /\ clearPending = SpecClearPending(candidate)
  /\ clearWorkers = SpecClearWorkers(candidate)
  /\ keepResultRx = SpecKeepResultRx(candidate)
  /\ applyVote = SpecApplyVote(candidate)
  /\ dropVote = SpecDropVote(candidate)
  /\ duplicateDrop = SpecDuplicateDrop(candidate)
  /\ invalidRejected = SpecInvalidRejected(candidate)

NoWorkerDispatchFallsBackInline ==
  candidate = "dispatch_no_workers_inline" =>
    /\ inlineVerify
    /\ ~deferWork
    /\ ~addInflight
    /\ ~addPending
    /\ ~applyVote

DuplicateDispatchDoesNotQueue ==
  candidate \in DuplicateCases =>
    /\ duplicateDrop
    /\ ~addInflight
    /\ ~addPending
    /\ ~inlineVerify
    /\ ~applyVote

SuccessfulDispatchOwnsInflight ==
  candidate \in {"dispatch_send_success", "pending_success", "poll_no_rx_dispatch_pending"} =>
    /\ deferWork
    /\ addInflight
    /\ ~addPending
    /\ ~retainPending
    /\ ~applyVote

BackpressureRetainsOrQueuesWork ==
  candidate = "dispatch_queue_full" =>
    /\ deferWork
    /\ addPending
    /\ ~addInflight
    /\ ~applyVote

PendingRetryFailClosed ==
  candidate \in {"pending_no_workers", "pending_queue_full_keep"} =>
    /\ retainPending
    /\ deferWork
    /\ ~addInflight
    /\ ~applyVote

DisconnectedDispatchFallsBackInline ==
  candidate = "dispatch_disconnect_all_inline" =>
    /\ inlineVerify
    /\ clearWorkers
    /\ ~addInflight
    /\ ~addPending

PollResultRequiresInflight ==
  candidate = "poll_no_inflight" =>
    /\ ~removeInflight
    /\ ~applyVote
    /\ ~dropVote

IdMismatchDoesNotApply ==
  candidate = "poll_id_mismatch" =>
    /\ removeInflight
    /\ ~applyVote
    /\ ~dropVote

RejectedPollsDoNotApply ==
  candidate \in RejectedPollCases =>
    /\ removeInflight
    /\ dropVote
    /\ ~applyVote

InvalidSignatureDoesNotApply ==
  candidate = "poll_invalid_signature" =>
    /\ removeInflight
    /\ invalidRejected
    /\ ~applyVote
    /\ ~dropVote

ValidSignatureAppliesOnce ==
  candidate = "poll_valid_signature" =>
    /\ removeInflight
    /\ applyVote
    /\ ~dropVote
    /\ ~invalidRejected

NormalPollKeepsResultRx ==
  candidate \in NormalPollCases => keepResultRx

ChannelDisconnectFailsClosed ==
  candidate = "poll_channel_disconnected" =>
    /\ clearWorkers
    /\ clearInflight
    /\ clearPending
    /\ ~keepResultRx
    /\ ~applyVote

Safety ==
  /\ MatchesSpec
  /\ NoWorkerDispatchFallsBackInline
  /\ DuplicateDispatchDoesNotQueue
  /\ SuccessfulDispatchOwnsInflight
  /\ BackpressureRetainsOrQueuesWork
  /\ PendingRetryFailClosed
  /\ DisconnectedDispatchFallsBackInline
  /\ PollResultRequiresInflight
  /\ IdMismatchDoesNotApply
  /\ RejectedPollsDoNotApply
  /\ InvalidSignatureDoesNotApply
  /\ ValidSignatureAppliesOnce
  /\ NormalPollKeepsResultRx
  /\ ChannelDisconnectFailsClosed

=============================================================================
====
