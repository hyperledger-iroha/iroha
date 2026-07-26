---- MODULE SumeragiV2ReplyRouteOwnershipMutation ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
Adversarial finite prefix for the production-shared reply-route kernel.

Source 0 advances one message for two semantic requests.  An exact duplicate
and a later route observation retain the first request's rank.  Source 1 is
then attached at cursor zero and independently advances one message.  The
fixed path retires source 0, reconnects the first request at its retained
cursor, and then rebinds the second request to the source-scoped new tenure at
its own retained cursor.  The mutations reset a reconnect cursor, replace
independent attempts, retain a sibling semantic ticket across a source-wide
tenure change, accept the authenticated source as a substituted semantic target,
or reuse a consumed ticket for the next cursor payload.  These cases pin the
production distinctions among semantic origin, authenticated source,
connection tenure, and one-item admission authority.
***************************************************************************)

CONSTANT RouteMutationMode

MutationOwners == {0}
MutationSourceOrder == <<0, 1>>
MutationSources ==
  {MutationSourceOrder[index]: index \in 1..Len(MutationSourceOrder)}
MutationSemantics == {"request-a", "request-b"}
MutationTargets == {2, 3}
MutationSemanticTarget(semantic) ==
  IF semantic = "request-a" THEN 2 ELSE 3
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
  semanticSequence,
  semanticHash,
  requesterNextSequence,
  requesterClosedThrough,
  closePendingThrough,
  closeSentThrough,
  closeAcknowledgedThrough,
  closeRetryGeneration,
  serviceGeneration,
  responderGeneration,
  durableResponderGeneration,
  requesterNextStreamEpoch,
  requesterStreamEpoch,
  closeStreamEpoch,
  closedPrefix,
  attemptLifecycleIdentities,
  pendingHintResets,
  discardedPartialIdentities,
  acceptedInvalidCapability,
  phase

MutationRoute ==
  INSTANCE SumeragiV2ReplyRouteOwnership WITH
    ReplyOwners <- MutationOwners,
    ReplySourceOrder <- MutationSourceOrder,
    ReplySemantics <- MutationSemantics,
    ReplyTargets <- MutationTargets,
    ReplySemanticTarget <- MutationSemanticTarget,
    ReplySourceCapacity <- MutationSourceCapacity,
    ReplyDeliveryOrdinalLimit <- MutationDeliveryOrdinalLimit,
    ReplyMessageCount <- MutationMessageCount,
    ReplyChunkCount <- MutationChunkCount,
    rrAttempts <- attempts,
    rrPayloads <- payloads,
    rrNextDeliveryOrdinal <- nextDeliveryOrdinal,
    rrConnectionTenure <- connectionTenure,
    rrSourceActive <- sourceActive,
    rrNextServiceIndex <- nextServiceIndex,
    rrSemanticSequence <- semanticSequence,
    rrSemanticHash <- semanticHash,
    rrRequesterNextSequence <- requesterNextSequence,
    rrRequesterClosedThrough <- requesterClosedThrough,
    rrClosePendingThrough <- closePendingThrough,
    rrCloseSentThrough <- closeSentThrough,
    rrCloseAcknowledgedThrough <- closeAcknowledgedThrough,
    rrCloseRetryGeneration <- closeRetryGeneration,
    rrServiceGeneration <- serviceGeneration,
    rrResponderGeneration <- responderGeneration,
    rrDurableResponderGeneration <- durableResponderGeneration,
    rrRequesterNextStreamEpoch <- requesterNextStreamEpoch,
    rrRequesterStreamEpoch <- requesterStreamEpoch,
    rrCloseStreamEpoch <- closeStreamEpoch,
    rrClosedPrefix <- closedPrefix,
    rrAttemptLifecycleIdentities <- attemptLifecycleIdentities,
    rrPendingHintResets <- pendingHintResets,
    rrDiscardedPartialIdentities <- discardedPartialIdentities

