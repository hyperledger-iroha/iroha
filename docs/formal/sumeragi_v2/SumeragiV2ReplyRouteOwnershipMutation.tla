---- MODULE SumeragiV2ReplyRouteOwnershipMutation ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
Adversarial finite prefix for the production-shared reply-route kernel.

Source 0 advances one message for two semantic requests.  An exact duplicate
and a later route observation retain the first request's rank.  Source 1 is
then attached at cursor zero and independently advances one message.  The
fixed path retires source 0, reconnects the first request at its retained
cursor, and then rebinds the second request to the source-scoped new tenure at
its own retained cursor.  The two mutations either reset source 0's cursor
during reconnect or replace its independently owned attempts during a later
delivery.
***************************************************************************)

CONSTANT RouteMutationMode

MutationOwners == {0}
MutationSourceOrder == <<0, 1>>
MutationSources ==
  {MutationSourceOrder[index]: index \in 1..Len(MutationSourceOrder)}
MutationSemantics == {"request-a", "request-b"}
MutationSourceCapacity == 2
MutationDeliveryOrdinalLimit == 8
MutationMessageCount == 2
MutationChunkCount == 2

VARIABLES
  attempts,
  payloads,
  nextDeliveryOrdinal,
  connectionTenure,
  sourceActive,
  nextServiceIndex,
  phase

MutationRoute ==
  INSTANCE SumeragiV2ReplyRouteOwnership WITH
    ReplyOwners <- MutationOwners,
    ReplySourceOrder <- MutationSourceOrder,
    ReplySemantics <- MutationSemantics,
    ReplySourceCapacity <- MutationSourceCapacity,
    ReplyDeliveryOrdinalLimit <- MutationDeliveryOrdinalLimit,
    ReplyMessageCount <- MutationMessageCount,
    ReplyChunkCount <- MutationChunkCount,
    rrAttempts <- attempts,
    rrPayloads <- payloads,
    rrNextDeliveryOrdinal <- nextDeliveryOrdinal,
    rrConnectionTenure <- connectionTenure,
    rrSourceActive <- sourceActive,
    rrNextServiceIndex <- nextServiceIndex

MutationRouteVars == MutationRoute!ReplyRouteVars
MutationVars == <<MutationRouteVars, phase>>

RequestA == "request-a"
RequestB == "request-b"

SourceAttempt(semantic, source) ==
  MutationRoute!ReplyAttemptFor(0, semantic, source)

AdvancePhase(action, nextPhase) ==
  /\ action
  /\ phase' = nextPhase

BuggyReconnectResetsCursor ==
  LET oldAttempt == SourceAttempt(RequestA, 0)
      deliveryOrdinal == nextDeliveryOrdinal[0]
      newTenure == connectionTenure[0][0] + 1
      routedAttempt ==
        MutationRoute!ReplyAttemptWithRoute(
          oldAttempt, deliveryOrdinal, newTenure)
      resetAttempt ==
        [routedAttempt EXCEPT
           !.messageCursor = 0,
           !.chunkCursor = 0]
  IN /\ phase = 13
     /\ ~sourceActive[0][0]
     /\ oldAttempt.messageCursor = 1
     /\ attempts' =
          MutationRoute!ReplaceReplyAttempt(oldAttempt, resetAttempt)
     /\ connectionTenure' =
          [connectionTenure EXCEPT ![0][0] = newTenure]
     /\ sourceActive' = [sourceActive EXCEPT ![0][0] = TRUE]
     /\ nextDeliveryOrdinal' =
          [nextDeliveryOrdinal EXCEPT ![0] = @ + 1]
     /\ UNCHANGED <<payloads, nextServiceIndex>>
     /\ phase' = 14

BuggyLaterDeliveryReplacesAlternateSource ==
  LET oldAttempt == SourceAttempt(RequestA, 0)
      deliveryOrdinal == nextDeliveryOrdinal[0]
      updatedAttempt ==
        MutationRoute!ReplyAttemptWithRoute(
          oldAttempt, deliveryOrdinal, connectionTenure[0][0])
  IN /\ phase = 11
     /\ MutationRoute!ReplyAttemptOwned(0, RequestA, 1)
     /\ MutationRoute!ReplyAttemptOwned(0, RequestB, 0)
     /\ attempts' = {updatedAttempt}
     /\ nextDeliveryOrdinal' =
          [nextDeliveryOrdinal EXCEPT ![0] = @ + 1]
     /\ UNCHANGED <<payloads, connectionTenure, sourceActive,
                    nextServiceIndex>>
     /\ phase' = 15

RouteMutationInit ==
  /\ MutationRoute!ReplyRouteInit
  /\ phase = 0

