---- MODULE SumeragiVNextSlotLifecycleGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for actor-owned vNext slot lifecycle transitions.

The live actor records non-canonical vNext diagnostic state around proposal,
availability, asynchronous validation, worker-result, timeout, and persisted
commit events. This gate checks that committed slots are sticky, validation
work is dispatched only from an installed non-committed slot with unqueued
validation state, matching worker events are the only events that mutate
validation ownership, valid/invalid worker results have the expected slot
effects, stale/terminal results are side-effect-free, and timeout recovery
fires only for due unprotected running/backpressured validation.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  slotState,
  \* @type: Str;
  validationState,
  \* @type: Bool;
  roundInstalled,
  \* @type: Bool;
  progress,
  \* @type: Bool;
  dispatchEffect,
  \* @type: Bool;
  acceptEffect,
  \* @type: Bool;
  rejectEffect,
  \* @type: Bool;
  recoveryEffect

\* @type: <<Str, Str, Str, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    slotState,
    validationState,
    roundInstalled,
    progress,
    dispatchEffect,
    acceptEffect,
    rejectEffect,
    recoveryEffect>>

Cases == {
  "proposal_no_base",
  "proposal_idle",
  "proposal_committed",
  "availability_no_base",
  "availability_idle",
  "availability_committed",
  "validation_no_base",
  "validation_unqueued_dispatch",
  "validation_committed",
  "worker_started_matching",
  "worker_started_stale",
  "queue_full_matching",
  "queue_full_stale",
  "result_valid_matching",
  "result_invalid_matching",
  "result_stale_wrong_owner",
  "result_terminal_committed",
  "defer_running",
  "defer_committed",
  "tick_running_before_timeout",
  "tick_running_due_unprotected",
  "tick_running_due_protected",
  "tick_backpressure_due_unprotected",
  "tick_backpressure_due_protected",
  "tick_terminal_committed",
  "tick_terminal_aborted",
  "commit_persisted_any",
  "commit_no_base"
}

SlotStates == {
  "none",
  "idle",
  "proposed",
  "awaiting_validation",
  "prepared",
  "committed",
  "recovering",
  "aborted"
}

ValidationStates == {
  "none",
  "unqueued",
  "queued",
  "running",
  "backpressured",
  "valid",
  "invalid"
}

NoBaseCases == {
  "proposal_no_base",
  "availability_no_base",
  "validation_no_base",
  "commit_no_base"
}

CommittedStickyCases == {
  "proposal_committed",
  "availability_committed",
  "validation_committed",
  "defer_committed",
  "result_terminal_committed",
  "tick_terminal_committed"
}

StaleWorkerCases == {
  "worker_started_stale",
  "queue_full_stale",
  "result_stale_wrong_owner",
  "result_terminal_committed"
}

RecoveryDueCases == {
  "tick_running_due_unprotected",
  "tick_backpressure_due_unprotected"
}

ProtectedTimeoutCases == {
  "tick_running_due_protected",
  "tick_backpressure_due_protected"
}

TerminalTickCases == {
  "tick_terminal_committed",
  "tick_terminal_aborted"
}

SpecRound(c) ==
  ~(c \in NoBaseCases)

SpecSlot(c) ==
  CASE c \in NoBaseCases -> "none"
    [] c = "proposal_idle" -> "proposed"
    [] c = "proposal_committed" -> "committed"
    [] c = "availability_idle" -> "awaiting_validation"
    [] c = "availability_committed" -> "committed"
    [] c = "validation_unqueued_dispatch" -> "awaiting_validation"
    [] c = "validation_committed" -> "committed"
    [] c = "worker_started_matching" -> "awaiting_validation"
    [] c = "worker_started_stale" -> "awaiting_validation"
    [] c = "queue_full_matching" -> "awaiting_validation"
    [] c = "queue_full_stale" -> "awaiting_validation"
    [] c = "result_valid_matching" -> "prepared"
    [] c = "result_invalid_matching" -> "aborted"
    [] c = "result_stale_wrong_owner" -> "awaiting_validation"
    [] c = "result_terminal_committed" -> "committed"
    [] c = "defer_running" -> "awaiting_validation"
    [] c = "defer_committed" -> "committed"
    [] c = "tick_running_before_timeout" -> "awaiting_validation"
    [] c = "tick_running_due_unprotected" -> "recovering"
    [] c = "tick_running_due_protected" -> "awaiting_validation"
    [] c = "tick_backpressure_due_unprotected" -> "recovering"
    [] c = "tick_backpressure_due_protected" -> "awaiting_validation"
    [] c = "tick_terminal_committed" -> "committed"
    [] c = "tick_terminal_aborted" -> "aborted"
    [] c = "commit_persisted_any" -> "committed"
    [] OTHER -> "none"