MutationRouteVars == MutationRoute!ReplyRouteV2Vars
MutationLifecycleVars ==
  <<semanticSequence, semanticHash, requesterNextSequence,
    requesterClosedThrough, closePendingThrough, closeSentThrough,
    closeAcknowledgedThrough, closeRetryGeneration,
    serviceGeneration, responderGeneration, durableResponderGeneration,
    requesterNextStreamEpoch, requesterStreamEpoch, closeStreamEpoch,
    closedPrefix, attemptLifecycleIdentities, pendingHintResets,
    discardedPartialIdentities>>
MutationVars == <<MutationRouteVars, acceptedInvalidCapability, phase>>

RequestA == "request-a"
RequestB == "request-b"
CloseThroughRequestA == 1
CloseRequestA ==
  MutationRoute!ReplyCanonicalCloseWitness(
    0, 0, CloseThroughRequestA)
CloseRequestAAcknowledgement ==
  MutationRoute!ReplyCanonicalCloseAcknowledgement(
    0, 0, CloseThroughRequestA)

ExactGenerationHint ==
  LET observedMessageHash ==
        MutationRoute!ReplyOutstandingRequestHash(
          0, RequestA, 0)
  IN MutationRoute!ReplyGenerationHint(
       0, 0, 0, "Request", RequestA,
       1, 2, observedMessageHash,
       0, 0, 1, 2, observedMessageHash)

ForgedResponderGenerationHint ==
  LET observedMessageHash ==
        MutationRoute!ReplyOutstandingRequestHash(
          0, RequestA, 0)
  IN MutationRoute!ReplyGenerationHint(
       0, 0, 1, "Request", RequestA,
       1, 2, observedMessageHash,
       0, 0, 1, 2, observedMessageHash)

UncorrelatedGenerationHint ==
  LET observedMessageHash ==
        {MutationRoute!ReplyCanonicalRequestIdentity(
           1, 1, 2, RequestB, 0, 0)}
  IN MutationRoute!ReplyGenerationHint(
       0, 0, 0, "Request", RequestA,
       1, 2, observedMessageHash,
       0, 0, 1, 2, observedMessageHash)

PendingGenerationReset ==
  CHOOSE reset \in pendingHintResets: TRUE

SourceAttempt(semantic, source) ==
  MutationRoute!ReplyAttemptFor(0, semantic, source)

AdvancePhase(action, nextPhase) ==
  /\ action
  /\ UNCHANGED acceptedInvalidCapability
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
     /\ UNCHANGED <<payloads, nextServiceIndex,
                    acceptedInvalidCapability>>
     /\ UNCHANGED MutationLifecycleVars
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
                    nextServiceIndex, acceptedInvalidCapability>>
     /\ UNCHANGED MutationLifecycleVars
     /\ phase' = 15

SourceAsTargetCapability(semantic, source) ==
  MutationRoute!ReplyCapability(
    0, source, source, semantic, nextDeliveryOrdinal[0],
    connectionTenure[0][source])

SourceAsTargetCapabilityRejected ==
  \A semantic \in MutationSemantics, source \in MutationSources:
    /\ MutationSemanticTarget(semantic) # source
    /\ MutationRoute!ReplyCapabilityRejection(
         SourceAsTargetCapability(semantic, source),
         0, source, semantic) = "Retargeted"
    /\ ~MutationRoute!ReplyCapabilityValidFor(
         SourceAsTargetCapability(semantic, source),
         0, source, semantic)

IntrinsicTenureSubstitutionCapability ==
  LET minted ==
        MutationRoute!ReplyCapability(
          0, 0, MutationSemanticTarget(RequestA), RequestA,
          nextDeliveryOrdinal[0], connectionTenure[0][0])
  IN [minted EXCEPT !.connectionTenure = @ + 1]

IntrinsicTenureSubstitutionRejected ==
  /\ MutationRoute!ReplyCapabilityRejection(
       IntrinsicTenureSubstitutionCapability, 0, 0, RequestA) =
       "EqualOrdinalDifferentTenure"
  /\ ~MutationRoute!ReplyCapabilityValidFor(
       IntrinsicTenureSubstitutionCapability, 0, 0, RequestA)

