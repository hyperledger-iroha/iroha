---- MODULE SumeragiPacemakerBackpressureTrackerGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `PacemakerBackpressureTracker::update(...)`.

This slice captures the per-reason tracker above the aggregate pacemaker
backpressure decision. It checks the exact reason labels used for telemetry,
the `deferring` gate applied to every `ProposalBackpressure` reason, first
activation telemetry, sustained-age updates, clear-duration telemetry, idle
no-ops, and saturating elapsed-time behavior for backward clock samples.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

QueueSaturation == "queue_saturation"
ActivePending == "active_pending"
RbcBacklog == "rbc_backlog"
RelayBackpressure == "relay_backpressure"
ConsensusQueueBackpressure == "consensus_queue_backpressure"

Reasons == {
  QueueSaturation,
  ActivePending,
  RbcBacklog,
  RelayBackpressure,
  ConsensusQueueBackpressure
}

First == "first"
SustainForward == "sustain_forward"
SustainBackward == "sustain_backward"
ClearForward == "clear_forward"
ClearBackward == "clear_backward"
ClearSignalNoDeferral == "clear_signal_no_deferral"
ClearNoSignalDeferring == "clear_no_signal_deferring"
ClearMissingStarted == "clear_missing_started"
InactiveSignalNoDeferral == "inactive_signal_no_deferral"
InactiveNoSignalDeferring == "inactive_no_signal_deferring"

Events == {
  First,
  SustainForward,
  SustainBackward,
  ClearForward,
  ClearBackward,
  ClearSignalNoDeferral,
  ClearNoSignalDeferring,
  ClearMissingStarted,
  InactiveSignalNoDeferral,
  InactiveNoSignalDeferring
}

AfterActive == 1
AfterInactive == 2
AfterStarted == 3
AfterNoStart == 4
IncCounter == 5
SetActiveTrue == 6
SetActiveFalse == 7
SetAgeZero == 8
SetAgeElapsed == 9
ObserveDurationZero == 10
ObserveDurationElapsed == 11

Actions == 1..11

SpecLabel(reason) ==
  CASE reason = QueueSaturation -> "queue_saturated"
    [] reason = ActivePending -> "active_pending"
    [] reason = RbcBacklog -> "rbc_backlog"
    [] reason = RelayBackpressure -> "relay_backpressure"
    [] reason = ConsensusQueueBackpressure -> "consensus_queue_backpressure"
    [] OTHER -> "unknown"

ActualLabel(reason) ==
  CASE Bug = "label_queue_saturation_wrong" /\ reason = QueueSaturation ->
      "queue_saturation"
    [] Bug = "label_active_pending_wrong" /\ reason = ActivePending ->
      "active-pending"
    [] Bug = "label_rbc_backlog_wrong" /\ reason = RbcBacklog ->
      "rbc"
    [] Bug = "label_relay_backpressure_wrong"
       /\ reason = RelayBackpressure ->
      "relay"
    [] Bug = "label_consensus_queue_wrong"
       /\ reason = ConsensusQueueBackpressure ->
      "consensus_queue"
    [] OTHER -> SpecLabel(reason)

PriorActive(event) ==
  event \in {
    SustainForward,
    SustainBackward,
    ClearForward,
    ClearBackward,
    ClearSignalNoDeferral,
    ClearNoSignalDeferring,
    ClearMissingStarted
  }

PriorStarted(event) ==
  event \in {
    SustainForward,
    SustainBackward,
    ClearForward,
    ClearBackward,
    ClearSignalNoDeferral,
    ClearNoSignalDeferring
  }

SignalPresent(event) ==
  event \in {
    First,
    SustainForward,
    SustainBackward,
    ClearSignalNoDeferral,
    InactiveSignalNoDeferral
  }

Deferring(event) ==
  event \in {
    First,
    SustainForward,
    SustainBackward,
    ClearForward,
    ClearBackward,
    ClearNoSignalDeferring,
    ClearMissingStarted,
    InactiveNoSignalDeferring
  }

TimeForward(event) ==
  event \in {SustainForward, ClearForward, ClearSignalNoDeferral,
    ClearNoSignalDeferring}

SpecActiveInput(event) == SignalPresent(event) /\ Deferring(event)

