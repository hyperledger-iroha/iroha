---- MODULE SumeragiV2ReplyRouteOwnership ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
Bounded exact-output reply ownership.

This state machine is deliberately independent of connection lifetime.  A
process-local actor allocates one monotonically increasing delivery ordinal
for every newly observed authenticated delivery.  Connection tenure is a
separate source-scoped value.  Canonical semantic request identity excludes
both values, so a request owns at most one attempt per authenticated source.

Every attempt retains its own message and chunk cursor.  An exact duplicate is
a route-state stutter, while a later delivery from the same source preserves
those cursors.  A reconnect changes connection tenure and invalidates the old
admission ticket, but retries the current message/chunk cursor.  Only a newly
attached alternate source starts at <<0, 0>>.  The payload carrier is a set
keyed only by semantic identity, which models immutable byte sharing without
conflating per-source progress.

Semantic ownership has a second, durable identity layer.  Each requester
assigns a cumulative sequence to a canonical semantic hash exactly once,
keeps only a bounded active sequence window, and advances that window only
with an authenticated cumulative close witness.  Connection recovery may
drop process-local tickets and activity, but it never rewinds a semantic
sequence, a close floor, or an attempt cursor.  There is deliberately no
wall-clock expiry: a delayed delivery is either the same active semantic or a
closed semantic which cannot reopen.
***************************************************************************)

CONSTANTS
  ReplyOwners,
  ReplySourceOrder,
  ReplySemantics,
  ReplyTargets,
  ReplySemanticTarget(_),
  ReplySourceCapacity,
  ReplyDeliveryOrdinalLimit,
  ReplyMessageCount,
  ReplyChunkCount

ReplySources ==
  {ReplySourceOrder[index]: index \in 1..Len(ReplySourceOrder)}
ReplyDeliveryOrdinals == 1..ReplyDeliveryOrdinalLimit
ReplyConnectionTenures == 1..ReplyDeliveryOrdinalLimit
ReplySemanticSequences == 1..ReplyDeliveryOrdinalLimit
NoReplyTicketTenure == 0
ReplyActiveWindowCapacity == ReplySourceCapacity

(***************************************************************************
The model keeps the digest symbolic while retaining the production
requirement that canonical bytes determine it.  The singleton is injective
over ReplySemantics, so a hash cannot be rebound to a different semantic.
***************************************************************************)
ReplyCanonicalSemanticHash(semantic) == {semantic}

ReplyRouteConfiguration ==
  /\ IsFiniteSet(ReplyOwners)
  /\ ReplyOwners # {}
  /\ ReplySourceOrder \in Seq(ReplySources)
  /\ Len(ReplySourceOrder) > 0
  /\ Len(ReplySourceOrder) = Cardinality(ReplySources)
  /\ \A left, right \in 1..Len(ReplySourceOrder):
       ReplySourceOrder[left] = ReplySourceOrder[right] => left = right
  /\ ReplySourceCapacity = Len(ReplySourceOrder)
  /\ IsFiniteSet(ReplySemantics)
  /\ ReplySemantics # {}
  /\ IsFiniteSet(ReplyTargets)
  /\ ReplyTargets # {}
  /\ \A semantic \in ReplySemantics:
       ReplySemanticTarget(semantic) \in ReplyTargets
  /\ ReplyDeliveryOrdinalLimit \in Nat \ {0}
  /\ ReplyMessageCount \in Nat \ {0}
  /\ ReplyChunkCount \in Nat \ {0}

ReplyCapability(owner, source, target, semantic, deliveryOrdinal,
                connectionTenure) ==
  [owner |-> owner, source |-> source, target |-> target,
   semantic |-> semantic, deliveryOrdinal |-> deliveryOrdinal,
   connectionTenure |-> connectionTenure,
   sourceCapacity |-> ReplySourceCapacity,
   bindingOwner |-> owner, bindingSource |-> source,
   bindingTarget |-> target, bindingDeliveryOrdinal |-> deliveryOrdinal,
   bindingConnectionTenure |-> connectionTenure,
   bindingSourceCapacity |-> ReplySourceCapacity]

ReplyTicket(owner, source, semantic, target, connectionTenure,
            messageCursor, chunkCursor) ==
  [owner |-> owner, source |-> source,
   semantic |-> {semantic}, target |-> {target},
   connectionTenure |-> connectionTenure,
   messageCursor |-> {messageCursor}, chunkCursor |-> {chunkCursor}]

ReplyAttempt(owner, source, semantic, deliveryOrdinal, connectionTenure,
             retiredDeliveryOrdinal, retiredConnectionTenure,
             ticketTenure, ticketSemantic, ticketTarget,
             ticketMessageCursor, ticketChunkCursor,
             messageCursor, chunkCursor) ==
  [owner |-> owner, source |-> source, semantic |-> semantic,
   deliveryOrdinal |-> deliveryOrdinal,
   connectionTenure |-> connectionTenure,
   retiredDeliveryOrdinal |-> retiredDeliveryOrdinal,
   retiredConnectionTenure |-> retiredConnectionTenure,
   ticketTenure |-> ticketTenure,
   ticketSemantic |-> ticketSemantic,
   ticketTarget |-> ticketTarget,
   ticketMessageCursor |-> ticketMessageCursor,
   ticketChunkCursor |-> ticketChunkCursor,
   messageCursor |-> messageCursor, chunkCursor |-> chunkCursor]

ReplyAttemptSet ==
  [owner: ReplyOwners, source: ReplySources, semantic: ReplySemantics,
   deliveryOrdinal: ReplyDeliveryOrdinals,
   connectionTenure: ReplyConnectionTenures,
   retiredDeliveryOrdinal: 0..ReplyDeliveryOrdinalLimit,
   retiredConnectionTenure: 0..ReplyDeliveryOrdinalLimit,
   ticketTenure: 0..ReplyDeliveryOrdinalLimit,
   ticketSemantic: SUBSET ReplySemantics,
   ticketTarget: SUBSET ReplyTargets,
   ticketMessageCursor: SUBSET (0..ReplyMessageCount),
   ticketChunkCursor: SUBSET (0..ReplyChunkCount),
   messageCursor: 0..ReplyMessageCount,
   chunkCursor: 0..ReplyChunkCount]

VARIABLES
  rrAttempts,
  rrPayloads,
  rrNextDeliveryOrdinal,
  rrConnectionTenure,
  rrSourceActive,
  rrNextServiceIndex,
  rrSemanticSequence,
  rrSemanticHash,
  rrRequesterNextSequence,
  rrRequesterClosedThrough,
  rrClosePendingThrough,
  rrCloseSentThrough,
  rrCloseAcknowledgedThrough,
  rrCloseRetryGeneration

ReplyLifecycleVars ==
  <<rrSemanticSequence, rrSemanticHash, rrRequesterNextSequence,
    rrRequesterClosedThrough>>

ReplyCloseVars ==
  <<rrClosePendingThrough, rrCloseSentThrough,
    rrCloseAcknowledgedThrough, rrCloseRetryGeneration>>