SourceCapacitySubstitutionCapability ==
  LET minted ==
        MutationRoute!ReplyCapability(
          0, 0, MutationSemanticTarget(RequestA), RequestA,
          nextDeliveryOrdinal[0], connectionTenure[0][0])
  IN [minted EXCEPT
        !.sourceCapacity = 1,
        !.bindingSourceCapacity = 1]

SourceCapacitySubstitutionRejected ==
  /\ MutationRoute!ReplyCapabilityIntrinsicBindingValid(
       SourceCapacitySubstitutionCapability)
  /\ MutationRoute!ReplyCapabilityRejection(
       SourceCapacitySubstitutionCapability, 0, 0, RequestA) =
       "ForeignOwner"
  /\ ~MutationRoute!ReplyCapabilityValidFor(
       SourceCapacitySubstitutionCapability, 0, 0, RequestA)

BuggyAcceptSourceAsSemanticTarget ==
  /\ phase = 0
  /\ SourceAsTargetCapabilityRejected
  /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                 connectionTenure, sourceActive, nextServiceIndex>>
  /\ UNCHANGED MutationLifecycleVars
  /\ acceptedInvalidCapability' = TRUE
  /\ phase' = 16

BuggyAcceptIntrinsicTenureSubstitution ==
  /\ phase = 0
  /\ IntrinsicTenureSubstitutionRejected
  /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                 connectionTenure, sourceActive, nextServiceIndex>>
  /\ UNCHANGED MutationLifecycleVars
  /\ acceptedInvalidCapability' = TRUE
  /\ phase' = 20

BuggyAcceptSourceCapacitySubstitution ==
  /\ phase = 0
  /\ SourceCapacitySubstitutionRejected
  /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                 connectionTenure, sourceActive, nextServiceIndex>>
  /\ UNCHANGED MutationLifecycleVars
  /\ acceptedInvalidCapability' = TRUE
  /\ phase' = 21

BuggyReuseTicketForNextPayload ==
  LET oldAttempt == SourceAttempt(RequestA, 0)
      serviced == MutationRoute!ReplyAttemptAfterService(oldAttempt)
      reused ==
        [serviced EXCEPT
           !.ticketTenure = oldAttempt.ticketTenure,
           !.ticketSemantic = oldAttempt.ticketSemantic,
           !.ticketTarget = oldAttempt.ticketTarget,
           !.ticketMessageCursor = oldAttempt.ticketMessageCursor,
           !.ticketChunkCursor = oldAttempt.ticketChunkCursor]
      selectedIndex ==
        MutationRoute!ReplySelectedSourceIndex(0, RequestA)
  IN /\ phase = 2
     /\ attempts' = MutationRoute!ReplaceReplyAttempt(oldAttempt, reused)
     /\ nextServiceIndex' =
          [nextServiceIndex EXCEPT
             ![0][RequestA] =
               MutationRoute!NextReplySourceIndex(selectedIndex)]
     /\ UNCHANGED <<payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive,
                    acceptedInvalidCapability>>
     /\ UNCHANGED MutationLifecycleVars
     /\ phase' = 3

(***************************************************************************
This mutation collapses teardown and reconnect into one source-tenure change,
but clears only the selected semantic attempt.  Request B therefore retains a
ticket minted by tenure 1 after source 0 advances to tenure 2.  A correct
source-wide invalidation changes no sibling route/cursor, but clears that
ticket atomically.
***************************************************************************)
BuggyReconnectRetainsSiblingTicket ==
  LET selectedAttempt == SourceAttempt(RequestA, 0)
      siblingAttempt == SourceAttempt(RequestB, 0)
      deliveryOrdinal == nextDeliveryOrdinal[0]
      newTenure == connectionTenure[0][0] + 1
      routedAttempt ==
        MutationRoute!ReplyAttemptWithRoute(
          selectedAttempt, deliveryOrdinal, newTenure)
  IN /\ phase = 17
     /\ MutationRoute!ReplyTicketValidForAttempt(siblingAttempt)
     /\ attempts' =
          MutationRoute!ReplaceReplyAttempt(
            selectedAttempt, routedAttempt)
     /\ connectionTenure' =
          [connectionTenure EXCEPT ![0][0] = newTenure]
     /\ nextDeliveryOrdinal' =
          [nextDeliveryOrdinal EXCEPT ![0] = @ + 1]
     /\ UNCHANGED <<payloads, sourceActive, nextServiceIndex,
                    acceptedInvalidCapability>>
     /\ UNCHANGED MutationLifecycleVars
     /\ phase' = 18