SpecValidation(c) ==
  CASE c \in NoBaseCases -> "none"
    [] c = "validation_unqueued_dispatch" -> "running"
    [] c = "worker_started_matching" -> "running"
    [] c = "worker_started_stale" -> "queued"
    [] c = "queue_full_matching" -> "backpressured"
    [] c = "queue_full_stale" -> "running"
    [] c = "result_valid_matching" -> "valid"
    [] c = "result_invalid_matching" -> "invalid"
    [] c = "defer_running" -> "unqueued"
    [] c = "commit_persisted_any" -> "running"
    [] c \in CommittedStickyCases -> "running"
    [] c = "tick_backpressure_due_unprotected" -> "backpressured"
    [] c = "tick_backpressure_due_protected" -> "backpressured"
    [] c = "result_stale_wrong_owner" -> "running"
    [] c = "tick_running_before_timeout" -> "running"
    [] c = "tick_running_due_unprotected" -> "running"
    [] c = "tick_running_due_protected" -> "running"
    [] c = "tick_terminal_aborted" -> "running"
    [] OTHER -> "unqueued"

SpecProgress(c) ==
  CASE c \in NoBaseCases -> FALSE
    [] c = "worker_started_stale" -> FALSE
    [] c = "queue_full_stale" -> FALSE
    [] c = "result_stale_wrong_owner" -> FALSE
    [] c = "result_terminal_committed" -> FALSE
    [] c = "defer_committed" -> FALSE
    [] c = "tick_running_before_timeout" -> FALSE
    [] c \in ProtectedTimeoutCases -> FALSE
    [] c \in TerminalTickCases -> FALSE
    [] OTHER -> TRUE

SpecDispatch(c) ==
  c = "validation_unqueued_dispatch"

SpecAccept(c) ==
  c = "result_valid_matching"

SpecReject(c) ==
  c = "result_invalid_matching"

SpecRecovery(c) ==
  c \in RecoveryDueCases

ActualSlot(c) ==
  CASE c = "proposal_committed" /\ Bug = "proposal_overwrites_committed" ->
      "proposed"
    [] c = "availability_committed" /\ Bug = "availability_overwrites_committed" ->
      "awaiting_validation"
    [] c = "validation_no_base" /\ Bug = "validation_dispatch_without_round" ->
      "awaiting_validation"
    [] c = "validation_committed" /\ Bug = "validation_dispatch_committed" ->
      "awaiting_validation"
    [] c = "validation_unqueued_dispatch" /\ Bug = "validation_skips_awaiting" ->
      "proposed"
    [] c = "result_valid_matching" /\ Bug = "valid_result_no_prepare" ->
      "awaiting_validation"
    [] c = "result_invalid_matching" /\ Bug = "invalid_result_no_abort" ->
      "awaiting_validation"
    [] c = "result_stale_wrong_owner" /\ Bug = "stale_result_mutates" ->
      "prepared"
    [] c = "result_terminal_committed" /\ Bug = "terminal_result_mutates" ->
      "prepared"
    [] c = "defer_committed" /\ Bug = "defer_committed_mutates" ->
      "awaiting_validation"
    [] c = "tick_running_before_timeout" /\ Bug = "timeout_before_due_recovers" ->
      "recovering"
    [] c = "tick_running_due_unprotected" /\ Bug = "timeout_due_no_recovery" ->
      "awaiting_validation"
    [] c = "tick_running_due_protected" /\ Bug = "timeout_protected_recovers" ->
      "recovering"
    [] c = "tick_terminal_committed" /\ Bug = "timeout_committed_recovers" ->
      "recovering"
    [] c = "tick_terminal_aborted" /\ Bug = "timeout_aborted_recovers" ->
      "recovering"
    [] c = "tick_backpressure_due_unprotected" /\ Bug = "backpressure_due_no_recovery" ->
      "awaiting_validation"
    [] c = "tick_backpressure_due_protected" /\ Bug = "backpressure_protected_recovers" ->
      "recovering"
    [] c = "commit_persisted_any" /\ Bug = "commit_not_sticky" ->
      "awaiting_validation"
    [] c = "proposal_no_base" /\ Bug = "install_without_base" ->
      "idle"
    [] c = "availability_no_base" /\ Bug = "install_without_base" ->
      "idle"
    [] c = "commit_no_base" /\ Bug = "install_without_base" ->
      "committed"
    [] OTHER -> SpecSlot(c)

