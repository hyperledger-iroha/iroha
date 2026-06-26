---- MODULE SumeragiPipelineEventEmissionGate ----
EXTENDS Naturals, Sequences

(***************************************************************************
A bounded abstract model for `emit_pipeline_events(...)`.

The helper is the common path for forwarding consensus pipeline notifications
from internal processing into the public event stream. It must not emit for an
empty vector, must wrap a single event in `EventBox::Pipeline`, must wrap
multiple events in one `EventBox::PipelineBatch` without reordering or
deduplicating, and must tolerate broadcast send failure by logging without
panicking.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Empty == 1
SingleA == 2
TwoAB == 3
ThreeABC == 4
TwoDuplicate == 5

InputCases == 1..5
Open == "open"
Closed == "closed"
SenderCases == {Open, Closed}

NoEnvelope == 0
PipelineEnvelope == 1
PipelineBatchEnvelope == 2
EnvelopeKinds == 0..2

EventSeqs == {
  <<>>,
  <<1>>,
  <<2>>,
  <<1, 2>>,
  <<2, 1>>,
  <<1, 2, 3>>,
  <<1, 2, 2>>,
  <<2, 2>>
}

InputEvents(c) ==
  CASE c = Empty -> <<>>
    [] c = SingleA -> <<1>>
    [] c = TwoAB -> <<1, 2>>
    [] c = ThreeABC -> <<1, 2, 3>>
    [] c = TwoDuplicate -> <<2, 2>>

SpecAttemptCount(c, sender) ==
  IF Len(InputEvents(c)) = 0 THEN 0 ELSE 1

ActualAttemptCount(c, sender) ==
  CASE Bug = "empty_sends_batch"
       /\ c = Empty -> 1
    [] Bug = "multi_split_attempts"
       /\ Len(InputEvents(c)) > 1 -> Len(InputEvents(c))
    [] Bug = "closed_skips_attempt"
       /\ sender = Closed
       /\ Len(InputEvents(c)) > 0 -> 0
    [] OTHER -> SpecAttemptCount(c, sender)

SpecAttemptKind(c, sender) ==
  CASE Len(InputEvents(c)) = 0 -> NoEnvelope
    [] Len(InputEvents(c)) = 1 -> PipelineEnvelope
    [] OTHER -> PipelineBatchEnvelope

ActualAttemptKind(c, sender) ==
  CASE Bug = "empty_sends_batch"
       /\ c = Empty -> PipelineBatchEnvelope
    [] Bug = "single_sent_as_batch"
       /\ c = SingleA -> PipelineBatchEnvelope
    [] Bug = "multi_sent_as_single"
       /\ Len(InputEvents(c)) > 1 -> PipelineEnvelope
    [] Bug = "multi_dropped"
       /\ Len(InputEvents(c)) > 1 -> NoEnvelope
    [] Bug = "closed_skips_attempt"
       /\ sender = Closed
       /\ Len(InputEvents(c)) > 0 -> NoEnvelope
    [] OTHER -> SpecAttemptKind(c, sender)

SpecAttemptPayload(c, sender) ==
  IF SpecAttemptKind(c, sender) = NoEnvelope THEN <<>> ELSE InputEvents(c)

ActualAttemptPayload(c, sender) ==
  CASE Bug = "empty_sends_batch"
       /\ c = Empty -> <<>>
    [] Bug = "single_wrong_event"
       /\ c = SingleA -> <<2>>
    [] Bug = "reverse_batch_order"
       /\ c = TwoAB -> <<2, 1>>
    [] Bug = "drop_batch_tail"
       /\ c = ThreeABC -> <<1, 2>>
    [] Bug = "duplicate_batch_dedup"
       /\ c = TwoDuplicate -> <<2>>
    [] Bug = "closed_skips_attempt"
       /\ sender = Closed
       /\ Len(InputEvents(c)) > 0 -> <<>>
    [] OTHER -> SpecAttemptPayload(c, sender)

SpecDeliveredKind(c, sender) ==
  IF sender = Open THEN SpecAttemptKind(c, sender) ELSE NoEnvelope

ActualDeliveredKind(c, sender) ==
  CASE Bug = "closed_delivers"
       /\ sender = Closed -> ActualAttemptKind(c, sender)
    [] Bug = "success_drops_single"
       /\ sender = Open
       /\ c = SingleA -> NoEnvelope
    [] Bug = "success_drops_batch"
       /\ sender = Open
       /\ Len(InputEvents(c)) > 1 -> NoEnvelope
    [] OTHER ->
       IF sender = Open THEN ActualAttemptKind(c, sender) ELSE NoEnvelope

SpecDeliveredPayload(c, sender) ==
  IF SpecDeliveredKind(c, sender) = NoEnvelope
  THEN <<>>
  ELSE InputEvents(c)

ActualDeliveredPayload(c, sender) ==
  IF ActualDeliveredKind(c, sender) = NoEnvelope
  THEN <<>>
  ELSE ActualAttemptPayload(c, sender)

SpecFailureLogged(c, sender) ==
  sender = Closed /\ SpecAttemptCount(c, sender) = 1

ActualFailureLogged(c, sender) ==
  CASE Bug = "missing_failure_log"
       /\ sender = Closed
       /\ c = SingleA -> FALSE
    [] Bug = "logs_success"
       /\ sender = Open
       /\ c = SingleA -> TRUE
    [] Bug = "logs_empty"
       /\ c = Empty -> TRUE
    [] OTHER -> SpecFailureLogged(c, sender)

SpecPanics(c, sender) ==
  FALSE

ActualPanics(c, sender) ==
  CASE Bug = "panic_on_closed"
       /\ sender = Closed
       /\ Len(InputEvents(c)) > 0 -> TRUE
    [] OTHER -> SpecPanics(c, sender)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 10
     /\ checked' = checked + 1
  \/ /\ checked = 10
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_sends_batch",
       "multi_split_attempts",
       "closed_skips_attempt",
       "single_sent_as_batch",
       "multi_sent_as_single",
       "multi_dropped",
       "single_wrong_event",
       "reverse_batch_order",
       "drop_batch_tail",
       "duplicate_batch_dedup",
       "closed_delivers",
       "success_drops_single",
       "success_drops_batch",
       "missing_failure_log",
       "logs_success",
       "logs_empty",
       "panic_on_closed"
     }
  /\ checked \in 0..10
  /\ \A c \in InputCases:
       /\ InputEvents(c) \in EventSeqs
       /\ \A sender \in SenderCases:
            /\ ActualAttemptCount(c, sender) \in 0..3
            /\ ActualAttemptKind(c, sender) \in EnvelopeKinds
            /\ ActualAttemptPayload(c, sender) \in EventSeqs
            /\ ActualDeliveredKind(c, sender) \in EnvelopeKinds
            /\ ActualDeliveredPayload(c, sender) \in EventSeqs
            /\ ActualFailureLogged(c, sender) \in BOOLEAN
            /\ ActualPanics(c, sender) \in BOOLEAN

