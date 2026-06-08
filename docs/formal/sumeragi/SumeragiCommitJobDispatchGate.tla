---- MODULE SumeragiCommitJobDispatchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for commit-job dispatch.

This slice models the dispatch contract in `Actor::start_commit_job(...)`.
The actor must not start a second job while one is already inflight, queue-full
worker sends must leave the candidate pending for retry, healthy worker sends
must hand ownership to the worker without executing on the actor thread, and
missing or disconnected worker channels must fall back to inline execution
without leaving a worker-owned inflight marker behind.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  job_started,
  \* @type: Bool;
  worker_enqueued,
  \* @type: Bool;
  inline_executed,
  \* @type: Bool;
  pending_retained,
  \* @type: Bool;
  inflight_set,
  \* @type: Bool;
  existing_inflight_preserved,
  \* @type: Bool;
  worker_state_cleared,
  \* @type: Bool;
  commit_start_recorded,
  \* @type: Bool;
  actor_thread_state_advanced,
  \* @type: Bool;
  return_true,
  \* @type: Bool;
  return_false,
  \* @type: Bool;
  block_recoverable

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, job_started, worker_enqueued, inline_executed,
  pending_retained, inflight_set, existing_inflight_preserved,
  worker_state_cleared, commit_start_recorded, actor_thread_state_advanced,
  return_true, return_false, block_recoverable>>

Cases == {
  "existing_same_block",
  "existing_other_block",
  "worker_ready_enqueued",
  "worker_queue_full",
  "worker_disconnected_inline_committed",
  "worker_disconnected_inline_retry",
  "missing_work_tx_inline_committed",
  "missing_work_tx_inline_retry",
  "missing_result_rx_inline_committed",
  "missing_result_rx_inline_retry",
  "missing_both_worker_ends_inline_committed",
  "missing_both_worker_ends_inline_retry"
}

ExistingInflightCases == {"existing_same_block", "existing_other_block"}
ExistingSameBlockCases == {"existing_same_block"}
ExistingOtherBlockCases == {"existing_other_block"}
WorkerReadyCases == {"worker_ready_enqueued"}
QueueFullCases == {"worker_queue_full"}
DisconnectedCases == {
  "worker_disconnected_inline_committed",
  "worker_disconnected_inline_retry"
}
MissingWorkTxCases == {
  "missing_work_tx_inline_committed",
  "missing_work_tx_inline_retry",
  "missing_both_worker_ends_inline_committed",
  "missing_both_worker_ends_inline_retry"
}
MissingResultRxCases == {
  "missing_result_rx_inline_committed",
  "missing_result_rx_inline_retry",
  "missing_both_worker_ends_inline_committed",
  "missing_both_worker_ends_inline_retry"
}
MissingWorkerCases == MissingWorkTxCases \union MissingResultRxCases
InlineCases == DisconnectedCases \union MissingWorkerCases
InlineCommittedCases == {
  "worker_disconnected_inline_committed",
  "missing_work_tx_inline_committed",
  "missing_result_rx_inline_committed",
  "missing_both_worker_ends_inline_committed"
}
InlineRetryCases == {
  "worker_disconnected_inline_retry",
  "missing_work_tx_inline_retry",
  "missing_result_rx_inline_retry",
  "missing_both_worker_ends_inline_retry"
}
DisconnectedRetryCases == {"worker_disconnected_inline_retry"}
MissingWorkerRetryCases == InlineRetryCases \ DisconnectedRetryCases

SpecJobStarted(c) == c \in WorkerReadyCases \union InlineCases
SpecWorkerEnqueued(c) == c \in WorkerReadyCases
SpecInlineExecuted(c) == c \in InlineCases
SpecPendingRetained(c) ==
  \/ c \in ExistingOtherBlockCases
  \/ c \in QueueFullCases
  \/ c \in InlineRetryCases
SpecInflightSet(c) == c \in WorkerReadyCases
SpecExistingInflightPreserved(c) == c \in ExistingInflightCases
SpecWorkerStateCleared(c) == c \in DisconnectedCases
SpecCommitStartRecorded(c) == c \in WorkerReadyCases \union InlineCases
SpecActorThreadStateAdvanced(c) == c \in InlineCommittedCases
SpecReturnTrue(c) == c \in WorkerReadyCases \union InlineCommittedCases
SpecReturnFalse(c) == ~SpecReturnTrue(c)
SpecBlockRecoverable(c) == TRUE

