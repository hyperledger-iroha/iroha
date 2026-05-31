---- MODULE SumeragiRbcCommitProcessingGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the RBC commit-pipeline trigger helpers:
`should_process_commit_after_ready(...)` and
`should_process_commit_after_deliver(...)`.

READY handling should re-drive commit work when pending state was cleared, a
READY quorum is reached, or new READY/DELIVER evidence changed the session
before delivery. Once the block was already delivered, ordinary READY/DELIVER
state changes must not keep waking commit work unless a READY quorum or
clear-pending condition explicitly asks for it. DELIVER handling wakes commit
work exactly once, on the first DELIVER.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  ready_case,
  \* @type: Bool;
  ready_process,
  \* @type: Str;
  deliver_case,
  \* @type: Bool;
  deliver_process

\* @type: <<Str, Bool, Str, Bool>>;
vars == <<ready_case, ready_process, deliver_case, deliver_process>>

ReadyCases == {
  "no_change",
  "recorded_ready",
  "clear_pending",
  "deliver_emitted",
  "ready_quorum",
  "delivered_before_recorded",
  "delivered_before_emitted",
  "delivered_before_ready_quorum",
  "recorded_and_emitted",
  "clear_pending_after_deliver"
}

DeliverCases == {"first_deliver", "duplicate_deliver"}

RecordedReady(c) ==
  c \in {"recorded_ready", "delivered_before_recorded", "recorded_and_emitted"}

ClearPending(c) ==
  c \in {"clear_pending", "clear_pending_after_deliver"}

DeliveredBefore(c) ==
  c \in {
    "delivered_before_recorded",
    "delivered_before_emitted",
    "delivered_before_ready_quorum",
    "clear_pending_after_deliver"
  }

DeliverEmitted(c) ==
  c \in {"deliver_emitted", "delivered_before_emitted", "recorded_and_emitted"}

ReadyQuorumReached(c) ==
  c \in {"ready_quorum", "delivered_before_ready_quorum"}

SpecReadyProcess(c) ==
  \/ ClearPending(c)
  \/ ReadyQuorumReached(c)
  \/ ((RecordedReady(c) \/ DeliverEmitted(c)) /\ ~DeliveredBefore(c))

FirstDeliver(c) ==
  c = "first_deliver"

SpecDeliverProcess(c) ==
  FirstDeliver(c)

ActualClearPending(c) ==
  IF Bug = "ignore_clear_pending" THEN FALSE ELSE ClearPending(c)

ActualReadyQuorumReached(c) ==
  IF Bug = "ignore_ready_quorum" THEN FALSE ELSE ReadyQuorumReached(c)

ActualRecordedReady(c) ==
  IF Bug = "ignore_recorded_ready" THEN FALSE ELSE RecordedReady(c)

ActualDeliverEmitted(c) ==
  IF Bug = "ignore_deliver_emitted" THEN FALSE ELSE DeliverEmitted(c)

ActualStateChangeBeforeDelivery(c) ==
  CASE Bug = "process_delivered_before_state_change" ->
         ActualRecordedReady(c) \/ ActualDeliverEmitted(c)
    [] Bug = "require_recorded_and_deliver_emitted" ->
         ActualRecordedReady(c) /\ ActualDeliverEmitted(c) /\ ~DeliveredBefore(c)
    [] OTHER ->
         (ActualRecordedReady(c) \/ ActualDeliverEmitted(c)) /\ ~DeliveredBefore(c)

ActualReadyProcess(c) ==
  \/ ActualClearPending(c)
  \/ ActualReadyQuorumReached(c)
  \/ ActualStateChangeBeforeDelivery(c)
  \/ (Bug = "process_without_change" /\ c = "no_change")

ActualDeliverProcess(c) ==
  CASE Bug = "deliver_skips_first" -> FALSE
    [] Bug = "deliver_runs_on_duplicate" -> TRUE
    [] Bug = "deliver_inverts_first" -> ~FirstDeliver(c)
    [] OTHER -> SpecDeliverProcess(c)

TypeInvariant ==
  /\ Bug \in {
       "none",
       "ignore_clear_pending",
       "ignore_ready_quorum",
       "process_delivered_before_state_change",
       "ignore_recorded_ready",
       "ignore_deliver_emitted",
       "require_recorded_and_deliver_emitted",
       "process_without_change",
       "deliver_skips_first",
       "deliver_runs_on_duplicate",
       "deliver_inverts_first"
     }
  /\ ready_case \in ReadyCases
  /\ ready_process \in BOOLEAN
  /\ deliver_case \in DeliverCases
  /\ deliver_process \in BOOLEAN

Init ==
  /\ ready_case \in ReadyCases
  /\ ready_process = ActualReadyProcess(ready_case)
  /\ deliver_case \in DeliverCases
  /\ deliver_process = ActualDeliverProcess(deliver_case)

Next ==
  UNCHANGED vars

ReadyDecisionMatchesSpec ==
  ready_process = SpecReadyProcess(ready_case)

DeliverDecisionMatchesSpec ==
  deliver_process = SpecDeliverProcess(deliver_case)

ClearPendingProcessesReady ==
  ready_case = "clear_pending" => ready_process

ReadyQuorumProcessesReady ==
  ready_case = "ready_quorum" => ready_process

ReadyQuorumOverridesDeliveredBefore ==
  ready_case = "delivered_before_ready_quorum" => ready_process

RecordedReadyProcessesBeforeDelivery ==
  ready_case = "recorded_ready" => ready_process

DeliverEmissionProcessesBeforeDelivery ==
  ready_case = "deliver_emitted" => ready_process

RecordedAndDeliverEmittedProcessBeforeDelivery ==
  ready_case = "recorded_and_emitted" => ready_process

DeliveredBeforeStateChangeDoesNotProcessWithoutQuorum ==
  ready_case \in {"delivered_before_recorded", "delivered_before_emitted"} =>
    ~ready_process

NoReadyStateChangeDoesNotProcess ==
  ready_case = "no_change" => ~ready_process

ClearPendingAfterDeliverStillProcesses ==
  ready_case = "clear_pending_after_deliver" => ready_process

ReadyProcessRequiresSpecCause ==
  ready_process => SpecReadyProcess(ready_case)

FirstDeliverProcesses ==
  deliver_case = "first_deliver" => deliver_process

DuplicateDeliverDoesNotProcess ==
  deliver_case = "duplicate_deliver" => ~deliver_process

Safety ==
  /\ ReadyDecisionMatchesSpec
  /\ DeliverDecisionMatchesSpec
  /\ ClearPendingProcessesReady
  /\ ReadyQuorumProcessesReady
  /\ ReadyQuorumOverridesDeliveredBefore
  /\ RecordedReadyProcessesBeforeDelivery
  /\ DeliverEmissionProcessesBeforeDelivery
  /\ RecordedAndDeliverEmittedProcessBeforeDelivery
  /\ DeliveredBeforeStateChangeDoesNotProcessWithoutQuorum
  /\ NoReadyStateChangeDoesNotProcess
  /\ ClearPendingAfterDeliverStillProcesses
  /\ ReadyProcessRequiresSpecCause
  /\ FirstDeliverProcesses
  /\ DuplicateDeliverDoesNotProcess

====
