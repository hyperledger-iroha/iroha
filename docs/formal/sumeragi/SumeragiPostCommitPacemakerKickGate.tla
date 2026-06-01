---- MODULE SumeragiPostCommitPacemakerKickGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for post-commit pacemaker kickstart gating.

This slice models `kickstart_pacemaker_after_commit(...)`. After a durable
commit, the actor may immediately ask the pacemaker to propose again only when
there is queued transaction work and proposal backpressure is either absent or
limited to pacing-only pressure (queue saturation or consensus ingress
backpressure). Active pending blocks, RBC backlog, and relay backpressure are
hard stops. The helper reports whether it attempted the kickstart, so a
trigger callback returning false must not make the helper return false.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  trigger_called,
  \* @type: Bool;
  timestamp_captured,
  \* @type: Bool;
  callback_false_seen,
  \* @type: Bool;
  returned_true,
  \* @type: Bool;
  returned_false

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, trigger_called, timestamp_captured,
  callback_false_seen, returned_true, returned_false>>

Cases == {
  "no_queue_healthy",
  "no_queue_queue_saturated",
  "no_queue_consensus_pacing",
  "queued_healthy_callback_true",
  "queued_healthy_callback_false",
  "queued_queue_saturated",
  "queued_consensus_pacing",
  "queued_combined_pacing",
  "queued_active_pending",
  "queued_rbc_backlog",
  "queued_relay_backpressure",
  "queued_active_pending_with_queue_saturated",
  "queued_rbc_backlog_with_consensus",
  "queued_relay_with_queue_saturated",
  "queued_all_backpressure"
}

NoQueueCases == {
  "no_queue_healthy",
  "no_queue_queue_saturated",
  "no_queue_consensus_pacing"
}
QueuedCases == Cases \ NoQueueCases
HealthyQueuedCases == {
  "queued_healthy_callback_true",
  "queued_healthy_callback_false"
}
QueueSaturatedOnlyCases == {"queued_queue_saturated"}
ConsensusPacingOnlyCases == {"queued_consensus_pacing"}
CombinedPacingOnlyCases == {"queued_combined_pacing"}
PacingOnlyQueuedCases ==
  QueueSaturatedOnlyCases \union ConsensusPacingOnlyCases \union CombinedPacingOnlyCases
ActivePendingCases == {
  "queued_active_pending",
  "queued_active_pending_with_queue_saturated",
  "queued_all_backpressure"
}
RbcBacklogCases == {
  "queued_rbc_backlog",
  "queued_rbc_backlog_with_consensus",
  "queued_all_backpressure"
}
RelayBackpressureCases == {
  "queued_relay_backpressure",
  "queued_relay_with_queue_saturated",
  "queued_all_backpressure"
}
HardBackpressureCases ==
  ActivePendingCases \union RbcBacklogCases \union RelayBackpressureCases
AllowedCases == HealthyQueuedCases \union PacingOnlyQueuedCases
CallbackFalseCases == {"queued_healthy_callback_false"}

HasQueuedWork(c) == c \in QueuedCases
HasHardBackpressure(c) == c \in HardBackpressureCases
HasPacingOnlyBackpressure(c) == c \in PacingOnlyQueuedCases
SpecTriggerCalled(c) == c \in AllowedCases
SpecTimestampCaptured(c) == c \in AllowedCases
SpecCallbackFalseSeen(c) == c \in CallbackFalseCases
SpecReturnedTrue(c) == c \in AllowedCases
SpecReturnedFalse(c) == ~SpecReturnedTrue(c)

ActualTriggerCalled(c) ==
  \/ /\ SpecTriggerCalled(c)
     /\ ~(Bug = "skip_healthy_queue" /\ c \in HealthyQueuedCases)
     /\ ~(Bug = "skip_queue_saturated_pacing"
          /\ c \in QueueSaturatedOnlyCases)
     /\ ~(Bug = "skip_consensus_pacing"
          /\ c \in ConsensusPacingOnlyCases)
     /\ ~(Bug = "skip_combined_pacing"
          /\ c \in CombinedPacingOnlyCases)
     /\ ~(Bug = "use_callback_result_false" /\ c \in CallbackFalseCases)
  \/ /\ c \in NoQueueCases
     /\ Bug = "trigger_without_queue"
  \/ /\ c \in {"queued_active_pending"}
     /\ Bug = "trigger_active_pending"
  \/ /\ c \in {"queued_rbc_backlog"}
     /\ Bug = "trigger_rbc_backlog"
  \/ /\ c \in {"queued_relay_backpressure"}
     /\ Bug = "trigger_relay_backpressure"
  \/ /\ c \in {"queued_all_backpressure"}
     /\ Bug = "trigger_hard_stop"
  \/ /\ c \in {"queued_active_pending_with_queue_saturated"}
     /\ Bug = "ignore_active_pending_with_pacing"
  \/ /\ c \in {"queued_rbc_backlog_with_consensus"}
     /\ Bug = "ignore_rbc_with_pacing"
  \/ /\ c \in {"queued_relay_with_queue_saturated"}
     /\ Bug = "ignore_relay_with_pacing"

ActualTimestampCaptured(c) ==
  \/ /\ SpecTimestampCaptured(c)
     /\ Bug # "skip_time_when_triggered"
     /\ ActualTriggerCalled(c)
  \/ /\ ~SpecTimestampCaptured(c)
     /\ Bug = "capture_time_when_suppressed"

ActualCallbackFalseSeen(c) == SpecCallbackFalseSeen(c)