ActualJobStarted(c) ==
  \/ /\ SpecJobStarted(c)
     /\ ~(Bug = "worker_ready_skips_enqueue" /\ c \in WorkerReadyCases)
     /\ ~(Bug = "disconnected_skips_inline" /\ c \in DisconnectedCases)
     /\ ~(Bug = "missing_worker_inline_skipped" /\ c \in MissingWorkerCases)
  \/ /\ c \in ExistingSameBlockCases
     /\ Bug = "same_block_starts_second_job"
  \/ /\ c \in ExistingOtherBlockCases
     /\ Bug = "other_block_overwrites_inflight"
  \/ /\ c \in QueueFullCases
     /\ Bug = "queue_full_enqueues"

ActualWorkerEnqueued(c) ==
  \/ /\ SpecWorkerEnqueued(c)
     /\ Bug # "worker_ready_skips_enqueue"
  \/ /\ c \in ExistingSameBlockCases
     /\ Bug = "same_block_starts_second_job"
  \/ /\ c \in ExistingOtherBlockCases
     /\ Bug = "other_block_overwrites_inflight"
  \/ /\ c \in QueueFullCases
     /\ Bug = "queue_full_enqueues"
  \/ /\ c \in MissingWorkTxCases
     /\ Bug = "missing_work_tx_tries_enqueue"
  \/ /\ c \in MissingResultRxCases
     /\ Bug = "missing_result_rx_tries_enqueue"
  \/ /\ c \in InlineCases
     /\ Bug = "enqueue_and_inline_same_job"

ActualInlineExecuted(c) ==
  \/ /\ SpecInlineExecuted(c)
     /\ ~(Bug = "disconnected_skips_inline" /\ c \in DisconnectedCases)
     /\ ~(Bug = "missing_worker_inline_skipped" /\ c \in MissingWorkerCases)
  \/ /\ c \in WorkerReadyCases
     /\ Bug = "worker_ready_executes_inline"
  \/ /\ c \in WorkerReadyCases
     /\ Bug = "enqueue_and_inline_same_job"

ActualPendingRetained(c) ==
  \/ /\ SpecPendingRetained(c)
     /\ ~(Bug = "other_block_dropped" /\ c \in ExistingOtherBlockCases)
     /\ ~(Bug = "queue_full_drops_pending" /\ c \in QueueFullCases)
     /\ ~(Bug = "disconnected_drops_unrecoverable"
          /\ c \in DisconnectedRetryCases)
     /\ ~(Bug = "inline_drops_unrecoverable"
          /\ c \in MissingWorkerRetryCases)
     /\ ~(Bug = "disconnected_skips_inline"
          /\ c \in DisconnectedRetryCases)
     /\ ~(Bug = "missing_worker_inline_skipped"
          /\ c \in MissingWorkerRetryCases)
  \/ /\ c \in ExistingSameBlockCases
     /\ Bug = "same_block_requeued"
  \/ /\ c \in WorkerReadyCases
     /\ Bug = "inflight_and_pending_same_job"

ActualInflightSet(c) ==
  \/ /\ SpecInflightSet(c)
     /\ Bug # "worker_ready_skips_inflight"
  \/ /\ c \in ExistingSameBlockCases
     /\ Bug = "same_block_starts_second_job"
  \/ /\ c \in ExistingOtherBlockCases
     /\ Bug = "other_block_overwrites_inflight"
  \/ /\ c \in QueueFullCases
     /\ Bug = "queue_full_sets_inflight"
  \/ /\ c \in DisconnectedCases
     /\ Bug = "disconnected_sets_worker_inflight"
  \/ /\ c \in InlineCases
     /\ Bug = "inline_sets_worker_inflight"

ActualExistingInflightPreserved(c) ==
  /\ SpecExistingInflightPreserved(c)
  /\ ~(Bug = "same_block_starts_second_job" /\ c \in ExistingSameBlockCases)
  /\ ~(Bug = "other_block_overwrites_inflight" /\ c \in ExistingOtherBlockCases)

ActualWorkerStateCleared(c) ==
  \/ /\ SpecWorkerStateCleared(c)
     /\ ~(Bug = "disconnected_keeps_worker_state" /\ c \in DisconnectedCases)
  \/ /\ c \in MissingWorkerCases
     /\ Bug = "missing_worker_clears_state"

ActualCommitStartRecorded(c) ==
  /\ SpecCommitStartRecorded(c)
  /\ ~(Bug = "worker_ready_skips_start_record" /\ c \in WorkerReadyCases)
  /\ ~(Bug = "disconnected_skips_inline" /\ c \in DisconnectedCases)
  /\ ~(Bug = "missing_worker_inline_skipped" /\ c \in MissingWorkerCases)

ActualActorThreadStateAdvanced(c) ==
  \/ /\ SpecActorThreadStateAdvanced(c)
     /\ ~(Bug = "disconnected_skips_inline" /\ c \in DisconnectedCases)
     /\ ~(Bug = "missing_worker_inline_skipped" /\ c \in MissingWorkerCases)
  \/ /\ c \in WorkerReadyCases
     /\ Bug = "worker_ready_executes_inline"