RetiredOrdinalCollisionCapability ==
  MutationRoute!ReplyCapability(
    0, 1, MutationSemanticTarget(RequestA), RequestA,
    SourceAttempt(RequestA, 0).retiredDeliveryOrdinal,
    connectionTenure[0][1])

RetiredOrdinalCollisionRejected ==
  /\ SourceAttempt(RequestA, 0).retiredDeliveryOrdinal # 0
  /\ MutationRoute!ReplyCapabilityRejection(
       RetiredOrdinalCollisionCapability, 0, 1, RequestA) =
       "EqualOrdinalDifferentTenure"
  /\ ~MutationRoute!ReplyCapabilityValidFor(
       RetiredOrdinalCollisionCapability, 0, 1, RequestA)

BuggyAcceptRetiredOrdinalCollision ==
  /\ phase = 14
  /\ RetiredOrdinalCollisionRejected
  /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                 connectionTenure, sourceActive, nextServiceIndex>>
  /\ UNCHANGED MutationLifecycleVars
  /\ acceptedInvalidCapability' = TRUE
  /\ phase' = 19

RouteMutationInit ==
  /\ MutationRoute!ReplyRouteInit
  /\ serviceGeneration =
       [owner \in MutationOwners |->
          [source \in MutationSources |->
             IF RouteMutationMode = "CapacityOverflowFixed"
             THEN MutationDeliveryOrdinalLimit
             ELSE 1]]
  /\ responderGeneration =
       [source \in MutationSources |->
          IF RouteMutationMode = "CapacityOverflowFixed"
          THEN MutationDeliveryOrdinalLimit
          ELSE 1]
  /\ durableResponderGeneration =
       [source \in MutationSources |->
          IF RouteMutationMode = "CapacityOverflowFixed"
          THEN MutationDeliveryOrdinalLimit
          ELSE 1]
  /\ requesterNextStreamEpoch =
       [owner \in MutationOwners |->
          IF RouteMutationMode = "CapacityOverflowFixed"
          THEN MutationDeliveryOrdinalLimit + 1
          ELSE 2]
  /\ requesterStreamEpoch =
       [owner \in MutationOwners |->
          [source \in MutationSources |-> 1]]
  /\ closeStreamEpoch = requesterStreamEpoch
  /\ closedPrefix =
       [owner \in MutationOwners |->
          [source \in MutationSources |->
             MutationRoute!ReplyOccurrenceCoordinate(0, 0, 0)]]
  /\ attemptLifecycleIdentities = {}
  /\ pendingHintResets = {}
  /\ discardedPartialIdentities = {}
  /\ acceptedInvalidCapability = FALSE
  /\ phase = 0

ClosePendingRetryStep ==
  /\ phase = 22
  /\ RouteMutationMode = "CloseLifecycleFixed"
  /\ AdvancePhase(
       MutationRoute!RetryCloseSemanticRequestV2(CloseRequestA), 23)

CloseAcknowledgementStep ==
  /\ phase = 23
  /\ RouteMutationMode = "CloseLifecycleFixed"
  /\ AdvancePhase(
       MutationRoute!AcknowledgeCloseSemanticRequestV2(
         CloseRequestAAcknowledgement), 24)

GenerationPersistStep ==
  /\ phase = 0
  /\ RouteMutationMode = "GenerationEpochFixed"
  /\ AdvancePhase(
       MutationRoute!PersistTerminalResponderGeneration(0), 40)