AttemptCountExact ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualAttemptCount(c, sender) = SpecAttemptCount(c, sender)

AttemptKindExact ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualAttemptKind(c, sender) = SpecAttemptKind(c, sender)

AttemptPayloadExact ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualAttemptPayload(c, sender) = SpecAttemptPayload(c, sender)

DeliveredKindExact ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualDeliveredKind(c, sender) = SpecDeliveredKind(c, sender)

DeliveredPayloadExact ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualDeliveredPayload(c, sender) = SpecDeliveredPayload(c, sender)

FailureLoggingExact ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualFailureLogged(c, sender) = SpecFailureLogged(c, sender)

NoPanic ==
  \A c \in InputCases:
    \A sender \in SenderCases:
      ActualPanics(c, sender) = FALSE

EmptyInputIsNoop ==
  \A sender \in SenderCases:
    /\ ActualAttemptCount(Empty, sender) = 0
    /\ ActualAttemptKind(Empty, sender) = NoEnvelope
    /\ ActualDeliveredKind(Empty, sender) = NoEnvelope
    /\ ActualFailureLogged(Empty, sender) = FALSE

SingleEventUsesPipelineEnvelope ==
  \A sender \in SenderCases:
    /\ ActualAttemptCount(SingleA, sender) = 1
    /\ ActualAttemptKind(SingleA, sender) = PipelineEnvelope
    /\ ActualAttemptPayload(SingleA, sender) = <<1>>