ReplyRouteVars ==
  <<rrAttempts, rrPayloads, rrNextDeliveryOrdinal, rrConnectionTenure,
    rrSourceActive, rrNextServiceIndex, rrSemanticSequence, rrSemanticHash,
    rrRequesterNextSequence, rrRequesterClosedThrough,
    rrClosePendingThrough, rrCloseSentThrough,
    rrCloseAcknowledgedThrough, rrCloseRetryGeneration>>

ReplySemanticBound(owner, semantic) ==
  /\ rrSemanticSequence[owner][semantic] \in ReplySemanticSequences
  /\ rrSemanticHash[owner][semantic] =
       ReplyCanonicalSemanticHash(semantic)

ReplySemanticClosed(owner, semantic) ==
  /\ ReplySemanticBound(owner, semantic)
  /\ rrSemanticSequence[owner][semantic]
       <= rrRequesterClosedThrough[owner]

ReplySemanticActive(owner, semantic) ==
  /\ ReplySemanticBound(owner, semantic)
  /\ rrSemanticSequence[owner][semantic]
       > rrRequesterClosedThrough[owner]

ReplyActiveSemantics(owner) ==
  {semantic \in ReplySemantics:
     ReplySemanticActive(owner, semantic)}

ReplyPayloadsForAttempts(attempts) ==
  {attempt.semantic: attempt \in attempts}

ReplyCloseWorkPending(requester, responder) ==
  rrClosePendingThrough[requester][responder]
    > rrCloseAcknowledgedThrough[requester][responder]

NextReplyCloseRetryGeneration(generation) ==
  IF generation = ReplyDeliveryOrdinalLimit
  THEN 0
  ELSE generation + 1

ReplyCloseWitness(requester, authenticatedRequester, responder,
                  authenticatedResponder, closedThrough,
                  bindingRequester, bindingResponder,
                  bindingClosedThrough) ==
  [requester |-> requester,
   authenticatedRequester |-> authenticatedRequester,
   responder |-> responder,
   authenticatedResponder |-> authenticatedResponder,
   closedThrough |-> closedThrough,
   bindingRequester |-> bindingRequester,
   bindingResponder |-> bindingResponder,
   bindingClosedThrough |-> bindingClosedThrough]

ReplyCloseWitnessSet ==
  [requester: ReplyOwners,
   authenticatedRequester: ReplyOwners,
   responder: ReplySources,
   authenticatedResponder: ReplySources,
   closedThrough: 0..ReplyDeliveryOrdinalLimit,
   bindingRequester: ReplyOwners,
   bindingResponder: ReplySources,
   bindingClosedThrough: 0..ReplyDeliveryOrdinalLimit]

ReplyCloseWitnessValid(witness) ==
  /\ witness \in ReplyCloseWitnessSet
  /\ witness.authenticatedRequester = witness.requester
  /\ witness.authenticatedResponder = witness.responder
  /\ witness.bindingRequester = witness.requester
  /\ witness.bindingResponder = witness.responder
  /\ witness.bindingClosedThrough = witness.closedThrough

ReplyCanonicalCloseWitness(requester, responder, closedThrough) ==
  ReplyCloseWitness(
    requester, requester, responder, responder, closedThrough,
    requester, responder, closedThrough)

ReplyCloseAcknowledgement(requester, responder, authenticatedResponder,
                          closedThrough,
                          bindingRequester, bindingResponder,
                          bindingClosedThrough) ==
  [requester |-> requester,
   responder |-> responder,
   authenticatedResponder |-> authenticatedResponder,
   closedThrough |-> closedThrough,
   bindingRequester |-> bindingRequester,
   bindingResponder |-> bindingResponder,
   bindingClosedThrough |-> bindingClosedThrough]

ReplyCloseAcknowledgementSet ==
  [requester: ReplyOwners,
   responder: ReplySources,
   authenticatedResponder: ReplySources,
   closedThrough: 0..ReplyDeliveryOrdinalLimit,
   bindingRequester: ReplyOwners,
   bindingResponder: ReplySources,
   bindingClosedThrough: 0..ReplyDeliveryOrdinalLimit]

ReplyCloseAcknowledgementValid(acknowledgement) ==
  /\ acknowledgement \in ReplyCloseAcknowledgementSet
  /\ acknowledgement.authenticatedResponder =
       acknowledgement.responder
  /\ acknowledgement.bindingRequester = acknowledgement.requester
  /\ acknowledgement.bindingResponder = acknowledgement.responder
  /\ acknowledgement.bindingClosedThrough =
       acknowledgement.closedThrough

ReplyCanonicalCloseAcknowledgement(requester, responder, closedThrough) ==
  ReplyCloseAcknowledgement(
    requester, responder, responder, closedThrough,
    requester, responder, closedThrough)

ReplyAttemptsFor(owner, semantic) ==
  {attempt \in rrAttempts:
     /\ attempt.owner = owner
     /\ attempt.semantic = semantic}

ReplyAttemptsForSource(owner, semantic, source) ==
  {attempt \in ReplyAttemptsFor(owner, semantic):
     attempt.source = source}

ReplyAttemptSources(owner, semantic) ==
  {attempt.source: attempt \in ReplyAttemptsFor(owner, semantic)}