ActualActiveInput(reason, event) ==
  CASE Bug = "missing_deferring_gate" /\ SignalPresent(event) ->
      TRUE
    [] Bug = "queue_saturation_ignores_signal"
       /\ reason = QueueSaturation ->
      Deferring(event)
    [] Bug = "active_pending_missed" /\ reason = ActivePending ->
      FALSE
    [] Bug = "rbc_backlog_missed" /\ reason = RbcBacklog ->
      FALSE
    [] Bug = "relay_backpressure_missed"
       /\ reason = RelayBackpressure ->
      FALSE
    [] Bug = "consensus_queue_missed"
       /\ reason = ConsensusQueueBackpressure ->
      FALSE
    [] OTHER -> SpecActiveInput(event)

TransitionActions(prev_active, prev_started, active_input, time_forward) ==
  CASE ~prev_active /\ active_input ->
      {AfterActive, AfterStarted, IncCounter, SetActiveTrue, SetAgeZero}
    [] prev_active /\ active_input /\ time_forward ->
      {AfterActive, AfterStarted, SetAgeElapsed}
    [] prev_active /\ active_input /\ ~time_forward ->
      {AfterActive, AfterStarted, SetAgeZero}
    [] prev_active /\ ~active_input /\ prev_started /\ time_forward ->
      {AfterInactive, AfterNoStart, ObserveDurationElapsed,
       SetActiveFalse, SetAgeZero}
    [] prev_active /\ ~active_input /\ prev_started /\ ~time_forward ->
      {AfterInactive, AfterNoStart, ObserveDurationZero,
       SetActiveFalse, SetAgeZero}
    [] prev_active /\ ~active_input /\ ~prev_started ->
      {AfterInactive, AfterNoStart}
    [] OTHER ->
      {AfterInactive, AfterNoStart}

SpecActions(event) ==
  TransitionActions(
    PriorActive(event),
    PriorStarted(event),
    SpecActiveInput(event),
    TimeForward(event)
  )

EntryEvent(event) == ~PriorActive(event) /\ SpecActiveInput(event)

SustainEvent(event) == PriorActive(event) /\ SpecActiveInput(event)

ClearEvent(event) ==
  PriorActive(event) /\ ~SpecActiveInput(event) /\ PriorStarted(event)

MissingStartedClearEvent(event) ==
  PriorActive(event) /\ ~SpecActiveInput(event) /\ ~PriorStarted(event)

IdleEvent(event) == ~PriorActive(event) /\ ~SpecActiveInput(event)

ActualActions(reason, event) ==
  LET base == TransitionActions(
      PriorActive(event),
      PriorStarted(event),
      ActualActiveInput(reason, event),
      IF Bug = "sustained_age_not_saturating"
         /\ event = SustainBackward
      THEN TRUE
      ELSE TimeForward(event)
    ) IN
  CASE Bug = "first_missing_counter" /\ EntryEvent(event) ->
      base \ {IncCounter}
    [] Bug = "first_missing_started" /\ EntryEvent(event) ->
      (base \ {AfterStarted}) \cup {AfterNoStart}
    [] Bug = "first_age_not_zero" /\ EntryEvent(event) ->
      (base \ {SetAgeZero}) \cup {SetAgeElapsed}
    [] Bug = "sustained_reincrements_counter" /\ SustainEvent(event) ->
      base \cup {IncCounter}
    [] Bug = "sustained_drops_started" /\ SustainEvent(event) ->
      (base \ {AfterStarted}) \cup {AfterNoStart}
    [] Bug = "clear_missing_duration" /\ ClearEvent(event) ->
      base \ {ObserveDurationZero, ObserveDurationElapsed}
    [] Bug = "clear_keeps_active" /\ ClearEvent(event) ->
      (base \ {AfterInactive}) \cup {AfterActive}
    [] Bug = "clear_keeps_started" /\ ClearEvent(event) ->
      (base \ {AfterNoStart}) \cup {AfterStarted}
    [] Bug = "clear_age_not_reset" /\ ClearEvent(event) ->
      (base \ {SetAgeZero}) \cup {SetAgeElapsed}
    [] Bug = "clear_active_gauge_not_false" /\ ClearEvent(event) ->
      (base \ {SetActiveFalse}) \cup {SetActiveTrue}
    [] Bug = "idle_sets_telemetry" /\ IdleEvent(event) ->
      base \cup {SetActiveFalse, SetAgeZero}
    [] Bug = "missing_started_clear_emits_duration"
       /\ MissingStartedClearEvent(event) ->
      base \cup {ObserveDurationZero, SetActiveFalse, SetAgeZero}
    [] OTHER -> base