ActualValidation(c) ==
  CASE c = "validation_no_base" /\ Bug = "validation_dispatch_without_round" ->
      "running"
    [] c = "validation_committed" /\ Bug = "validation_dispatch_committed" ->
      "running"
    [] c = "validation_unqueued_dispatch" /\ Bug = "validation_fails_to_run" ->
      "queued"
    [] c = "worker_started_stale" /\ Bug = "worker_started_wrong_owner" ->
      "running"
    [] c = "queue_full_matching" /\ Bug = "queue_full_keeps_queued" ->
      "queued"
    [] c = "queue_full_stale" /\ Bug = "queue_full_wrong_owner" ->
      "backpressured"
    [] c = "result_stale_wrong_owner" /\ Bug = "stale_result_mutates" ->
      "valid"
    [] c = "result_terminal_committed" /\ Bug = "terminal_result_mutates" ->
      "valid"
    [] c = "defer_running" /\ Bug = "defer_keeps_running" ->
      "running"
    [] c = "defer_committed" /\ Bug = "defer_committed_mutates" ->
      "unqueued"
    [] c = "proposal_no_base" /\ Bug = "install_without_base" ->
      "unqueued"
    [] c = "availability_no_base" /\ Bug = "install_without_base" ->
      "unqueued"
    [] c = "commit_no_base" /\ Bug = "install_without_base" ->
      "running"
    [] OTHER -> SpecValidation(c)

ActualRound(c) ==
  CASE c \in NoBaseCases /\ Bug = "install_without_base" -> TRUE
    [] c = "validation_no_base" /\ Bug = "validation_dispatch_without_round" -> TRUE
    [] OTHER -> SpecRound(c)

ActualProgress(c) ==
  CASE c \in NoBaseCases /\ Bug = "install_without_base" -> TRUE
    [] c = "validation_no_base" /\ Bug = "validation_dispatch_without_round" -> TRUE
    [] c = "validation_committed" /\ Bug = "validation_dispatch_committed" -> TRUE
    [] c = "worker_started_stale" /\ Bug = "worker_started_wrong_owner" -> TRUE
    [] c = "queue_full_stale" /\ Bug = "queue_full_wrong_owner" -> TRUE
    [] c = "result_stale_wrong_owner" /\ Bug = "stale_result_mutates" -> TRUE
    [] c = "result_terminal_committed" /\ Bug = "terminal_result_mutates" -> TRUE
    [] c = "defer_committed" /\ Bug = "defer_committed_mutates" -> TRUE
    [] c = "tick_running_before_timeout" /\ Bug = "timeout_before_due_recovers" -> TRUE
    [] c = "tick_running_due_unprotected" /\ Bug = "timeout_due_no_recovery" -> FALSE
    [] c = "tick_running_due_protected" /\ Bug = "timeout_protected_recovers" -> TRUE
    [] c = "tick_terminal_committed" /\ Bug = "timeout_committed_recovers" -> TRUE
    [] c = "tick_terminal_aborted" /\ Bug = "timeout_aborted_recovers" -> TRUE
    [] c = "tick_backpressure_due_unprotected" /\ Bug = "backpressure_due_no_recovery" -> FALSE
    [] c = "tick_backpressure_due_protected" /\ Bug = "backpressure_protected_recovers" -> TRUE
    [] c = "commit_persisted_any" /\ Bug = "commit_missing_progress" -> FALSE
    [] OTHER -> SpecProgress(c)

ActualDispatch(c) ==
  CASE c = "validation_no_base" /\ Bug = "validation_dispatch_without_round" -> TRUE
    [] c = "validation_committed" /\ Bug = "validation_dispatch_committed" -> TRUE
    [] c = "validation_unqueued_dispatch" /\ Bug = "validation_fails_to_run" -> FALSE
    [] c \in RecoveryDueCases /\ Bug = "recovery_dispatches_worker" -> TRUE
    [] OTHER -> SpecDispatch(c)