ActualReturnedTrue(c) ==
  \/ /\ SpecReturnedTrue(c)
     /\ ~(Bug = "skip_healthy_queue" /\ c \in HealthyQueuedCases)
     /\ ~(Bug = "skip_queue_saturated_pacing"
          /\ c \in QueueSaturatedOnlyCases)
     /\ ~(Bug = "skip_consensus_pacing"
          /\ c \in ConsensusPacingOnlyCases)
     /\ ~(Bug = "skip_combined_pacing"
          /\ c \in CombinedPacingOnlyCases)
     /\ ~(Bug = "use_callback_result_false" /\ c \in CallbackFalseCases)
     /\ Bug # "trigger_without_return_true"
  \/ /\ c \in NoQueueCases
     /\ Bug = "trigger_without_queue"
  \/ /\ c \in {"queued_active_pending"}
     /\ Bug = "trigger_active_pending"
  \/ /\ c \in {"queued_rbc_backlog"}
     /\ Bug = "trigger_rbc_backlog"
  \/ /\ c \in {"queued_relay_backpressure"}
     /\ Bug = "trigger_relay_backpressure"
  \/ /\ c \in {"queued_all_backpressure"}
     /\ Bug = "trigger_hard_stop"
  \/ /\ c \in {"queued_active_pending_with_queue_saturated"}
     /\ Bug = "ignore_active_pending_with_pacing"
  \/ /\ c \in {"queued_rbc_backlog_with_consensus"}
     /\ Bug = "ignore_rbc_with_pacing"
  \/ /\ c \in {"queued_relay_with_queue_saturated"}
     /\ Bug = "ignore_relay_with_pacing"
  \/ /\ ~SpecReturnedTrue(c)
     /\ Bug = "return_true_without_trigger"

ActualReturnedFalse(c) == ~ActualReturnedTrue(c)

Init ==
  /\ candidate = "none"
  /\ trigger_called = FALSE
  /\ timestamp_captured = FALSE
  /\ callback_false_seen = FALSE
  /\ returned_true = FALSE
  /\ returned_false = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ trigger_called' = ActualTriggerCalled(c)
  /\ timestamp_captured' = ActualTimestampCaptured(c)
  /\ callback_false_seen' = ActualCallbackFalseSeen(c)
  /\ returned_true' = ActualReturnedTrue(c)
  /\ returned_false' = ActualReturnedFalse(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ trigger_called \in BOOLEAN
  /\ timestamp_captured \in BOOLEAN
  /\ callback_false_seen \in BOOLEAN
  /\ returned_true \in BOOLEAN
  /\ returned_false \in BOOLEAN

CasePartitionExact ==
  /\ Cases =
       NoQueueCases \union HealthyQueuedCases \union PacingOnlyQueuedCases
       \union HardBackpressureCases
  /\ NoQueueCases \intersect QueuedCases = {}
  /\ AllowedCases = QueuedCases \ HardBackpressureCases
  /\ AllowedCases \intersect HardBackpressureCases = {}

TriggerMatchesSpec ==
  candidate = "none" \/ trigger_called = SpecTriggerCalled(candidate)

TimestampMatchesSpec ==
  candidate = "none" \/
    timestamp_captured = SpecTimestampCaptured(candidate)

ReturnMatchesSpec ==
  candidate = "none" \/
    /\ returned_true = SpecReturnedTrue(candidate)
    /\ returned_false = SpecReturnedFalse(candidate)
    /\ returned_true # returned_false

NoQueueNeverTriggers ==
  candidate \in NoQueueCases =>
    /\ ~trigger_called
    /\ ~timestamp_captured
    /\ returned_false

QueuedHealthyAlwaysTriggers ==
  candidate \in HealthyQueuedCases =>
    /\ trigger_called
    /\ timestamp_captured
    /\ returned_true

PacingOnlyBackpressureStillTriggers ==
  candidate \in PacingOnlyQueuedCases =>
    /\ trigger_called
    /\ timestamp_captured
    /\ returned_true

HardBackpressureSuppressesKickstart ==
  candidate \in HardBackpressureCases =>
    /\ ~trigger_called
    /\ ~timestamp_captured
    /\ returned_false

CallbackResultDoesNotControlReturn ==
  candidate \in CallbackFalseCases =>
    /\ callback_false_seen
    /\ trigger_called
    /\ returned_true

TriggerRequiresQueuedWork ==
  trigger_called => HasQueuedWork(candidate)

TriggerRejectsHardBackpressure ==
  trigger_called => ~HasHardBackpressure(candidate)

TriggerReturnAndTimeAreConsistent ==
  /\ (trigger_called <=> returned_true)
  /\ (trigger_called <=> timestamp_captured)

SuppressionDoesNotCaptureTime ==
  returned_false => ~timestamp_captured

Safety ==
  /\ CasePartitionExact
  /\ TriggerMatchesSpec
  /\ TimestampMatchesSpec
  /\ ReturnMatchesSpec
  /\ NoQueueNeverTriggers
  /\ QueuedHealthyAlwaysTriggers
  /\ PacingOnlyBackpressureStillTriggers
  /\ HardBackpressureSuppressesKickstart
  /\ CallbackResultDoesNotControlReturn
  /\ TriggerRequiresQueuedWork
  /\ TriggerRejectsHardBackpressure
  /\ TriggerReturnAndTimeAreConsistent
  /\ SuppressionDoesNotCaptureTime

=============================================================================
====