GenerationInstallStep ==
  /\ phase = 40
  /\ RouteMutationMode = "GenerationEpochFixed"
  /\ AdvancePhase(
       MutationRoute!InstallPersistedResponderGeneration(0), 41)

GenerationStaleRequestStep ==
  /\ phase = 41
  /\ RouteMutationMode = "GenerationEpochFixed"
  /\ AdvancePhase(
       MutationRoute!ObserveNewReplySourceV2(
         0, RequestA, 0), 42)

GenerationHintPersistenceStep ==
  /\ phase = 42
  /\ RouteMutationMode = "GenerationEpochFixed"
  /\ AdvancePhase(
       MutationRoute!PersistFreshEpochForGenerationHint(
         ExactGenerationHint), 43)

GenerationDiscardStep ==
  /\ phase = 43
  /\ RouteMutationMode = "GenerationEpochFixed"
  /\ AdvancePhase(
       MutationRoute!DiscardPersistedHintPartialState(
         PendingGenerationReset), 44)

FutureGenerationRejectStep ==
  /\ phase = 44
  /\ RouteMutationMode = "GenerationEpochFixed"
  /\ AdvancePhase(
       MutationRoute!RejectFutureGenerationWithoutMutation(
         0, 0, 3), 45)

CapacityActiveObserveStep ==
  /\ phase = 0
  /\ RouteMutationMode = "CapacityOverflowFixed"
  /\ AdvancePhase(
       MutationRoute!ObserveNewReplySourceV2(
         0, RequestA, 0), 50)

CapacityActiveCompactionRejectStep ==
  /\ phase = 50
  /\ RouteMutationMode = "CapacityOverflowFixed"
  /\ AdvancePhase(
       MutationRoute!
         RejectNonTerminalResponderCompactionWithoutMutation(0), 51)

CapacityEpochOverflowRejectStep ==
  /\ phase = 51
  /\ RouteMutationMode = "CapacityOverflowFixed"
  /\ AdvancePhase(
       MutationRoute!
         RejectRequesterEpochOverflowWithoutMutation(0), 52)

CapacityGenerationOverflowRejectStep ==
  /\ phase = 52
  /\ RouteMutationMode = "CapacityOverflowFixed"
  /\ AdvancePhase(
       MutationRoute!RejectResponderGenerationOverflow(0), 53)