ActualAccept(c) ==
  CASE c = "result_valid_matching" /\ Bug = "valid_result_no_accept" -> FALSE
    [] c = "result_invalid_matching" /\ Bug = "accept_without_valid" -> TRUE
    [] c = "result_stale_wrong_owner" /\ Bug = "stale_result_mutates" -> TRUE
    [] c = "result_terminal_committed" /\ Bug = "terminal_result_mutates" -> TRUE
    [] c \in RecoveryDueCases /\ Bug = "recovery_accepts" -> TRUE
    [] OTHER -> SpecAccept(c)

ActualReject(c) ==
  CASE c = "result_invalid_matching" /\ Bug = "invalid_result_no_reject" -> FALSE
    [] c = "result_valid_matching" /\ Bug = "reject_without_invalid" -> TRUE
    [] c \in RecoveryDueCases /\ Bug = "recovery_rejects" -> TRUE
    [] OTHER -> SpecReject(c)

ActualRecovery(c) ==
  CASE c = "tick_running_before_timeout" /\ Bug = "timeout_before_due_recovers" -> TRUE
    [] c = "tick_running_due_unprotected" /\ Bug = "timeout_due_no_recovery" -> FALSE
    [] c = "tick_running_due_protected" /\ Bug = "timeout_protected_recovers" -> TRUE
    [] c = "tick_terminal_committed" /\ Bug = "timeout_committed_recovers" -> TRUE
    [] c = "tick_terminal_aborted" /\ Bug = "timeout_aborted_recovers" -> TRUE
    [] c = "tick_backpressure_due_unprotected" /\ Bug = "backpressure_due_no_recovery" -> FALSE
    [] c = "tick_backpressure_due_protected" /\ Bug = "backpressure_protected_recovers" -> TRUE
    [] OTHER -> SpecRecovery(c)