ActualReturnTrue(c) ==
  \/ /\ SpecReturnTrue(c)
     /\ ~(Bug = "worker_ready_returns_false" /\ c \in WorkerReadyCases)
     /\ ~(Bug = "start_without_return_true" /\ c \in WorkerReadyCases)
  \/ /\ c \in ExistingSameBlockCases
     /\ Bug = "same_block_starts_second_job"
  \/ /\ c \in ExistingOtherBlockCases
     /\ Bug = "other_block_overwrites_inflight"
  \/ /\ c \in QueueFullCases
     /\ Bug = "queue_full_returns_true"

ActualReturnFalse(c) == ~ActualReturnTrue(c)

ActualBlockRecoverable(c) ==
  \/ ActualExistingInflightPreserved(c)
  \/ ActualWorkerEnqueued(c)
  \/ ActualPendingRetained(c)
  \/ ActualActorThreadStateAdvanced(c)

Init ==
  /\ candidate = "none"
  /\ job_started = FALSE
  /\ worker_enqueued = FALSE
  /\ inline_executed = FALSE
  /\ pending_retained = FALSE
  /\ inflight_set = FALSE
  /\ existing_inflight_preserved = FALSE
  /\ worker_state_cleared = FALSE
  /\ commit_start_recorded = FALSE
  /\ actor_thread_state_advanced = FALSE
  /\ return_true = FALSE
  /\ return_false = FALSE
  /\ block_recoverable = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ job_started' = ActualJobStarted(c)
  /\ worker_enqueued' = ActualWorkerEnqueued(c)
  /\ inline_executed' = ActualInlineExecuted(c)
  /\ pending_retained' = ActualPendingRetained(c)
  /\ inflight_set' = ActualInflightSet(c)
  /\ existing_inflight_preserved' = ActualExistingInflightPreserved(c)
  /\ worker_state_cleared' = ActualWorkerStateCleared(c)
  /\ commit_start_recorded' = ActualCommitStartRecorded(c)
  /\ actor_thread_state_advanced' = ActualActorThreadStateAdvanced(c)
  /\ return_true' = ActualReturnTrue(c)
  /\ return_false' = ActualReturnFalse(c)
  /\ block_recoverable' = ActualBlockRecoverable(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ job_started \in BOOLEAN
  /\ worker_enqueued \in BOOLEAN
  /\ inline_executed \in BOOLEAN
  /\ pending_retained \in BOOLEAN
  /\ inflight_set \in BOOLEAN
  /\ existing_inflight_preserved \in BOOLEAN
  /\ worker_state_cleared \in BOOLEAN
  /\ commit_start_recorded \in BOOLEAN
  /\ actor_thread_state_advanced \in BOOLEAN
  /\ return_true \in BOOLEAN
  /\ return_false \in BOOLEAN
  /\ block_recoverable \in BOOLEAN

JobStartedMatchesSpec ==
  candidate = "none" \/ job_started = SpecJobStarted(candidate)

WorkerEnqueuedMatchesSpec ==
  candidate = "none" \/ worker_enqueued = SpecWorkerEnqueued(candidate)

InlineExecutionMatchesSpec ==
  candidate = "none" \/ inline_executed = SpecInlineExecuted(candidate)

PendingRetentionMatchesSpec ==
  candidate = "none" \/ pending_retained = SpecPendingRetained(candidate)

InflightSetMatchesSpec ==
  candidate = "none" \/ inflight_set = SpecInflightSet(candidate)

ExistingInflightPreservationMatchesSpec ==
  candidate = "none" \/
    existing_inflight_preserved = SpecExistingInflightPreserved(candidate)

WorkerStateClearMatchesSpec ==
  candidate = "none" \/ worker_state_cleared = SpecWorkerStateCleared(candidate)

CommitStartMatchesSpec ==
  candidate = "none" \/ commit_start_recorded = SpecCommitStartRecorded(candidate)

ActorThreadAdvanceMatchesSpec ==
  candidate = "none" \/
    actor_thread_state_advanced = SpecActorThreadStateAdvanced(candidate)

ReturnValueMatchesSpec ==
  candidate = "none" \/
    /\ return_true = SpecReturnTrue(candidate)
    /\ return_false = SpecReturnFalse(candidate)
    /\ return_true # return_false

BlockRecoveryMatchesSpec ==
  candidate = "none" \/ block_recoverable = SpecBlockRecoverable(candidate)

ExistingSameBlockSuppressesSecondDispatch ==
  candidate \in ExistingSameBlockCases =>
    /\ existing_inflight_preserved
    /\ ~job_started
    /\ ~worker_enqueued
    /\ ~inline_executed
    /\ ~pending_retained
    /\ ~inflight_set
    /\ return_false