ReplyRetiredDeliverySources(owner, semantic) ==
  {attempt.source:
     attempt \in {candidate \in ReplyAttemptsFor(owner, semantic):
                    candidate.retiredDeliveryOrdinal # 0}}

ReplyAttemptOwned(owner, semantic, source) ==
  ReplyAttemptsForSource(owner, semantic, source) # {}

ReplyAttemptFor(owner, semantic, source) ==
  CHOOSE attempt \in ReplyAttemptsForSource(owner, semantic, source): TRUE

ReplyAttemptCursor(attempt) ==
  <<attempt.messageCursor, attempt.chunkCursor>>

ReplyAttemptRank(attempt) ==
  attempt.messageCursor * (ReplyChunkCount + 1) + attempt.chunkCursor

ReplyAttemptComplete(attempt) ==
  /\ attempt.messageCursor = ReplyMessageCount
  /\ attempt.chunkCursor = ReplyChunkCount

ReplyAttemptCurrent(attempt) ==
  /\ rrSourceActive[attempt.owner][attempt.source]
  /\ attempt.connectionTenure =
       rrConnectionTenure[attempt.owner][attempt.source]

ReplyTicketForAttempt(attempt) ==
  [owner |-> attempt.owner, source |-> attempt.source,
   semantic |-> attempt.ticketSemantic,
   target |-> attempt.ticketTarget,
   connectionTenure |-> attempt.ticketTenure,
   messageCursor |-> attempt.ticketMessageCursor,
   chunkCursor |-> attempt.ticketChunkCursor]

ReplyTicketValidForAttempt(attempt) ==
  /\ ReplyAttemptCurrent(attempt)
  /\ attempt.ticketTenure = attempt.connectionTenure
  /\ ReplyTicketForAttempt(attempt) =
       ReplyTicket(
         attempt.owner, attempt.source, attempt.semantic,
         ReplySemanticTarget(attempt.semantic),
         rrConnectionTenure[attempt.owner][attempt.source],
         attempt.messageCursor, attempt.chunkCursor)

ReplyAttemptHasNoTicket(attempt) ==
  /\ attempt.ticketTenure = NoReplyTicketTenure
  /\ attempt.ticketSemantic = {}
  /\ attempt.ticketTarget = {}
  /\ attempt.ticketMessageCursor = {}
  /\ attempt.ticketChunkCursor = {}

ReplyCapabilityIntrinsicBindingValid(capability) ==
  /\ capability.bindingOwner = capability.owner
  /\ capability.bindingSource = capability.source
  /\ capability.bindingTarget = capability.target
  /\ capability.bindingDeliveryOrdinal = capability.deliveryOrdinal
  /\ capability.bindingConnectionTenure = capability.connectionTenure
  /\ capability.bindingSourceCapacity = capability.sourceCapacity

ReplyAttemptHasNoRetiredDelivery(attempt) ==
  /\ attempt.retiredDeliveryOrdinal = 0
  /\ attempt.retiredConnectionTenure = 0

ReplyAttemptRetiredDeliveryWellFormed(attempt) ==
  \/ ReplyAttemptHasNoRetiredDelivery(attempt)
  \/ /\ attempt.retiredDeliveryOrdinal \in ReplyDeliveryOrdinals
     /\ attempt.retiredDeliveryOrdinal < attempt.deliveryOrdinal
     /\ attempt.retiredConnectionTenure \in ReplyConnectionTenures

ReplyCapabilityIdentityMatchesAttempt(capability, attempt,
                                      deliveryOrdinal,
                                      connectionTenure) ==
  /\ capability.owner = attempt.owner
  /\ capability.source = attempt.source
  /\ capability.target = ReplySemanticTarget(attempt.semantic)
  /\ capability.semantic = attempt.semantic
  /\ capability.deliveryOrdinal = deliveryOrdinal
  /\ capability.connectionTenure = connectionTenure

(***************************************************************************
Delivery ordinals are actor-global.  A candidate which reuses a known ordinal
must reproduce the complete source/target/semantic/tenure identity.  The
latest retired delivery retained on every bounded source attempt preserves the
same cross-source collision check after live-route pruning.
***************************************************************************)
ReplyCapabilityHasKnownOrdinalCollision(capability) ==
  \E attempt \in rrAttempts:
    /\ attempt.owner = capability.owner
    /\ \/ /\ attempt.deliveryOrdinal = capability.deliveryOrdinal
           /\ ~ReplyCapabilityIdentityMatchesAttempt(
                capability, attempt, attempt.deliveryOrdinal,
                attempt.connectionTenure)
       \/ /\ attempt.retiredDeliveryOrdinal # 0
           /\ attempt.retiredDeliveryOrdinal = capability.deliveryOrdinal
           /\ ~ReplyCapabilityIdentityMatchesAttempt(
                capability, attempt, attempt.retiredDeliveryOrdinal,
                attempt.retiredConnectionTenure)

ReplyCapabilityValidFor(capability, expectedOwner, expectedSource,
                        expectedSemantic) ==
  /\ ReplyCapabilityIntrinsicBindingValid(capability)
  /\ capability.owner = expectedOwner
  /\ capability.source = expectedSource
  /\ capability.target = ReplySemanticTarget(expectedSemantic)
  /\ capability.semantic = expectedSemantic
  /\ capability.sourceCapacity = ReplySourceCapacity
  /\ expectedOwner \in ReplyOwners
  /\ expectedSource \in ReplySources
  /\ expectedSemantic \in ReplySemantics
  /\ rrSourceActive[expectedOwner][expectedSource]
  /\ capability.connectionTenure =
       rrConnectionTenure[expectedOwner][expectedSource]
  /\ capability.deliveryOrdinal \in ReplyDeliveryOrdinals
  /\ ~ReplyCapabilityHasKnownOrdinalCollision(capability)

ReplyCapabilityRejection(capability, expectedOwner, expectedSource,
                         expectedSemantic) ==
  CASE ~ReplyCapabilityIntrinsicBindingValid(capability) ->
         "EqualOrdinalDifferentTenure"
    [] capability.owner # expectedOwner -> "ForeignOwner"
    [] capability.source # expectedSource -> "DifferentSource"
    [] capability.target # ReplySemanticTarget(expectedSemantic) ->
         "Retargeted"
    [] capability.semantic # expectedSemantic -> "Retargeted"
    [] capability.sourceCapacity # ReplySourceCapacity -> "ForeignOwner"
    [] ~rrSourceActive[expectedOwner][expectedSource] -> "Inactive"
    [] ReplyCapabilityHasKnownOrdinalCollision(capability) ->
         "EqualOrdinalDifferentTenure"
    [] ReplyAttemptOwned(expectedOwner, expectedSemantic, expectedSource)
         /\ capability.deliveryOrdinal
              < ReplyAttemptFor(expectedOwner, expectedSemantic,
                                expectedSource).deliveryOrdinal -> "Stale"
    [] ReplyAttemptOwned(expectedOwner, expectedSemantic, expectedSource)
         /\ capability.deliveryOrdinal
              = ReplyAttemptFor(expectedOwner, expectedSemantic,
                                expectedSource).deliveryOrdinal
         /\ capability.connectionTenure
              # ReplyAttemptFor(expectedOwner, expectedSemantic,
                                expectedSource).connectionTenure ->
           "EqualOrdinalDifferentTenure"
    [] OTHER -> "NotRejected"

ReplaceReplyAttempt(oldAttempt, newAttempt) ==
  (rrAttempts \ {oldAttempt}) \cup {newAttempt}

ReplyAttemptWithRoute(attempt, deliveryOrdinal, connectionTenure) ==
  IF connectionTenure = attempt.connectionTenure
  THEN [attempt EXCEPT
          !.deliveryOrdinal = deliveryOrdinal,
          !.retiredDeliveryOrdinal = attempt.deliveryOrdinal,
          !.retiredConnectionTenure = attempt.connectionTenure]
  ELSE [attempt EXCEPT
          !.deliveryOrdinal = deliveryOrdinal,
          !.connectionTenure = connectionTenure,
          !.retiredDeliveryOrdinal = attempt.deliveryOrdinal,
          !.retiredConnectionTenure = attempt.connectionTenure,
          !.ticketTenure = NoReplyTicketTenure,
          !.ticketSemantic = {},
          !.ticketTarget = {},
          !.ticketMessageCursor = {},
          !.ticketChunkCursor = {}]

ReplyAttemptWithoutTicket(attempt) ==
  [attempt EXCEPT
     !.ticketTenure = NoReplyTicketTenure,
     !.ticketSemantic = {},
     !.ticketTarget = {},
     !.ticketMessageCursor = {},
     !.ticketChunkCursor = {}]

ReplySourceHasNoTickets(owner, source) ==
  \A attempt \in rrAttempts:
    (attempt.owner = owner /\ attempt.source = source) =>
      ReplyAttemptHasNoTicket(attempt)

(***************************************************************************
A connection-tenure change invalidates every admission ticket minted by that
source tenure.  Only the selected semantic attempt receives the new route;
all sibling semantic attempts retain their route and cursors while losing
their now-stale tickets.
***************************************************************************)
ReplyAttemptsAfterReconnect(oldAttempt, routedAttempt) ==
  {IF attempt = oldAttempt
   THEN routedAttempt
   ELSE IF attempt.owner = oldAttempt.owner
             /\ attempt.source = oldAttempt.source
        THEN ReplyAttemptWithoutTicket(attempt)
        ELSE attempt: attempt \in rrAttempts}

ReplyAttemptsAfterClose(requester, closedThrough) ==
  {attempt \in rrAttempts:
     \/ attempt.owner # requester
     \/ rrSemanticSequence[requester][attempt.semantic] > closedThrough}

ReplyAttemptWithTicket(attempt) ==
  [attempt EXCEPT
     !.ticketTenure = attempt.connectionTenure,
     !.ticketSemantic = {attempt.semantic},
     !.ticketTarget = {ReplySemanticTarget(attempt.semantic)},
     !.ticketMessageCursor = {attempt.messageCursor},
     !.ticketChunkCursor = {attempt.chunkCursor}]

ReplyAttemptAfterService(attempt) ==
  IF attempt.messageCursor < ReplyMessageCount
  THEN [attempt EXCEPT
          !.messageCursor = @ + 1,
          !.ticketTenure = NoReplyTicketTenure,
          !.ticketSemantic = {},
          !.ticketTarget = {},
          !.ticketMessageCursor = {},
          !.ticketChunkCursor = {}]
  ELSE [attempt EXCEPT
          !.chunkCursor = @ + 1,
          !.ticketTenure = NoReplyTicketTenure,
          !.ticketSemantic = {},
          !.ticketTarget = {},
          !.ticketMessageCursor = {},
          !.ticketChunkCursor = {}]

(***************************************************************************
Shared exact-attempt terminal kernel.  The bounded pipeline supplies its own
source/class FIFO and exact ticket authority before invoking this action; the
kernel advances only the named live attempt and clears any route ticket.
***************************************************************************)
ReplyAttemptServiceKernelValid(oldAttempt, serviced) ==
  /\ serviced \in ReplyAttemptSet
  /\ serviced.owner = oldAttempt.owner
  /\ serviced.source = oldAttempt.source
  /\ serviced.semantic = oldAttempt.semantic
  /\ serviced.deliveryOrdinal = oldAttempt.deliveryOrdinal
  /\ serviced.connectionTenure = oldAttempt.connectionTenure
  /\ serviced.retiredDeliveryOrdinal =
       oldAttempt.retiredDeliveryOrdinal
  /\ serviced.retiredConnectionTenure =
       oldAttempt.retiredConnectionTenure
  /\ ReplyAttemptRank(serviced) > ReplyAttemptRank(oldAttempt)
  /\ ReplyAttemptHasNoTicket(serviced)

AdvanceCurrentReplyAttempt(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      serviced == ReplyAttemptAfterService(oldAttempt)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(oldAttempt)
     /\ ~ReplyAttemptComplete(oldAttempt)
     /\ ReplyAttemptServiceKernelValid(oldAttempt, serviced)
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, serviced)
     /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                    rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex, rrSemanticSequence,
                    rrSemanticHash, rrRequesterNextSequence,
                    rrRequesterClosedThrough, rrClosePendingThrough,
                    rrCloseSentThrough, rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

NextReplySourceIndex(index) ==
  IF index = Len(ReplySourceOrder) THEN 1 ELSE index + 1

ReplySourceCyclicDistance(startIndex, candidateIndex) ==
  IF candidateIndex >= startIndex
  THEN candidateIndex - startIndex
  ELSE Len(ReplySourceOrder) - startIndex + candidateIndex

ReplySourceIndices(source) ==
  {index \in 1..Len(ReplySourceOrder):
     ReplySourceOrder[index] = source}

ReplySourceIndex(source) ==
  CHOOSE index \in ReplySourceIndices(source): TRUE

ReplySourceRoundRobinRank(owner, semantic, source) ==
  ReplySourceCyclicDistance(
    rrNextServiceIndex[owner][semantic], ReplySourceIndex(source))

ReplyPendingSourceIndices(owner, semantic) ==
  {index \in 1..Len(ReplySourceOrder):
     LET source == ReplySourceOrder[index]
     IN /\ ReplyAttemptOwned(owner, semantic, source)
        /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
           IN /\ ReplyTicketValidForAttempt(attempt)
              /\ ~ReplyAttemptComplete(attempt)}

ReplySelectedSourceIndex(owner, semantic) ==
  LET pending == ReplyPendingSourceIndices(owner, semantic)
      start == rrNextServiceIndex[owner][semantic]
  IN CHOOSE index \in pending:
       \A other \in pending:
         ReplySourceCyclicDistance(start, index)
           <= ReplySourceCyclicDistance(start, other)

ReplySelectedSource(owner, semantic) ==
  ReplySourceOrder[ReplySelectedSourceIndex(owner, semantic)]

ReplyRouteInit ==
  /\ ReplyRouteConfiguration
  /\ rrAttempts = {}
  /\ rrPayloads = {}
  /\ rrNextDeliveryOrdinal = [owner \in ReplyOwners |-> 1]
  /\ rrConnectionTenure =
       [owner \in ReplyOwners |-> [source \in ReplySources |-> 1]]
  /\ rrSourceActive =
       [owner \in ReplyOwners |-> [source \in ReplySources |-> TRUE]]
  /\ rrNextServiceIndex =
       [owner \in ReplyOwners |-> [semantic \in ReplySemantics |-> 1]]
  /\ rrSemanticSequence =
       [owner \in ReplyOwners |->
          [semantic \in ReplySemantics |-> 0]]
  /\ rrSemanticHash =
       [owner \in ReplyOwners |->
          [semantic \in ReplySemantics |-> {}]]
  /\ rrRequesterNextSequence = [owner \in ReplyOwners |-> 1]
  /\ rrRequesterClosedThrough = [owner \in ReplyOwners |-> 0]
  /\ rrClosePendingThrough =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> 0]]
  /\ rrCloseSentThrough =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> 0]]
  /\ rrCloseAcknowledgedThrough =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> 0]]
  /\ rrCloseRetryGeneration =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> 0]]