BugModes == {
  "none",
  "install_without_base",
  "proposal_overwrites_committed",
  "availability_overwrites_committed",
  "validation_dispatch_without_round",
  "validation_dispatch_committed",
  "validation_skips_awaiting",
  "validation_fails_to_run",
  "worker_started_wrong_owner",
  "queue_full_wrong_owner",
  "queue_full_keeps_queued",
  "valid_result_no_prepare",
  "valid_result_no_accept",
  "invalid_result_no_abort",
  "invalid_result_no_reject",
  "accept_without_valid",
  "reject_without_invalid",
  "stale_result_mutates",
  "terminal_result_mutates",
  "defer_committed_mutates",
  "defer_keeps_running",
  "timeout_before_due_recovers",
  "timeout_due_no_recovery",
  "timeout_protected_recovers",
  "timeout_committed_recovers",
  "timeout_aborted_recovers",
  "backpressure_due_no_recovery",
  "backpressure_protected_recovers",
  "commit_not_sticky",
  "commit_missing_progress",
  "recovery_dispatches_worker",
  "recovery_accepts",
  "recovery_rejects"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases \union {"none"}
  /\ slotState \in SlotStates
  /\ validationState \in ValidationStates
  /\ roundInstalled \in BOOLEAN
  /\ progress \in BOOLEAN
  /\ dispatchEffect \in BOOLEAN
  /\ acceptEffect \in BOOLEAN
  /\ rejectEffect \in BOOLEAN
  /\ recoveryEffect \in BOOLEAN

Init ==
  /\ candidate = "none"
  /\ slotState = "none"
  /\ validationState = "none"
  /\ roundInstalled = FALSE
  /\ progress = FALSE
  /\ dispatchEffect = FALSE
  /\ acceptEffect = FALSE
  /\ rejectEffect = FALSE
  /\ recoveryEffect = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ slotState' = ActualSlot(c)
  /\ validationState' = ActualValidation(c)
  /\ roundInstalled' = ActualRound(c)
  /\ progress' = ActualProgress(c)
  /\ dispatchEffect' = ActualDispatch(c)
  /\ acceptEffect' = ActualAccept(c)
  /\ rejectEffect' = ActualReject(c)
  /\ recoveryEffect' = ActualRecovery(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  candidate = "none" \/
    /\ slotState = SpecSlot(candidate)
    /\ validationState = SpecValidation(candidate)
    /\ roundInstalled = SpecRound(candidate)
    /\ progress = SpecProgress(candidate)
    /\ dispatchEffect = SpecDispatch(candidate)
    /\ acceptEffect = SpecAccept(candidate)
    /\ rejectEffect = SpecReject(candidate)
    /\ recoveryEffect = SpecRecovery(candidate)

NoBaseNeverInstallsOrProgresses ==
  candidate \in NoBaseCases =>
    /\ ~roundInstalled
    /\ ~progress
    /\ slotState = "none"
    /\ validationState = "none"

CommittedSlotsAreSticky ==
  candidate \in CommittedStickyCases =>
    /\ slotState = "committed"
    /\ ~acceptEffect
    /\ ~rejectEffect
    /\ ~recoveryEffect

ValidationDispatchRequiresInstalledNonCommittedSlot ==
  dispatchEffect =>
    /\ candidate = "validation_unqueued_dispatch"
    /\ roundInstalled
    /\ slotState = "awaiting_validation"
    /\ validationState = "running"

MatchingWorkerStartOnlyMutatesQueued ==
  candidate = "worker_started_matching" =>
    /\ validationState = "running"
    /\ progress

StaleWorkerEventsAreSideEffectFree ==
  candidate \in StaleWorkerCases =>
    /\ ~progress
    /\ ~dispatchEffect
    /\ ~acceptEffect
    /\ ~rejectEffect
    /\ ~recoveryEffect

QueueFullMatchingBackpressures ==
  candidate = "queue_full_matching" =>
    /\ validationState = "backpressured"
    /\ progress

ValidResultPreparesAndAccepts ==
  candidate = "result_valid_matching" =>
    /\ slotState = "prepared"
    /\ validationState = "valid"
    /\ acceptEffect
    /\ ~rejectEffect

InvalidResultAbortsAndRejects ==
  candidate = "result_invalid_matching" =>
    /\ slotState = "aborted"
    /\ validationState = "invalid"
    /\ rejectEffect
    /\ ~acceptEffect

DeferResetsOnlyNonCommittedSlots ==
  /\ (candidate = "defer_running" =>
        /\ slotState = "awaiting_validation"
        /\ validationState = "unqueued"
        /\ progress)
  /\ (candidate = "defer_committed" =>
        /\ slotState = "committed"
        /\ validationState = "running"
        /\ ~progress)

RecoveryRequiresDueUnprotectedTimeout ==
  recoveryEffect => candidate \in RecoveryDueCases

DueUnprotectedTimeoutsRecover ==
  candidate \in RecoveryDueCases =>
    /\ slotState = "recovering"
    /\ recoveryEffect
    /\ progress

ProtectedTimeoutsDoNotRecover ==
  candidate \in ProtectedTimeoutCases =>
    /\ slotState = "awaiting_validation"
    /\ ~recoveryEffect
    /\ ~progress

TerminalTicksDoNotRecover ==
  candidate \in TerminalTickCases =>
    /\ ~recoveryEffect
    /\ ~progress

CommitPersistedCommits ==
  candidate = "commit_persisted_any" =>
    /\ slotState = "committed"
    /\ progress

RecoveryDoesNotEmitValidationResultEffects ==
  recoveryEffect =>
    /\ ~dispatchEffect
    /\ ~acceptEffect
    /\ ~rejectEffect

VNextSlotLifecycleExactness ==
  /\ MatchesSpec
  /\ NoBaseNeverInstallsOrProgresses
  /\ CommittedSlotsAreSticky
  /\ ValidationDispatchRequiresInstalledNonCommittedSlot
  /\ MatchingWorkerStartOnlyMutatesQueued
  /\ StaleWorkerEventsAreSideEffectFree
  /\ QueueFullMatchingBackpressures
  /\ ValidResultPreparesAndAccepts
  /\ InvalidResultAbortsAndRejects
  /\ DeferResetsOnlyNonCommittedSlots
  /\ RecoveryRequiresDueUnprotectedTimeout
  /\ DueUnprotectedTimeoutsRecover
  /\ ProtectedTimeoutsDoNotRecover
  /\ TerminalTicksDoNotRecover
  /\ CommitPersistedCommits
  /\ RecoveryDoesNotEmitValidationResultEffects

Safety ==
  VNextSlotLifecycleExactness

VNextSlotLifecycleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextSlotLifecycleExactness

SafetyFast ==
  VNextSlotLifecycleExactness

====
