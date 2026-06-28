---- MODULE SumeragiVNextValidationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for quarantined vNext validation ownership.

The Rust helpers in `ValidationState` map one pending block state to a
scheduling decision, record worker ownership when validation starts, and apply
only the worker result whose `(id, generation)` still matches the current
running owner. Timeout checks use saturating elapsed milliseconds, so a clock
sample before the recorded start time must not underflow into an immediate
performance suspicion.

This model checks representative one-step cases for dispatch/await,
running-timeout and backpressure-timeout boundaries, terminal accept/reject
decisions, worker-start ownership recording, matching result application, and
stale result rejection.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugDispatchQueued,
  \* @type: Bool;
  BugRaiseRunningBeforeTimeout,
  \* @type: Bool;
  BugMissRunningAtTimeout,
  \* @type: Bool;
  BugBackpressureBeforeTimeoutRaises,
  \* @type: Bool;
  BugMissBackpressureAtTimeout,
  \* @type: Bool;
  BugAcceptValidAsAwait,
  \* @type: Bool;
  BugRejectInvalidAsAwait,
  \* @type: Bool;
  BugUnderflowElapsed,
  \* @type: Bool;
  BugWorkerStartedKeepsQueued,
  \* @type: Bool;
  BugApplyWrongId,
  \* @type: Bool;
  BugApplyWrongGeneration,
  \* @type: Bool;
  BugApplyNotRunning,
  \* @type: Bool;
  BugIgnoreMatchingValid,
  \* @type: Bool;
  BugIgnoreMatchingInvalid,
  \* @type: Bool;
  BugStaleMutatesState

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  decision,
  \* @type: Str;
  nextState,
  \* @type: Str;
  action

\* @type: <<Str, Str, Str, Str>>;
vars == <<candidate, decision, nextState, action>>

Timeout == 2

Cases == {
  "unqueued_dispatch",
  "queued_await",
  "running_before_timeout",
  "running_at_timeout",
  "running_after_timeout",
  "running_now_before_started",
  "backpressured_before_timeout",
  "backpressured_at_timeout",
  "backpressured_after_timeout",
  "valid_accept",
  "invalid_reject",
  "worker_started_records_owner",
  "worker_result_valid_matching",
  "worker_result_invalid_matching",
  "worker_result_wrong_id",
  "worker_result_wrong_generation",
  "worker_result_not_running"
}

RunningDecisionCases == {
  "running_before_timeout",
  "running_at_timeout",
  "running_after_timeout",
  "running_now_before_started"
}

BackpressureDecisionCases == {
  "backpressured_before_timeout",
  "backpressured_at_timeout",
  "backpressured_after_timeout"
}

TerminalDecisionCases == {"valid_accept", "invalid_reject"}

MatchingWorkerResultCases == {
  "worker_result_valid_matching",
  "worker_result_invalid_matching"
}

StaleWorkerResultCases == {
  "worker_result_wrong_id",
  "worker_result_wrong_generation",
  "worker_result_not_running"
}

WorkerResultCases == MatchingWorkerResultCases \union StaleWorkerResultCases

StateBefore(c) ==
  CASE c = "unqueued_dispatch" -> "Unqueued"
    [] c \in {"queued_await", "worker_started_records_owner"} -> "Queued"
    [] c \in RunningDecisionCases -> "Running"
    [] c \in {"worker_result_valid_matching",
               "worker_result_invalid_matching",
               "worker_result_wrong_id",
               "worker_result_wrong_generation"} -> "Running"
    [] c \in BackpressureDecisionCases -> "Backpressured"
    [] c = "valid_accept" -> "Valid"
    [] c = "invalid_reject" -> "Invalid"
    [] c = "worker_result_not_running" -> "Queued"
    [] OTHER -> "None"

Elapsed(c) ==
  CASE c \in {"running_before_timeout", "backpressured_before_timeout"} -> 1
    [] c \in {"running_at_timeout", "backpressured_at_timeout"} -> Timeout
    [] c \in {"running_after_timeout", "backpressured_after_timeout"} -> 3
    [] c = "running_now_before_started" -> 0
    [] OTHER -> 0