RouteMutationNext ==
  \/ GenerationPersistStep
  \/ GenerationInstallStep
  \/ GenerationStaleRequestStep
  \/ GenerationHintPersistenceStep
  \/ GenerationDiscardStep
  \/ FutureGenerationRejectStep
  \/ CapacityActiveObserveStep
  \/ CapacityActiveCompactionRejectStep
  \/ CapacityEpochOverflowRejectStep
  \/ CapacityGenerationOverflowRejectStep
  \/ /\ phase = 0
     /\ RouteMutationMode \notin
          {"TargetSubstitution", "IntrinsicTenureSubstitution",
           "SourceCapacitySubstitution", "GenerationEpochFixed",
           "CapacityOverflowFixed"}
     /\ AdvancePhase(
          MutationRoute!ObserveNewReplySourceV2(0, RequestA, 0), 1)
  \/ /\ RouteMutationMode = "TargetSubstitution"
     /\ BuggyAcceptSourceAsSemanticTarget
  \/ /\ RouteMutationMode = "IntrinsicTenureSubstitution"
     /\ BuggyAcceptIntrinsicTenureSubstitution
  \/ /\ RouteMutationMode = "SourceCapacitySubstitution"
     /\ BuggyAcceptSourceCapacitySubstitution
  \/ /\ phase = 1
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicketV2(0, RequestA, 0), 2)
  \/ /\ phase = 2
     /\ RouteMutationMode # "TicketPayloadReuse"
     /\ AdvancePhase(
          MutationRoute!ServiceReplyRouteV2(0, RequestA), 3)
  \/ /\ RouteMutationMode = "TicketPayloadReuse"
     /\ BuggyReuseTicketForNextPayload
  \/ /\ phase = 3
     /\ AdvancePhase(
          MutationRoute!ObserveNewReplySourceV2(0, RequestB, 0), 4)
  \/ /\ phase = 4
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicketV2(0, RequestB, 0), 5)
  \/ /\ phase = 5
     /\ AdvancePhase(
          MutationRoute!ServiceReplyRouteV2(0, RequestB), 6)
  \/ /\ phase = 6
     /\ AdvancePhase(
          MutationRoute!RetryExactReplySourceV2(0, RequestA, 0), 7)
  \/ /\ phase = 7
     /\ AdvancePhase(
          MutationRoute!ObserveLaterReplyDeliveryV2(0, RequestA, 0), 8)
  \/ /\ phase = 8
     /\ AdvancePhase(
          MutationRoute!ObserveNewReplySourceV2(0, RequestA, 1), 9)
  \/ /\ phase = 9
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicketV2(0, RequestA, 1), 10)
  \/ /\ phase = 10
     /\ AdvancePhase(
          MutationRoute!ServiceReplyRouteV2(0, RequestA), 11)
  \/ /\ phase = 11
     /\ RouteMutationMode \in
          {"Fixed", "CloseLifecycleFixed", "CursorReset",
           "RetiredOrdinalCollision"}
     /\ AdvancePhase(
          MutationRoute!ObserveLaterReplyDeliveryV2(0, RequestA, 0), 12)
  \/ /\ RouteMutationMode = "SourceReplacement"
     /\ BuggyLaterDeliveryReplacesAlternateSource
  \/ /\ phase = 11
     /\ RouteMutationMode = "ReconnectSiblingTicket"
     /\ AdvancePhase(
          MutationRoute!AcquireReplyTicketV2(0, RequestB, 0), 17)
  \/ /\ RouteMutationMode = "ReconnectSiblingTicket"
     /\ BuggyReconnectRetainsSiblingTicket
  \/ /\ phase = 12
     /\ RouteMutationMode \in
          {"Fixed", "CloseLifecycleFixed", "CursorReset",
           "RetiredOrdinalCollision"}
     /\ AdvancePhase(MutationRoute!RetireReplySourceV2(0, 0), 13)
  \/ /\ phase = 13
     /\ RouteMutationMode
          \in {"Fixed", "CloseLifecycleFixed",
               "RetiredOrdinalCollision"}
     /\ AdvancePhase(
          MutationRoute!ReconnectReplySourceV2(0, RequestA, 0), 14)
  \/ /\ RouteMutationMode = "CursorReset"
     /\ BuggyReconnectResetsCursor
  \/ /\ phase = 14
     /\ RouteMutationMode \in {"Fixed", "CloseLifecycleFixed",
                               "CursorReset"}
     /\ AdvancePhase(
          MutationRoute!ObserveLaterReplyDeliveryV2(0, RequestB, 0), 15)
  \/ /\ RouteMutationMode = "RetiredOrdinalCollision"
     /\ BuggyAcceptRetiredOrdinalCollision
  \/ /\ phase = 15
     /\ RouteMutationMode = "CloseLifecycleFixed"
     /\ AdvancePhase(
          MutationRoute!CloseSemanticRequestV2(CloseRequestA), 22)
  \/ ClosePendingRetryStep
  \/ CloseAcknowledgementStep
  \/ /\ phase = 24
     /\ RouteMutationMode = "CloseLifecycleFixed"
     /\ AdvancePhase(
          MutationRoute!RetryCloseSemanticRequestV2(CloseRequestA), 25)
  \/ /\ phase = 25
     /\ RouteMutationMode = "CloseLifecycleFixed"
     /\ AdvancePhase(
          MutationRoute!AcknowledgeCloseSemanticRequestV2(
            CloseRequestAAcknowledgement), 26)

BothSemanticAttemptsRetained ==
  phase < 4
    \/ phase >= 22
    \/ /\ MutationRoute!ReplyAttemptOwned(0, RequestA, 0)
       /\ MutationRoute!ReplyAttemptOwned(0, RequestB, 0)