Bugs == {
  "none",
  "label_queue_saturation_wrong",
  "label_active_pending_wrong",
  "label_rbc_backlog_wrong",
  "label_relay_backpressure_wrong",
  "label_consensus_queue_wrong",
  "missing_deferring_gate",
  "queue_saturation_ignores_signal",
  "active_pending_missed",
  "rbc_backlog_missed",
  "relay_backpressure_missed",
  "consensus_queue_missed",
  "first_missing_counter",
  "first_missing_started",
  "first_age_not_zero",
  "sustained_reincrements_counter",
  "sustained_age_not_saturating",
  "sustained_drops_started",
  "clear_missing_duration",
  "clear_keeps_active",
  "clear_keeps_started",
  "clear_age_not_reset",
  "clear_active_gauge_not_false",
  "idle_sets_telemetry",
  "missing_started_clear_emits_duration"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A reason \in Reasons:
       /\ SpecLabel(reason) \in {
            "queue_saturated",
            "active_pending",
            "rbc_backlog",
            "relay_backpressure",
            "consensus_queue_backpressure"
          }
       /\ ActualLabel(reason) \in {
            "queue_saturated",
            "queue_saturation",
            "active_pending",
            "active-pending",
            "rbc_backlog",
            "rbc",
            "relay_backpressure",
            "relay",
            "consensus_queue_backpressure",
            "consensus_queue"
          }
       /\ \A event \in Events:
            /\ SpecActions(event) \subseteq Actions
            /\ ActualActions(reason, event) \subseteq Actions

LabelsMatch ==
  \A reason \in Reasons:
    ActualLabel(reason) = SpecLabel(reason)

ActionsMatch ==
  \A reason \in Reasons:
    \A event \in Events:
      ActualActions(reason, event) = SpecActions(event)

ReasonSignalsGateTelemetry ==
  \A reason \in Reasons:
    \A event \in Events:
      ActualActiveInput(reason, event) = SpecActiveInput(event)

FirstActivationTelemetry ==
  \A reason \in Reasons:
    /\ IncCounter \in ActualActions(reason, First)
    /\ SetActiveTrue \in ActualActions(reason, First)
    /\ SetAgeZero \in ActualActions(reason, First)
    /\ AfterStarted \in ActualActions(reason, First)

SustainedForwardUpdatesAgeOnly ==
  \A reason \in Reasons:
    /\ ActualActions(reason, SustainForward) =
       {AfterActive, AfterStarted, SetAgeElapsed}
    /\ ActualActions(reason, SustainBackward) =
       {AfterActive, AfterStarted, SetAgeZero}

ClearingObservesDurationAndResets ==
  \A reason \in Reasons:
    /\ ActualActions(reason, ClearForward) =
       {AfterInactive, AfterNoStart, ObserveDurationElapsed,
        SetActiveFalse, SetAgeZero}
    /\ ActualActions(reason, ClearBackward) =
       {AfterInactive, AfterNoStart, ObserveDurationZero,
        SetActiveFalse, SetAgeZero}
    /\ ActualActions(reason, ClearSignalNoDeferral) =
       {AfterInactive, AfterNoStart, ObserveDurationElapsed,
        SetActiveFalse, SetAgeZero}
    /\ ActualActions(reason, ClearNoSignalDeferring) =
       {AfterInactive, AfterNoStart, ObserveDurationElapsed,
        SetActiveFalse, SetAgeZero}

IdleAndMissingStartedDoNotEmitTelemetry ==
  \A reason \in Reasons:
    /\ ActualActions(reason, InactiveSignalNoDeferral) =
       {AfterInactive, AfterNoStart}
    /\ ActualActions(reason, InactiveNoSignalDeferring) =
       {AfterInactive, AfterNoStart}
    /\ ActualActions(reason, ClearMissingStarted) =
       {AfterInactive, AfterNoStart}

Safety ==
  /\ LabelsMatch
  /\ ActionsMatch
  /\ ReasonSignalsGateTelemetry
  /\ FirstActivationTelemetry
  /\ SustainedForwardUpdatesAgeOnly
  /\ ClearingObservesDurationAndResets
  /\ IdleAndMissingStartedDoNotEmitTelemetry

====