SpecDecision(c) ==
  CASE c = "unqueued_dispatch" -> "DispatchWorker"
    [] c = "queued_await" -> "AwaitWorker"
    [] c \in RunningDecisionCases ->
      IF Elapsed(c) >= Timeout THEN "RaiseSuspicion" ELSE "AwaitWorker"
    [] c \in BackpressureDecisionCases ->
      IF Elapsed(c) >= Timeout THEN "RaiseSuspicion" ELSE "Backpressure"
    [] c = "valid_accept" -> "Accept"
    [] c = "invalid_reject" -> "Reject"
    [] OTHER -> "NoDecision"

ActualDecision(c) ==
  CASE c = "queued_await" /\ BugDispatchQueued -> "DispatchWorker"
    [] c = "running_before_timeout" /\ BugRaiseRunningBeforeTimeout ->
      "RaiseSuspicion"
    [] c = "running_at_timeout" /\ BugMissRunningAtTimeout -> "AwaitWorker"
    [] c = "running_now_before_started" /\ BugUnderflowElapsed ->
      "RaiseSuspicion"
    [] c = "backpressured_before_timeout" /\
          BugBackpressureBeforeTimeoutRaises -> "RaiseSuspicion"
    [] c = "backpressured_at_timeout" /\ BugMissBackpressureAtTimeout ->
      "Backpressure"
    [] c = "valid_accept" /\ BugAcceptValidAsAwait -> "AwaitWorker"
    [] c = "invalid_reject" /\ BugRejectInvalidAsAwait -> "AwaitWorker"
    [] OTHER -> SpecDecision(c)

SpecAction(c) ==
  CASE c = "worker_started_records_owner" -> "Started"
    [] c \in MatchingWorkerResultCases -> "Applied"
    [] c \in StaleWorkerResultCases -> "IgnoredStale"
    [] OTHER -> "NoAction"

ActualAction(c) ==
  CASE c = "worker_result_wrong_id" /\ BugApplyWrongId -> "Applied"
    [] c = "worker_result_wrong_generation" /\ BugApplyWrongGeneration ->
      "Applied"
    [] c = "worker_result_not_running" /\ BugApplyNotRunning -> "Applied"
    [] c = "worker_result_valid_matching" /\ BugIgnoreMatchingValid ->
      "IgnoredStale"
    [] c = "worker_result_invalid_matching" /\ BugIgnoreMatchingInvalid ->
      "IgnoredStale"
    [] OTHER -> SpecAction(c)

SpecNextState(c) ==
  CASE c = "worker_started_records_owner" -> "RunningOwned"
    [] c = "worker_result_valid_matching" -> "Valid"
    [] c = "worker_result_invalid_matching" -> "Invalid"
    [] c \in StaleWorkerResultCases -> StateBefore(c)
    [] OTHER -> StateBefore(c)

ActualNextState(c) ==
  CASE c = "worker_started_records_owner" /\ BugWorkerStartedKeepsQueued ->
      "Queued"
    [] c = "worker_result_valid_matching" /\ BugIgnoreMatchingValid ->
      "Running"
    [] c = "worker_result_invalid_matching" /\ BugIgnoreMatchingInvalid ->
      "Running"
    [] c = "worker_result_wrong_id" /\ BugApplyWrongId -> "Valid"
    [] c = "worker_result_wrong_generation" /\ BugApplyWrongGeneration ->
      "Valid"
    [] c = "worker_result_not_running" /\ BugApplyNotRunning -> "Valid"
    [] c \in StaleWorkerResultCases /\ BugStaleMutatesState -> "Invalid"
    [] OTHER -> SpecNextState(c)