BothSourcesRetained ==
  phase < 9
    \/ phase >= 22
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
    \/ phase >= 22
    \/ connectionTenure[0][0] = 1
    \/ /\ SourceAttempt(RequestA, 0).messageCursor = 1
       /\ SourceAttempt(RequestA, 0).chunkCursor = 0
       /\ MutationRoute!ReplyAttemptHasNoTicket(
            SourceAttempt(RequestA, 0))
       /\ SourceAttempt(RequestA, 1).messageCursor = 1
       /\ SourceAttempt(RequestA, 1).chunkCursor = 0

PerAttemptRebindPreservesCurrentCursor ==
  LET attemptsA ==
        MutationRoute!ReplyAttemptsForSource(0, RequestA, 0)
      attemptsB ==
        MutationRoute!ReplyAttemptsForSource(0, RequestB, 0)
  IN phase < 15
       \/ phase >= 22
       \/ phase \in {17, 18, 19}
       \/ /\ Cardinality(attemptsA) = 1
          /\ Cardinality(attemptsB) = 1
          /\ \A attempt \in attemptsA \cup attemptsB:
               /\ attempt.connectionTenure = 2
               /\ MutationRoute!ReplyAttemptHasNoTicket(attempt)
               /\ attempt.messageCursor = 1
               /\ attempt.chunkCursor = 0
          /\ \A attempt \in attemptsB:
               attempt.deliveryOrdinal = 7

ConsumedTicketCannotAuthorizeNextPayload ==
  phase < 3
    \/ phase >= 22
    \/ phase = 16
    \/ MutationRoute!ReplyAttemptHasNoTicket(
         SourceAttempt(RequestA, 0))

SourceAsSemanticTargetNeverAccepted ==
  /\ SourceAsTargetCapabilityRejected
  /\ (RouteMutationMode # "TargetSubstitution"
       \/ ~acceptedInvalidCapability)

IntrinsicTenureSubstitutionNeverAccepted ==
  /\ IntrinsicTenureSubstitutionRejected
  /\ (RouteMutationMode # "IntrinsicTenureSubstitution"
       \/ ~acceptedInvalidCapability)

SourceCapacitySubstitutionNeverAccepted ==
  /\ SourceCapacitySubstitutionRejected
  /\ (RouteMutationMode # "SourceCapacitySubstitution"
       \/ ~acceptedInvalidCapability)

RetiredOrdinalCollisionNeverAccepted ==
  RouteMutationMode # "RetiredOrdinalCollision"
    \/ phase < 14
    \/ /\ RetiredOrdinalCollisionRejected
       /\ ~acceptedInvalidCapability

ReconnectInvalidatesEverySemanticTicket ==
  phase # 18
    \/ MutationRoute!ReplySourceHasNoTickets(0, 0)

CloseLifecycleIsCumulativeAndIdempotent ==
  RouteMutationMode # "CloseLifecycleFixed"
    \/ phase < 22
    \/ /\ requesterClosedThrough[0] = CloseThroughRequestA
       /\ MutationRoute!ReplySemanticClosed(0, RequestA)
       /\ MutationRoute!ReplySemanticActive(0, RequestB)
       /\ ~MutationRoute!ReplyAttemptOwned(0, RequestA, 0)
       /\ ~MutationRoute!ReplyAttemptOwned(0, RequestA, 1)
       /\ MutationRoute!ReplyAttemptOwned(0, RequestB, 0)
       /\ closePendingThrough[0][0] = CloseThroughRequestA
       /\ closeSentThrough[0][0] = CloseThroughRequestA
       /\ IF phase < 24
          THEN closeAcknowledgedThrough[0][0] = 0
          ELSE /\ closeAcknowledgedThrough[0][0] =
                    CloseThroughRequestA
               /\ ~MutationRoute!ReplyCloseWorkPending(0, 0)
       /\ IF phase < 23
          THEN closeRetryGeneration[0][0] = 0
          ELSE closeRetryGeneration[0][0] = 1