MultipleEventsUseOneOrderedBatch ==
  \A sender \in SenderCases:
    /\ ActualAttemptCount(TwoAB, sender) = 1
    /\ ActualAttemptKind(TwoAB, sender) = PipelineBatchEnvelope
    /\ ActualAttemptPayload(TwoAB, sender) = <<1, 2>>
    /\ ActualAttemptCount(ThreeABC, sender) = 1
    /\ ActualAttemptKind(ThreeABC, sender) = PipelineBatchEnvelope
    /\ ActualAttemptPayload(ThreeABC, sender) = <<1, 2, 3>>

DuplicateEventsArePreserved ==
  \A sender \in SenderCases:
    /\ ActualAttemptKind(TwoDuplicate, sender) = PipelineBatchEnvelope
    /\ ActualAttemptPayload(TwoDuplicate, sender) = <<2, 2>>

ClosedSenderDoesNotDeliver ==
  \A c \in InputCases:
    /\ ActualDeliveredKind(c, Closed) = NoEnvelope
    /\ ActualDeliveredPayload(c, Closed) = <<>>

OpenSenderDeliversAttemptedEnvelope ==
  \A c \in InputCases:
    /\ ActualDeliveredKind(c, Open) = ActualAttemptKind(c, Open)
    /\ ActualDeliveredPayload(c, Open) = ActualAttemptPayload(c, Open)

AttemptEnvelopeAnchors ==
  /\ AttemptCountExact
  /\ AttemptKindExact
  /\ AttemptPayloadExact
  /\ EmptyInputIsNoop
  /\ SingleEventUsesPipelineEnvelope
  /\ MultipleEventsUseOneOrderedBatch
  /\ DuplicateEventsArePreserved

DeliveryAnchors ==
  /\ DeliveredKindExact
  /\ DeliveredPayloadExact
  /\ ClosedSenderDoesNotDeliver
  /\ OpenSenderDeliversAttemptedEnvelope

FailureAndPanicAnchors ==
  /\ FailureLoggingExact
  /\ NoPanic
  /\ ActualFailureLogged(SingleA, Closed)
  /\ ~ActualFailureLogged(SingleA, Open)
  /\ ~ActualPanics(SingleA, Closed)

PipelineEventEmissionSafetyAnchors ==
  /\ AttemptEnvelopeAnchors
  /\ DeliveryAnchors
  /\ FailureAndPanicAnchors

PipelineEventEmissionCoreSafety ==
  /\ AttemptCountExact
  /\ AttemptKindExact
  /\ AttemptPayloadExact
  /\ DeliveredKindExact
  /\ DeliveredPayloadExact
  /\ FailureLoggingExact
  /\ NoPanic
  /\ EmptyInputIsNoop
  /\ SingleEventUsesPipelineEnvelope
  /\ MultipleEventsUseOneOrderedBatch
  /\ DuplicateEventsArePreserved
  /\ ClosedSenderDoesNotDeliver
  /\ OpenSenderDeliversAttemptedEnvelope

SafetyFast ==
  PipelineEventEmissionCoreSafety

Safety == PipelineEventEmissionSafetyAnchors

PipelineEventEmissionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SafetyFast
  /\ PipelineEventEmissionSafetyAnchors

====