ExistingOtherBlockKeepsNewPendingAndCurrentInflight ==
  candidate \in ExistingOtherBlockCases =>
    /\ existing_inflight_preserved
    /\ pending_retained
    /\ ~job_started
    /\ ~worker_enqueued
    /\ ~inline_executed
    /\ ~inflight_set
    /\ return_false

WorkerReadySuccessUsesWorkerOnly ==
  candidate \in WorkerReadyCases =>
    /\ job_started
    /\ worker_enqueued
    /\ inflight_set
    /\ commit_start_recorded
    /\ return_true
    /\ ~inline_executed
    /\ ~actor_thread_state_advanced
    /\ ~pending_retained

QueueFullKeepsPendingAndDoesNotOwn ==
  candidate \in QueueFullCases =>
    /\ pending_retained
    /\ return_false
    /\ ~job_started
    /\ ~worker_enqueued
    /\ ~inline_executed
    /\ ~inflight_set
    /\ ~commit_start_recorded

DisconnectedFallbackClearsWorkerAndRunsInline ==
  candidate \in DisconnectedCases =>
    /\ job_started
    /\ inline_executed
    /\ worker_state_cleared
    /\ commit_start_recorded
    /\ ~worker_enqueued
    /\ ~inflight_set

MissingWorkerRunsInlineWithoutClearingWorkerState ==
  candidate \in MissingWorkerCases =>
    /\ job_started
    /\ inline_executed
    /\ commit_start_recorded
    /\ ~worker_enqueued
    /\ ~worker_state_cleared
    /\ ~inflight_set

InlineLeavesNoWorkerInflight ==
  candidate \in InlineCases => ~inflight_set

WorkerQueueNeedsBothChannels ==
  candidate \in MissingWorkerCases => ~worker_enqueued

WorkerStateClearedOnlyOnDisconnected ==
  worker_state_cleared => candidate \in DisconnectedCases

CommitStartRecordedForStartedWork ==
  job_started => commit_start_recorded

CommitStartRequiresOwnedExecution ==
  commit_start_recorded => worker_enqueued \/ inline_executed

ActorThreadAdvancesOnlyForInlineCommit ==
  actor_thread_state_advanced =>
    /\ inline_executed
    /\ candidate \in InlineCommittedCases

ReturnTrueRequiresStartedWork ==
  return_true => job_started

JobOwnersAreExclusive ==
  /\ ~(worker_enqueued /\ inline_executed)
  /\ ~(inflight_set /\ pending_retained)
  /\ ~(worker_enqueued /\ pending_retained)

EveryDispatchLeavesBlockRecoverable ==
  candidate # "none" => block_recoverable

CommitJobDispatchProgressExact ==
  /\ JobStartedMatchesSpec
  /\ CommitStartMatchesSpec
  /\ ActorThreadAdvanceMatchesSpec
  /\ ReturnValueMatchesSpec

CommitJobDispatchOwnershipExact ==
  /\ WorkerEnqueuedMatchesSpec
  /\ InlineExecutionMatchesSpec
  /\ InflightSetMatchesSpec
  /\ ExistingInflightPreservationMatchesSpec
  /\ WorkerStateClearMatchesSpec

CommitJobDispatchRetentionExact ==
  /\ PendingRetentionMatchesSpec
  /\ BlockRecoveryMatchesSpec

CommitJobDispatchScenarioExact ==
  /\ ExistingSameBlockSuppressesSecondDispatch
  /\ ExistingOtherBlockKeepsNewPendingAndCurrentInflight
  /\ WorkerReadySuccessUsesWorkerOnly
  /\ QueueFullKeepsPendingAndDoesNotOwn
  /\ DisconnectedFallbackClearsWorkerAndRunsInline
  /\ MissingWorkerRunsInlineWithoutClearingWorkerState

CommitJobDispatchStructuralExact ==
  /\ InlineLeavesNoWorkerInflight
  /\ WorkerQueueNeedsBothChannels
  /\ WorkerStateClearedOnlyOnDisconnected
  /\ CommitStartRecordedForStartedWork
  /\ CommitStartRequiresOwnedExecution
  /\ ActorThreadAdvancesOnlyForInlineCommit
  /\ ReturnTrueRequiresStartedWork
  /\ JobOwnersAreExclusive
  /\ EveryDispatchLeavesBlockRecoverable

CommitJobDispatchExactness ==
  /\ CommitJobDispatchProgressExact
  /\ CommitJobDispatchOwnershipExact
  /\ CommitJobDispatchRetentionExact
  /\ CommitJobDispatchScenarioExact
  /\ CommitJobDispatchStructuralExact

Safety ==
  CommitJobDispatchExactness

=============================================================================
====