RouteMutationSafety ==
  /\ MutationRoute!ReplyRouteV2SafetyInvariant
  /\ SourceAsSemanticTargetNeverAccepted
  /\ IntrinsicTenureSubstitutionNeverAccepted
  /\ SourceCapacitySubstitutionNeverAccepted
  /\ RetiredOrdinalCollisionNeverAccepted
  /\ ReconnectInvalidatesEverySemanticTicket
  /\ ConsumedTicketCannotAuthorizeNextPayload
  /\ BothSemanticAttemptsRetained
  /\ BothSourcesRetained
  /\ ExactAndLaterDuplicatesKeepRank
  /\ NewAlternateStartsAtZero
  /\ ReconnectPreservesCurrentCursor
  /\ PerAttemptRebindPreservesCurrentCursor
  /\ CloseLifecycleIsCumulativeAndIdempotent

GenerationEpochLifecycleSafety ==
  /\ RouteMutationSafety
  /\ RouteMutationMode # "GenerationEpochFixed"
       \/ /\ phase \in {0, 40, 41, 42, 43, 44, 45}
          /\ (phase = 40 =>
                /\ durableResponderGeneration[0] = 2
                /\ responderGeneration[0] = 1
                /\ MutationRoute!ReplyResponderStateTerminal(0))
          /\ (phase \in {41, 42} =>
                /\ durableResponderGeneration[0] = 2
                /\ responderGeneration[0] = 2
                /\ serviceGeneration[0][0] = 1)
          /\ (phase = 42 =>
                /\ MutationRoute!ReplyGenerationHintValid(
                     ExactGenerationHint)
                /\ ~MutationRoute!ReplyGenerationHintValid(
                     ForgedResponderGenerationHint)
                /\ ~MutationRoute!ReplyGenerationHintValid(
                     UncorrelatedGenerationHint))
          /\ (phase = 43 =>
                /\ serviceGeneration[0][0] = 2
                /\ requesterStreamEpoch[0][0] = 2
                /\ requesterNextStreamEpoch[0] = 3
                /\ Cardinality(pendingHintResets) = 1
                /\ SourceAttempt(RequestA, 0).messageCursor = 0
                /\ Cardinality(discardedPartialIdentities) = 0)
          /\ (phase \in {44, 45} =>
                LET successor ==
                      MutationRoute!ReplyAttemptLifecycleIdentityFor(
                        0, RequestA, 0)
                IN /\ successor.serviceGeneration = 2
                   /\ successor.streamEpoch = 2
                   /\ successor.semanticSequence = 1
                   /\ Cardinality(pendingHintResets) = 0
                   /\ Cardinality(discardedPartialIdentities) = 1
                   /\ MutationRoute!
                        ReplyStaleArtifactCannotAffectSuccessor)

CapacityOverflowLifecycleSafety ==
  /\ RouteMutationSafety
  /\ RouteMutationMode # "CapacityOverflowFixed"
       \/ /\ phase \in {0, 50, 51, 52, 53}
          /\ responderGeneration[0] = MutationDeliveryOrdinalLimit
          /\ durableResponderGeneration[0] =
               MutationDeliveryOrdinalLimit
          /\ requesterNextStreamEpoch[0] =
               MutationDeliveryOrdinalLimit + 1
          /\ (phase \in {50, 51, 52, 53} =>
                /\ MutationRoute!ReplyAttemptOwned(
                     0, RequestA, 0)
                /\ ~MutationRoute!ReplyResponderStateTerminal(0)
                /\ serviceGeneration[0][0] =
                     MutationDeliveryOrdinalLimit)

RouteMutationTemporalProperties ==
  /\ MutationRoute!ReplyTenureAwareReplay
  /\ MutationRoute!ReplySourceIsolation
  /\ MutationRoute!ReplyLifecycleJournal
  /\ MutationRoute!ReplyCloseWorkEventuallyTerminates(0, 0)

RouteMutationCloseSpec ==
  /\ RouteMutationInit
  /\ [][RouteMutationNext]_MutationVars
  /\ WF_MutationVars(ClosePendingRetryStep)
  /\ WF_MutationVars(CloseAcknowledgementStep)

=============================================================================