RouteMutationNext ==
  \/ /\ phase = 0
     /\ AdvancePhase(
          MutationRoute!ObserveNewReplySource(0, RequestA, 0), 1)
  \/ /\ phase = 1
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicket(0, RequestA, 0), 2)
  \/ /\ phase = 2
     /\ AdvancePhase(
          MutationRoute!ServiceReplyRoute(0, RequestA), 3)
  \/ /\ phase = 3
     /\ AdvancePhase(
          MutationRoute!ObserveNewReplySource(0, RequestB, 0), 4)
  \/ /\ phase = 4
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicket(0, RequestB, 0), 5)
  \/ /\ phase = 5
     /\ AdvancePhase(
          MutationRoute!ServiceReplyRoute(0, RequestB), 6)
  \/ /\ phase = 6
     /\ AdvancePhase(
          MutationRoute!RetryExactReplySource(0, RequestA, 0), 7)
  \/ /\ phase = 7
     /\ AdvancePhase(
          MutationRoute!ObserveLaterReplyDelivery(0, RequestA, 0), 8)
  \/ /\ phase = 8
     /\ AdvancePhase(
          MutationRoute!ObserveNewReplySource(0, RequestA, 1), 9)
  \/ /\ phase = 9
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicket(0, RequestA, 1), 10)
  \/ /\ phase = 10
     /\ AdvancePhase(
          MutationRoute!ServiceReplyRoute(0, RequestA), 11)
  \/ /\ phase = 11
     /\ RouteMutationMode \in {"Fixed", "CursorReset"}
     /\ AdvancePhase(
          MutationRoute!ObserveLaterReplyDelivery(0, RequestA, 0), 12)
  \/ /\ RouteMutationMode = "SourceReplacement"
     /\ BuggyLaterDeliveryReplacesAlternateSource
  \/ /\ phase = 12
     /\ RouteMutationMode \in {"Fixed", "CursorReset"}
     /\ AdvancePhase(MutationRoute!RetireReplySource(0, 0), 13)
  \/ /\ phase = 13
     /\ RouteMutationMode = "Fixed"
     /\ AdvancePhase(
          MutationRoute!ReconnectReplySource(0, RequestA, 0), 14)
  \/ /\ RouteMutationMode = "CursorReset"
     /\ BuggyReconnectResetsCursor
  \/ /\ phase = 14
     /\ RouteMutationMode \in {"Fixed", "CursorReset"}
     /\ AdvancePhase(
          MutationRoute!ObserveLaterReplyDelivery(0, RequestB, 0), 15)

BothSemanticAttemptsRetained ==
  phase < 4
    \/ /\ MutationRoute!ReplyAttemptOwned(0, RequestA, 0)
       /\ MutationRoute!ReplyAttemptOwned(0, RequestB, 0)

BothSourcesRetained ==
  phase < 9
    \/ /\ MutationRoute!ReplyAttemptOwned(0, RequestA, 0)
       /\ MutationRoute!ReplyAttemptOwned(0, RequestA, 1)

ExactAndLaterDuplicatesKeepRank ==
  phase < 7
    \/ phase >= 14
    \/ /\ SourceAttempt(RequestA, 0).messageCursor = 1
       /\ SourceAttempt(RequestA, 0).chunkCursor = 0

NewAlternateStartsAtZero ==
  phase \notin {9, 10}
    \/ /\ SourceAttempt(RequestA, 1).messageCursor = 0
       /\ SourceAttempt(RequestA, 1).chunkCursor = 0

ReconnectPreservesCurrentCursor ==
  phase < 14
    \/ connectionTenure[0][0] = 1
    \/ /\ SourceAttempt(RequestA, 0).messageCursor = 1
       /\ SourceAttempt(RequestA, 0).chunkCursor = 0
       /\ SourceAttempt(RequestA, 0).ticketTenure =
              MutationRoute!NoReplyTicketTenure
       /\ SourceAttempt(RequestA, 1).messageCursor = 1
       /\ SourceAttempt(RequestA, 1).chunkCursor = 0

PerAttemptRebindPreservesCurrentCursor ==
  LET attemptsA ==
        MutationRoute!ReplyAttemptsForSource(0, RequestA, 0)
      attemptsB ==
        MutationRoute!ReplyAttemptsForSource(0, RequestB, 0)
  IN phase < 15
       \/ /\ Cardinality(attemptsA) = 1
          /\ Cardinality(attemptsB) = 1
          /\ \A attempt \in attemptsA \cup attemptsB:
               /\ attempt.connectionTenure = 2
               /\ attempt.ticketTenure =
                    MutationRoute!NoReplyTicketTenure
               /\ attempt.messageCursor = 1
               /\ attempt.chunkCursor = 0
          /\ \A attempt \in attemptsB:
               attempt.deliveryOrdinal = 7

RouteMutationSafety ==
  /\ MutationRoute!ReplyRouteSafetyInvariant
  /\ BothSemanticAttemptsRetained
  /\ BothSourcesRetained
  /\ ExactAndLaterDuplicatesKeepRank
  /\ NewAlternateStartsAtZero
  /\ ReconnectPreservesCurrentCursor
  /\ PerAttemptRebindPreservesCurrentCursor

RouteMutationTemporalProperties ==
  /\ MutationRoute!ReplyTenureAwareReplay
  /\ MutationRoute!ReplySourceIsolation

=============================================================================