TypeInvariant ==
  /\ BugDispatchQueued \in BOOLEAN
  /\ BugRaiseRunningBeforeTimeout \in BOOLEAN
  /\ BugMissRunningAtTimeout \in BOOLEAN
  /\ BugBackpressureBeforeTimeoutRaises \in BOOLEAN
  /\ BugMissBackpressureAtTimeout \in BOOLEAN
  /\ BugAcceptValidAsAwait \in BOOLEAN
  /\ BugRejectInvalidAsAwait \in BOOLEAN
  /\ BugUnderflowElapsed \in BOOLEAN
  /\ BugWorkerStartedKeepsQueued \in BOOLEAN
  /\ BugApplyWrongId \in BOOLEAN
  /\ BugApplyWrongGeneration \in BOOLEAN
  /\ BugApplyNotRunning \in BOOLEAN
  /\ BugIgnoreMatchingValid \in BOOLEAN
  /\ BugIgnoreMatchingInvalid \in BOOLEAN
  /\ BugStaleMutatesState \in BOOLEAN
  /\ candidate \in Cases \union {"none"}
  /\ decision \in {
       "NoDecision",
       "DispatchWorker",
       "AwaitWorker",
       "RaiseSuspicion",
       "Backpressure",
       "Accept",
       "Reject"
     }
  /\ nextState \in {
       "None",
       "Unqueued",
       "Queued",
       "Running",
       "RunningOwned",
       "Backpressured",
       "Valid",
       "Invalid"
     }
  /\ action \in {"NoAction", "Started", "Applied", "IgnoredStale"}

Init ==
  /\ candidate = "none"
  /\ decision = "NoDecision"
  /\ nextState = "None"
  /\ action = "NoAction"

Apply(c) ==
  /\ candidate' = c
  /\ decision' = ActualDecision(c)
  /\ nextState' = ActualNextState(c)
  /\ action' = ActualAction(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

DecisionMatchesSpec ==
  candidate = "none" \/ decision = SpecDecision(candidate)

RunningTimeoutBoundaryMatchesSpec ==
  candidate \in RunningDecisionCases => decision = SpecDecision(candidate)

BackpressureTimeoutBoundaryMatchesSpec ==
  candidate \in BackpressureDecisionCases => decision = SpecDecision(candidate)

SaturatingElapsedDoesNotRaiseEarly ==
  candidate = "running_now_before_started" => decision = "AwaitWorker"

TerminalStatesNeverDispatchOrSuspect ==
  candidate \in TerminalDecisionCases =>
    /\ decision \notin {"DispatchWorker", "RaiseSuspicion"}
    /\ decision = SpecDecision(candidate)

WorkerStartedRecordsOwner ==
  candidate = "worker_started_records_owner" =>
    /\ action = "Started"
    /\ nextState = "RunningOwned"

MatchingWorkerResultsApply ==
  candidate \in MatchingWorkerResultCases =>
    /\ action = "Applied"
    /\ nextState = SpecNextState(candidate)
    /\ nextState \in {"Valid", "Invalid"}

StaleWorkerResultsIgnored ==
  candidate \in StaleWorkerResultCases => action = "IgnoredStale"

IgnoredResultsPreserveState ==
  candidate \in StaleWorkerResultCases => nextState = StateBefore(candidate)

AppliedResultsReachTerminalState ==
  candidate \in WorkerResultCases /\ action = "Applied" =>
    /\ candidate \in MatchingWorkerResultCases
    /\ nextState \in {"Valid", "Invalid"}

VNextValidationExactness ==
  /\ DecisionMatchesSpec
  /\ RunningTimeoutBoundaryMatchesSpec
  /\ BackpressureTimeoutBoundaryMatchesSpec
  /\ SaturatingElapsedDoesNotRaiseEarly
  /\ TerminalStatesNeverDispatchOrSuspect
  /\ WorkerStartedRecordsOwner
  /\ MatchingWorkerResultsApply
  /\ StaleWorkerResultsIgnored
  /\ IgnoredResultsPreserveState
  /\ AppliedResultsReachTerminalState

Safety ==
  VNextValidationExactness

VNextValidationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ VNextValidationExactness

SafetyFast ==
  VNextValidationExactness

====