(***************************************************************************
Only a new authenticated source allocates a cursor-zero attempt.  Capacity is
the exact configured source geometry, not an eviction policy.
***************************************************************************)
ObserveNewReplySource(owner, semantic, source) ==
  LET deliveryOrdinal == rrNextDeliveryOrdinal[owner]
      connectionTenure == rrConnectionTenure[owner][source]
      semanticWasBound == ReplySemanticBound(owner, semantic)
      semanticSequence ==
        IF semanticWasBound
        THEN rrSemanticSequence[owner][semantic]
        ELSE rrRequesterNextSequence[owner]
      capability ==
        ReplyCapability(
          owner, source, ReplySemanticTarget(semantic), semantic,
          deliveryOrdinal, connectionTenure)
      attempt ==
        ReplyAttempt(owner, source, semantic, deliveryOrdinal,
                     connectionTenure, 0, 0, NoReplyTicketTenure,
                     {}, {}, {}, {}, 0, 0)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ~ReplyAttemptOwned(owner, semantic, source)
     /\ Cardinality(ReplyAttemptSources(owner, semantic))
          < ReplySourceCapacity
     /\ semanticSequence \in ReplySemanticSequences
     /\ semanticSequence > rrRequesterClosedThrough[owner]
     /\ semanticSequence
          <= rrRequesterClosedThrough[owner]
               + ReplyActiveWindowCapacity
     /\ IF semanticWasBound
        THEN ReplySemanticActive(owner, semantic)
        ELSE /\ rrSemanticSequence[owner][semantic] = 0
             /\ rrSemanticHash[owner][semantic] = {}
             /\ Cardinality(ReplyActiveSemantics(owner))
                  < ReplyActiveWindowCapacity
     /\ deliveryOrdinal \in ReplyDeliveryOrdinals
     /\ ReplyCapabilityValidFor(
          capability, owner, source, semantic)
     /\ rrAttempts' = rrAttempts \cup {attempt}
     /\ rrPayloads' = rrPayloads \cup {semantic}
     /\ rrNextDeliveryOrdinal' =
          [rrNextDeliveryOrdinal EXCEPT ![owner] = @ + 1]
     /\ rrSemanticSequence' =
          IF semanticWasBound
          THEN rrSemanticSequence
          ELSE [rrSemanticSequence EXCEPT
                  ![owner][semantic] = semanticSequence]
     /\ rrSemanticHash' =
          IF semanticWasBound
          THEN rrSemanticHash
          ELSE [rrSemanticHash EXCEPT
                  ![owner][semantic] =
                    ReplyCanonicalSemanticHash(semantic)]
     /\ rrRequesterNextSequence' =
          IF semanticWasBound
          THEN rrRequesterNextSequence
          ELSE [rrRequesterNextSequence EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex, rrRequesterClosedThrough,
                    rrClosePendingThrough, rrCloseSentThrough,
                    rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

(***************************************************************************
A later delivery from one source changes only that attempt.  A same-tenure
delivery retains message/chunk rank and its ticket.  If another semantic
attempt already advanced the source-scoped connection tenure, this delivery
rebinds only the selected attempt and clears its stale ticket.  Neither case
changes the source-owned cursor.
***************************************************************************)
ObserveLaterReplyDelivery(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      deliveryOrdinal == rrNextDeliveryOrdinal[owner]
      connectionTenure == rrConnectionTenure[owner][source]
      capability ==
        ReplyCapability(
          owner, source, ReplySemanticTarget(semantic), semantic,
          deliveryOrdinal, connectionTenure)
      routed ==
        ReplyAttemptWithRoute(oldAttempt, deliveryOrdinal,
                              connectionTenure)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ rrSourceActive[owner][source]
     /\ oldAttempt.connectionTenure <= connectionTenure
     /\ deliveryOrdinal \in ReplyDeliveryOrdinals
     /\ deliveryOrdinal > oldAttempt.deliveryOrdinal
     /\ ReplyCapabilityValidFor(
          capability, owner, source, semantic)
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, routed)
     /\ rrNextDeliveryOrdinal' =
          [rrNextDeliveryOrdinal EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<rrPayloads, rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex, rrSemanticSequence,
                    rrSemanticHash, rrRequesterNextSequence,
                    rrRequesterClosedThrough, rrClosePendingThrough,
                    rrCloseSentThrough, rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

(***************************************************************************
An exact authenticated duplicate observes the already-owned attempt without
changing its route, ticket, service rank, or either cursor.
***************************************************************************)
RetryExactReplySource(owner, semantic, source) ==
  LET attempt == ReplyAttemptFor(owner, semantic, source)
      capability ==
        ReplyCapability(owner, source, ReplySemanticTarget(semantic), semantic,
                        attempt.deliveryOrdinal,
                        attempt.connectionTenure)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyCapabilityValidFor(
          capability, owner, source, semantic)
     /\ UNCHANGED ReplyRouteVars

(***************************************************************************
Connection teardown invalidates tenure-bound tickets but does not erase
semantic ownership or either progress cursor.
***************************************************************************)
RetireReplySource(owner, source) ==
  /\ owner \in ReplyOwners
  /\ source \in ReplySources
  /\ rrSourceActive[owner][source]
  /\ rrAttempts' =
       {IF attempt.owner = owner /\ attempt.source = source
        THEN ReplyAttemptWithoutTicket(attempt)
        ELSE attempt: attempt \in rrAttempts}
  /\ ReplySourceHasNoTickets(owner, source)'
  /\ rrSourceActive' =
       [rrSourceActive EXCEPT ![owner][source] = FALSE]
  /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                 rrConnectionTenure, rrNextServiceIndex,
                 rrSemanticSequence, rrSemanticHash,
                 rrRequesterNextSequence, rrRequesterClosedThrough,
                 rrClosePendingThrough, rrCloseSentThrough,
                 rrCloseAcknowledgedThrough,
                 rrCloseRetryGeneration>>

(***************************************************************************
Reconnect is a new delivery under a new connection tenure.  The new writer
has no flush continuity, so the selected attempt retries its retained current
message/chunk and must acquire a fresh admission ticket.  Every sibling
semantic attempt for the same authenticated source retains its route and
cursors but atomically loses any ticket minted by the retired tenure.

The production refinement linearizes a successful old-writer flush before
this reconnect action even when its receipt is observed later.  That mapping
is valid only while the reconnect retry is still queued and has not been
actor-admitted; it never authorizes service through an inactive capability.
***************************************************************************)
ReconnectReplySource(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      deliveryOrdinal == rrNextDeliveryOrdinal[owner]
      connectionTenure == rrConnectionTenure[owner][source] + 1
      capability ==
        ReplyCapability(
          owner, source, ReplySemanticTarget(semantic), semantic,
          deliveryOrdinal, connectionTenure)
      routed ==
        ReplyAttemptWithRoute(oldAttempt, deliveryOrdinal,
                              connectionTenure)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ~rrSourceActive[owner][source]
     /\ oldAttempt.connectionTenure < connectionTenure
     /\ connectionTenure \in ReplyConnectionTenures
     /\ deliveryOrdinal \in ReplyDeliveryOrdinals
     /\ deliveryOrdinal > oldAttempt.deliveryOrdinal
     /\ capability.owner = owner
     /\ capability.source = source
     /\ capability.target = ReplySemanticTarget(semantic)
     /\ capability.semantic = semantic
     /\ rrAttempts' = ReplyAttemptsAfterReconnect(oldAttempt, routed)
     /\ ReplySourceHasNoTickets(owner, source)'
     /\ rrConnectionTenure' =
          [rrConnectionTenure EXCEPT
             ![owner][source] = connectionTenure]
     /\ rrSourceActive' =
          [rrSourceActive EXCEPT ![owner][source] = TRUE]
     /\ rrNextDeliveryOrdinal' =
          [rrNextDeliveryOrdinal EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<rrPayloads, rrNextServiceIndex,
                    rrSemanticSequence, rrSemanticHash,
                    rrRequesterNextSequence, rrRequesterClosedThrough,
                    rrClosePendingThrough, rrCloseSentThrough,
                    rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

AcquireReplyTicket(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      ticketed == ReplyAttemptWithTicket(oldAttempt)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(oldAttempt)
     /\ ReplyAttemptHasNoTicket(oldAttempt)
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, ticketed)
     /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                    rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex, rrSemanticSequence,
                    rrSemanticHash, rrRequesterNextSequence,
                    rrRequesterClosedThrough, rrClosePendingThrough,
                    rrCloseSentThrough, rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

(***************************************************************************
Terminal exact-item linearization.  For ordinary reliable output this action
maps to actor FIFO admission.  For certified sidecar output, actor admission
and writer work are hidden stutters and this action maps to the exact writer
flush receipt.  The abstract ticket is therefore a ghost reservation witness
until this terminal event; it cannot authorize another payload or an inactive
route.  This avoids treating mere sidecar actor admission as chunk progress.
***************************************************************************)
ServiceReplyRoute(owner, semantic) ==
  LET selectedIndex == ReplySelectedSourceIndex(owner, semantic)
      source == ReplySourceOrder[selectedIndex]
      oldAttempt == ReplyAttemptFor(owner, semantic, source)
      serviced == ReplyAttemptAfterService(oldAttempt)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ ReplyPendingSourceIndices(owner, semantic) # {}
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, serviced)
     /\ rrNextServiceIndex' =
          [rrNextServiceIndex EXCEPT
             ![owner][semantic] = NextReplySourceIndex(selectedIndex)]
     /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                    rrConnectionTenure, rrSourceActive,
                    rrSemanticSequence, rrSemanticHash,
                    rrRequesterNextSequence, rrRequesterClosedThrough,
                    rrClosePendingThrough, rrCloseSentThrough,
                    rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

(***************************************************************************
Only an authenticated requester may advance its cumulative close floor.  The
witness is bound to the exact requester and exact cumulative sequence, and it
may cover only sequences which that requester has already issued.  All
source attempts for covered semantic requests leave together; attempts owned
by another requester survive even when they share the same semantic payload.
The durable sequence/hash journal is retained so delayed delivery cannot bind
the closed semantic to a new sequence.
***************************************************************************)
CloseSemanticRequest(witness) ==
  LET requester == witness.requester
      responder == witness.responder
      closedThrough == witness.closedThrough
      remainingAttempts ==
        ReplyAttemptsAfterClose(requester, closedThrough)
  IN /\ ReplyCloseWitnessValid(witness)
     /\ ~ReplyCloseWorkPending(requester, responder)
     /\ closedThrough > rrRequesterClosedThrough[requester]
     /\ closedThrough < rrRequesterNextSequence[requester]
     /\ rrAttempts' = remainingAttempts
     /\ rrPayloads' = ReplyPayloadsForAttempts(remainingAttempts)
     /\ rrRequesterClosedThrough' =
          [rrRequesterClosedThrough EXCEPT
             ![requester] = closedThrough]
     /\ rrClosePendingThrough' =
          [rrClosePendingThrough EXCEPT
             ![requester][responder] = closedThrough]
     /\ rrCloseSentThrough' =
          [rrCloseSentThrough EXCEPT
             ![requester][responder] = closedThrough]
     /\ UNCHANGED <<rrNextDeliveryOrdinal, rrConnectionTenure,
                    rrSourceActive, rrNextServiceIndex,
                    rrSemanticSequence, rrSemanticHash,
                    rrRequesterNextSequence,
                    rrCloseAcknowledgedThrough,
                    rrCloseRetryGeneration>>

(***************************************************************************
A close carried on a newly authenticated request has the same durable
linearization as a standalone close.  The carrying request is subsequently
observed through ObserveNewReplySource or ObserveLaterReplyDelivery, so the
close cannot inherit or manufacture a route capability.
***************************************************************************)
PiggybackCloseSemanticRequest(witness) ==
  CloseSemanticRequest(witness)

(***************************************************************************
The requester retains the latest cumulative close until the exact responder
acknowledges the exact floor.  Retry generation is bounded bookkeeping only;
it is neither a delivery ordinal nor a reply capability.  Replaying the
latest witness after acknowledgement is a semantic stutter.
***************************************************************************)
RetryCloseSemanticRequest(witness) ==
  LET requester == witness.requester
      responder == witness.responder
      closedThrough == witness.closedThrough
  IN /\ ReplyCloseWitnessValid(witness)
     /\ closedThrough =
          rrClosePendingThrough[requester][responder]
     /\ closedThrough =
          rrCloseSentThrough[requester][responder]
     /\ closedThrough <= rrRequesterClosedThrough[requester]
     /\ IF ReplyCloseWorkPending(requester, responder)
        THEN /\ rrCloseRetryGeneration' =
                  [rrCloseRetryGeneration EXCEPT
                     ![requester][responder] =
                       NextReplyCloseRetryGeneration(@)]
             /\ UNCHANGED <<rrAttempts, rrPayloads,
                            rrNextDeliveryOrdinal,
                            rrConnectionTenure, rrSourceActive,
                            rrNextServiceIndex, rrSemanticSequence,
                            rrSemanticHash, rrRequesterNextSequence,
                            rrRequesterClosedThrough,
                            rrClosePendingThrough, rrCloseSentThrough,
                            rrCloseAcknowledgedThrough>>
        ELSE /\ rrCloseAcknowledgedThrough[requester][responder]
                  >= closedThrough
             /\ UNCHANGED ReplyRouteVars

(***************************************************************************
The acknowledgement carries only the authenticated close identity.  It has
no delivery ordinal, connection tenure, target, ticket, or route capability,
so it cannot be replayed as permission to emit output.
***************************************************************************)
AcknowledgeCloseSemanticRequest(acknowledgement) ==
  LET requester == acknowledgement.requester
      responder == acknowledgement.responder
      closedThrough == acknowledgement.closedThrough
  IN /\ ReplyCloseAcknowledgementValid(acknowledgement)
     /\ closedThrough # 0
     /\ closedThrough =
          rrClosePendingThrough[requester][responder]
     /\ closedThrough =
          rrCloseSentThrough[requester][responder]
     /\ closedThrough <= rrRequesterClosedThrough[requester]
     /\ rrCloseAcknowledgedThrough[requester][responder]
          <= closedThrough
     /\ rrCloseAcknowledgedThrough' =
          [rrCloseAcknowledgedThrough EXCEPT
             ![requester][responder] = closedThrough]
     /\ UNCHANGED <<rrAttempts, rrPayloads, rrNextDeliveryOrdinal,
                    rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex, rrSemanticSequence,
                    rrSemanticHash, rrRequesterNextSequence,
                    rrRequesterClosedThrough, rrClosePendingThrough,
                    rrCloseSentThrough, rrCloseRetryGeneration>>

(***************************************************************************
Recovery invalidates one authenticated source's process-local admission
tickets and live capability, but retains each semantic binding and every
per-source cursor.  A full actor rehydration is the finite composition of
these source-scoped recovery steps; sources with no retained attempt remain
available for a first authenticated delivery.
***************************************************************************)
RecoverReplyRouteState(owner, source) ==
  RetireReplySource(owner, source)

ReplyRouteNext ==
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveNewReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveLaterReplyDelivery(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       RetryExactReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, source \in ReplySources:
       RetireReplySource(owner, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ReconnectReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       AcquireReplyTicket(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics:
       ServiceReplyRoute(owner, semantic)
  \/ \E witness \in ReplyCloseWitnessSet:
       CloseSemanticRequest(witness)
  \/ \E witness \in ReplyCloseWitnessSet:
       PiggybackCloseSemanticRequest(witness)
  \/ \E witness \in ReplyCloseWitnessSet:
       RetryCloseSemanticRequest(witness)
  \/ \E acknowledgement \in ReplyCloseAcknowledgementSet:
       AcknowledgeCloseSemanticRequest(acknowledgement)
  \/ \E owner \in ReplyOwners, source \in ReplySources:
       RecoverReplyRouteState(owner, source)

ReplyRouteFairness ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       WF_ReplyRouteVars(AcquireReplyTicket(owner, semantic, source))
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics:
       WF_ReplyRouteVars(ServiceReplyRoute(owner, semantic))
  /\ \A witness \in ReplyCloseWitnessSet:
       WF_ReplyRouteVars(RetryCloseSemanticRequest(witness))
  /\ \A acknowledgement \in ReplyCloseAcknowledgementSet:
       WF_ReplyRouteVars(
         AcknowledgeCloseSemanticRequest(acknowledgement))

ReplyRouteSpec ==
  ReplyRouteInit
    /\ [][ReplyRouteNext]_ReplyRouteVars
    /\ ReplyRouteFairness

ReplyRouteTypeInvariant ==
  /\ rrAttempts \subseteq ReplyAttemptSet
  /\ rrPayloads \subseteq ReplySemantics
  /\ rrNextDeliveryOrdinal
       \in [ReplyOwners -> 1..(ReplyDeliveryOrdinalLimit + 1)]
  /\ rrConnectionTenure
       \in [ReplyOwners -> [ReplySources -> ReplyConnectionTenures]]
  /\ rrSourceActive \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
  /\ rrNextServiceIndex
       \in [ReplyOwners -> [ReplySemantics -> 1..Len(ReplySourceOrder)]]

ReplyLifecycleTypeInvariant ==
  /\ rrSemanticSequence
       \in [ReplyOwners ->
             [ReplySemantics -> 0..ReplyDeliveryOrdinalLimit]]
  /\ rrSemanticHash
       \in [ReplyOwners ->
             [ReplySemantics -> SUBSET ReplySemantics]]
  /\ rrRequesterNextSequence
       \in [ReplyOwners -> 1..(ReplyDeliveryOrdinalLimit + 1)]
  /\ rrRequesterClosedThrough
       \in [ReplyOwners -> 0..ReplyDeliveryOrdinalLimit]
  /\ rrClosePendingThrough
       \in [ReplyOwners ->
             [ReplySources -> 0..ReplyDeliveryOrdinalLimit]]
  /\ rrCloseSentThrough
       \in [ReplyOwners ->
             [ReplySources -> 0..ReplyDeliveryOrdinalLimit]]
  /\ rrCloseAcknowledgedThrough
       \in [ReplyOwners ->
             [ReplySources -> 0..ReplyDeliveryOrdinalLimit]]
  /\ rrCloseRetryGeneration
       \in [ReplyOwners ->
             [ReplySources -> 0..ReplyDeliveryOrdinalLimit]]

ReplyRouteOwnershipInvariant ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       /\ IsFiniteSet(
            ReplyAttemptsForSource(owner, semantic, source))
       /\ Cardinality(
            ReplyAttemptsForSource(owner, semantic, source)) <= 1
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics:
       /\ Cardinality(ReplyAttemptSources(owner, semantic))
            <= ReplySourceCapacity
       /\ Cardinality(ReplyRetiredDeliverySources(owner, semantic))
            <= ReplySourceCapacity
       /\ (ReplyAttemptsFor(owner, semantic) # {} =>
             semantic \in rrPayloads)
  /\ \A attempt \in rrAttempts:
       /\ attempt.deliveryOrdinal
            < rrNextDeliveryOrdinal[attempt.owner]
       /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
       /\ IF attempt.ticketTenure = NoReplyTicketTenure
          THEN ReplyAttemptHasNoTicket(attempt)
          ELSE ReplyTicketValidForAttempt(attempt)

ReplyLifecycleOwnershipInvariant ==
  /\ \A owner \in ReplyOwners:
       /\ rrRequesterClosedThrough[owner]
            < rrRequesterNextSequence[owner]
       /\ \A semantic \in ReplySemantics:
            /\ (rrSemanticSequence[owner][semantic] = 0)
                 <=> (rrSemanticHash[owner][semantic] = {})
            /\ rrSemanticSequence[owner][semantic] # 0 =>
                 /\ rrSemanticHash[owner][semantic] =
                      ReplyCanonicalSemanticHash(semantic)
                 /\ rrSemanticSequence[owner][semantic]
                      < rrRequesterNextSequence[owner]
            /\ ReplySemanticActive(owner, semantic) =>
                 rrSemanticSequence[owner][semantic]
                   <= rrRequesterClosedThrough[owner]
                        + ReplyActiveWindowCapacity
       /\ \A left, right \in ReplySemantics:
            /\ rrSemanticSequence[owner][left] # 0
            /\ rrSemanticSequence[owner][left] =
                 rrSemanticSequence[owner][right]
            => left = right
  /\ \A attempt \in rrAttempts:
       ReplySemanticActive(attempt.owner, attempt.semantic)
  /\ rrPayloads = ReplyPayloadsForAttempts(rrAttempts)
  /\ \A owner \in ReplyOwners, responder \in ReplySources:
       /\ rrCloseSentThrough[owner][responder] =
            rrClosePendingThrough[owner][responder]
       /\ rrCloseAcknowledgedThrough[owner][responder]
            <= rrClosePendingThrough[owner][responder]
       /\ rrClosePendingThrough[owner][responder]
            <= rrRequesterClosedThrough[owner]

(***************************************************************************
This transition predicate is the tenure-aware retry contract consumed by both
the asynchronous composition and the mutation matrix.  Matching is semantic
and source-scoped.  A same-tenure update cannot regress its delivery ordinal or
either cursor.  A reconnect must advance both tenure and delivery ordinal,
discard its old admission ticket, and preserve its source-owned cursor.
***************************************************************************)
ReplySourceTenureInvalidationStep ==
  \A owner \in ReplyOwners, source \in ReplySources:
    rrConnectionTenure'[owner][source]
      > rrConnectionTenure[owner][source] =>
        ReplySourceHasNoTickets(owner, source)'

ReplyAttemptCoveredByCloseStep(attempt) ==
  /\ ReplySemanticBound(attempt.owner, attempt.semantic)
  /\ rrSemanticSequence'[attempt.owner][attempt.semantic] =
       rrSemanticSequence[attempt.owner][attempt.semantic]
  /\ rrSemanticHash'[attempt.owner][attempt.semantic] =
       rrSemanticHash[attempt.owner][attempt.semantic]
  /\ rrRequesterClosedThrough'[attempt.owner]
       > rrRequesterClosedThrough[attempt.owner]
  /\ rrSemanticSequence[attempt.owner][attempt.semantic]
       > rrRequesterClosedThrough[attempt.owner]
  /\ rrSemanticSequence[attempt.owner][attempt.semantic]
       <= rrRequesterClosedThrough'[attempt.owner]

ReplyAttemptReplayValid(oldAttempt, newAttempt) ==
  /\ newAttempt.owner = oldAttempt.owner
  /\ newAttempt.semantic = oldAttempt.semantic
  /\ newAttempt.source = oldAttempt.source
  /\ IF newAttempt.connectionTenure = oldAttempt.connectionTenure
     THEN /\ newAttempt.deliveryOrdinal
                  >= oldAttempt.deliveryOrdinal
          /\ newAttempt.messageCursor >= oldAttempt.messageCursor
          /\ newAttempt.chunkCursor >= oldAttempt.chunkCursor
     ELSE /\ newAttempt.connectionTenure
                  > oldAttempt.connectionTenure
          /\ newAttempt.deliveryOrdinal
                  > oldAttempt.deliveryOrdinal
          /\ ReplyAttemptHasNoTicket(newAttempt)
          /\ ReplyAttemptCursor(newAttempt) =
               ReplyAttemptCursor(oldAttempt)

ReplyAttemptReplayStep ==
  \A oldAttempt \in rrAttempts:
    \/ ReplyAttemptCoveredByCloseStep(oldAttempt)
    \/ \E newAttempt \in rrAttempts':
         ReplyAttemptReplayValid(oldAttempt, newAttempt)

ReplyLifecycleJournalStep ==
  /\ \A owner \in ReplyOwners:
       rrRequesterClosedThrough'[owner]
         >= rrRequesterClosedThrough[owner]
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics:
       ReplySemanticBound(owner, semantic) =>
         /\ rrSemanticSequence'[owner][semantic] =
              rrSemanticSequence[owner][semantic]
         /\ rrSemanticHash'[owner][semantic] =
              rrSemanticHash[owner][semantic]

ReplyTenureAwareReplayStep ==
  /\ ReplyAttemptReplayStep
  /\ ReplySourceTenureInvalidationStep

ReplyTenureAwareReplay ==
  [][ReplyTenureAwareReplayStep]_ReplyRouteVars

ReplyLifecycleJournal ==
  [][ReplyLifecycleJournalStep]_ReplyRouteVars

(***************************************************************************
Every owned attempt survives a route transition unless the exact durable
sequence for that attempt is covered by an authenticated cumulative close.
When one retained attempt changes, all other attempts owned by the same actor
retain their independent cursors, including other semantic requests from the
same authenticated source.
***************************************************************************)
SameReplyAttemptIdentity(left, right) ==
  /\ left.owner = right.owner
  /\ left.semantic = right.semantic
  /\ left.source = right.source

ReplyRecoveryCursorPreservationStep ==
  \A before \in rrAttempts:
    \E after \in rrAttempts':
      /\ SameReplyAttemptIdentity(before, after)
      /\ ReplyAttemptCursor(after) = ReplyAttemptCursor(before)

ReplyAttemptSurvivalStep ==
  \A retainedBefore \in rrAttempts:
    \/ ReplyAttemptCoveredByCloseStep(retainedBefore)
    \/ \E retainedAfter \in rrAttempts':
         SameReplyAttemptIdentity(retainedBefore, retainedAfter)

ReplyOtherCursorIsolationStep ==
  \A changedBefore \in rrAttempts:
    \A changedAfter \in rrAttempts':
      LET sameAttempt ==
            SameReplyAttemptIdentity(changedBefore, changedAfter)
          attemptChanged ==
            ReplyAttemptCursor(changedAfter) #
              ReplyAttemptCursor(changedBefore)
      IN (sameAttempt /\ attemptChanged) =>
           \A otherBefore \in rrAttempts:
             (otherBefore.owner = changedBefore.owner
               /\ ~SameReplyAttemptIdentity(
                    otherBefore, changedBefore))
             => \E otherAfter \in rrAttempts':
                  /\ SameReplyAttemptIdentity(
                       otherBefore, otherAfter)
                  /\ ReplyAttemptCursor(otherAfter) =
                       ReplyAttemptCursor(otherBefore)

ReplySourceIsolationStep ==
  /\ ReplyAttemptSurvivalStep
  /\ ReplyOtherCursorIsolationStep

ReplySourceIsolation ==
  [][ReplySourceIsolationStep]_ReplyRouteVars

ReplyRouteSafetyInvariant ==
  /\ ReplyRouteTypeInvariant
  /\ ReplyRouteOwnershipInvariant

ReplyRouteLifecycleInvariant ==
  /\ ReplyLifecycleTypeInvariant
  /\ ReplyLifecycleOwnershipInvariant

ReplyRouteFullSafetyInvariant ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplyRouteLifecycleInvariant

(***************************************************************************
This is an explicit liveness obligation, not a safety shorthand.  Stability
requires only that the source-owned attempt eventually remains active under a
single current connection tenure; it deliberately does not pre-assume an
admission ticket.  Weak fairness must first acquire that ticket and then serve
the finite round-robin distance.  The conclusion requires a strictly larger
cursor rank or completion; merely retiring the route, reconnecting it, or
invalidating its ticket cannot satisfy the leads-to target.  An authenticated
cumulative close is the sole alternative target because it explicitly
supersedes the exact semantic sequence rather than silently losing ownership.
***************************************************************************)
ReplySourceServiceEligible(owner, semantic, source) ==
  /\ ReplyAttemptOwned(owner, semantic, source)
  /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
     IN ReplyTicketValidForAttempt(attempt)

ReplySourceRouteStable(owner, semantic, source) ==
  /\ ReplyAttemptOwned(owner, semantic, source)
  /\ ReplyAttemptCurrent(ReplyAttemptFor(owner, semantic, source))

ReplySourceStableResponsive(owner, semantic, source) ==
  <>[] ReplySourceRouteStable(owner, semantic, source)

ReplySourceAtCursor(owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplyAttemptOwned(owner, semantic, source)
  /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
     IN /\ ~ReplyAttemptComplete(attempt)
        /\ attempt.messageCursor = messageCursor
        /\ attempt.chunkCursor = chunkCursor

ReplySourceAdvancedFrom(owner, semantic, source,
                        messageCursor, chunkCursor) ==
  \/ ReplySemanticClosed(owner, semantic)
  \/ /\ ReplyAttemptOwned(owner, semantic, source)
     /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
            oldRank ==
              messageCursor * (ReplyChunkCount + 1) + chunkCursor
        IN \/ ReplyAttemptComplete(attempt)
           \/ ReplyAttemptRank(attempt) > oldRank

ReplySourceEventuallyProgresses(owner, semantic, source) ==
  ReplySourceStableResponsive(owner, semantic, source) =>
    \A messageCursor \in 0..ReplyMessageCount,
       chunkCursor \in 0..ReplyChunkCount:
      ReplySourceAtCursor(owner, semantic, source,
                          messageCursor, chunkCursor)
        ~> ReplySourceAdvancedFrom(owner, semantic, source,
                                   messageCursor, chunkCursor)

ReplyCloseWorkEventuallyTerminates(requester, responder) ==
  ReplyCloseWorkPending(requester, responder)
    ~> ~ReplyCloseWorkPending(requester, responder)

=============================================================================
