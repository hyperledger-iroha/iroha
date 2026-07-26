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
ReplyServiceGenerations == 1..ReplyDeliveryOrdinalLimit
ReplyStreamEpochs == 1..ReplyDeliveryOrdinalLimit
NoReplyTicketTenure == 0
NoReplyServiceGeneration == 0
NoReplyStreamEpoch == 0
NoReplySemanticSequence == 0
ReplyActiveWindowCapacity == ReplySourceCapacity

(***************************************************************************
The model keeps the digest symbolic while retaining the production
requirement that canonical bytes determine it.  The singleton is injective
over ReplySemantics, so a hash cannot be rebound to a different semantic.
***************************************************************************)
ReplyCanonicalSemanticHash(semantic) == {semantic}

(***************************************************************************
Wire version one is the sole first-release format.  The canonical request
identity binds every immutable semantic coordinate and intentionally ignores
only the cumulative close floor.  `semantic` is the finite-model carrier for
the immutable payload; its singleton hash is the compact-reference digest.
***************************************************************************)
ReplyProtocolVersion == 1

ReplyCanonicalReference(semantic) ==
  [entryHash |-> ReplyCanonicalSemanticHash(semantic),
   encodedLength |-> ReplyMessageCount + ReplyChunkCount,
   mergeEpoch |-> ReplyMessageCount,
   referenceDigest |-> ReplyCanonicalSemanticHash(semantic)]

ReplyCanonicalRequestIdentity(
    serviceGeneration, streamEpoch, semanticSequence,
    semantic, requester, responder) ==
  [version |-> ReplyProtocolVersion,
   serviceGeneration |-> serviceGeneration,
   streamEpoch |-> streamEpoch,
   semanticSequence |-> semanticSequence,
   payload |-> semantic,
   reference |-> ReplyCanonicalReference(semantic),
   requesterPeer |-> requester,
   responderPeer |-> responder]

ReplyCanonicalRequestIdentityWithCloseFloor(
    serviceGeneration, streamEpoch, semanticSequence,
    semantic, requester, responder, closedThrough) ==
  ReplyCanonicalRequestIdentity(
    serviceGeneration, streamEpoch, semanticSequence,
    semantic, requester, responder)

ReplyRequestIdentitySet ==
  [version: {ReplyProtocolVersion},
   serviceGeneration: ReplyServiceGenerations,
   streamEpoch: ReplyStreamEpochs,
   semanticSequence: ReplySemanticSequences,
   payload: ReplySemantics,
   reference:
     [entryHash: SUBSET ReplySemantics,
      encodedLength: {ReplyMessageCount + ReplyChunkCount},
      mergeEpoch: {ReplyMessageCount},
      referenceDigest: SUBSET ReplySemantics],
   requesterPeer: ReplyOwners,
   responderPeer: ReplySources]

ReplyOccurrenceCoordinate(serviceGeneration, streamEpoch,
                          semanticSequence) ==
  [serviceGeneration |-> serviceGeneration,
   streamEpoch |-> streamEpoch,
   semanticSequence |-> semanticSequence]

ReplyOccurrenceCoordinateSet ==
  [serviceGeneration: 0..ReplyDeliveryOrdinalLimit,
   streamEpoch: 0..ReplyDeliveryOrdinalLimit,
   semanticSequence: 0..ReplyDeliveryOrdinalLimit]

ReplyCoordinateAtOrBefore(left, right) ==
  \/ left.serviceGeneration < right.serviceGeneration
  \/ /\ left.serviceGeneration = right.serviceGeneration
     /\ \/ left.streamEpoch < right.streamEpoch
        \/ /\ left.streamEpoch = right.streamEpoch
           /\ left.semanticSequence <= right.semanticSequence

ReplyCoordinateStrictlyBefore(left, right) ==
  /\ ReplyCoordinateAtOrBefore(left, right)
  /\ left # right

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
  rrCloseRetryGeneration,
  rrServiceGeneration,
  rrResponderGeneration,
  rrDurableResponderGeneration,
  rrRequesterNextStreamEpoch,
  rrRequesterStreamEpoch,
  rrCloseStreamEpoch,
  rrClosedPrefix,
  rrAttemptLifecycleIdentities,
  rrPendingHintResets,
  rrDiscardedPartialIdentities

ReplyLifecycleVars ==
  <<rrSemanticSequence, rrSemanticHash, rrRequesterNextSequence,
    rrRequesterClosedThrough>>

ReplyCloseVars ==
  <<rrClosePendingThrough, rrCloseSentThrough,
    rrCloseAcknowledgedThrough, rrCloseRetryGeneration>>

ReplyCoordinateVars ==
  <<rrServiceGeneration, rrResponderGeneration,
    rrDurableResponderGeneration, rrRequesterNextStreamEpoch,
    rrRequesterStreamEpoch, rrCloseStreamEpoch, rrClosedPrefix,
    rrAttemptLifecycleIdentities, rrPendingHintResets,
    rrDiscardedPartialIdentities>>

ReplyRouteVars ==
  <<rrAttempts, rrPayloads, rrNextDeliveryOrdinal, rrConnectionTenure,
    rrSourceActive, rrNextServiceIndex, rrSemanticSequence, rrSemanticHash,
    rrRequesterNextSequence, rrRequesterClosedThrough,
    rrClosePendingThrough, rrCloseSentThrough,
    rrCloseAcknowledgedThrough, rrCloseRetryGeneration>>

ReplyRouteV2Vars == <<ReplyRouteVars, ReplyCoordinateVars>>

(***************************************************************************
The bounded route attempt remains the process-local cursor carrier proved
below.  This second, durable carrier is its V2 occurrence identity.  The
coupling invariant gives every live route attempt exactly one identity with
the non-zero generation/epoch/sequence triple used by request gates, chunks,
flush receipts, and cancellation.
***************************************************************************)
ReplyAttemptLifecycleIdentity(
    owner, semantic, source, serviceGeneration,
    streamEpoch, semanticSequence) ==
  [owner |-> owner,
   semantic |-> semantic,
   source |-> source,
   serviceGeneration |-> serviceGeneration,
   streamEpoch |-> streamEpoch,
   semanticSequence |-> semanticSequence,
   requestIdentity |->
     ReplyCanonicalRequestIdentity(
       serviceGeneration, streamEpoch, semanticSequence,
       semantic, owner, source)]

ReplyAttemptLifecycleIdentitySet ==
  [owner: ReplyOwners,
   semantic: ReplySemantics,
   source: ReplySources,
   serviceGeneration: ReplyServiceGenerations,
   streamEpoch: ReplyStreamEpochs,
   semanticSequence: ReplySemanticSequences,
   requestIdentity: ReplyRequestIdentitySet]

ReplyAttemptLifecycleIdentitiesFor(owner, semantic, source) ==
  {identity \in rrAttemptLifecycleIdentities:
     /\ identity.owner = owner
     /\ identity.semantic = semantic
     /\ identity.source = source}

ReplyAttemptLifecycleIdentityOwned(owner, semantic, source) ==
  ReplyAttemptLifecycleIdentitiesFor(owner, semantic, source) # {}

ReplyAttemptLifecycleIdentityFor(owner, semantic, source) ==
  CHOOSE identity
    \in ReplyAttemptLifecycleIdentitiesFor(owner, semantic, source):
      TRUE

ReplyLifecycleIdentityCoordinate(identity) ==
  ReplyOccurrenceCoordinate(
    identity.serviceGeneration,
    identity.streamEpoch,
    identity.semanticSequence)

ReplyLifecycleIdentityMatchesCanonicalRequest(identity) ==
  identity.requestIdentity =
    ReplyCanonicalRequestIdentity(
      identity.serviceGeneration,
      identity.streamEpoch,
      identity.semanticSequence,
      identity.semantic,
      identity.owner,
      identity.source)

ReplyAttemptOccurrenceCancelled(identity) ==
  ReplyCoordinateAtOrBefore(
    ReplyLifecycleIdentityCoordinate(identity),
    rrClosedPrefix[identity.owner][identity.source])

ReplyAttemptOccurrenceCurrent(identity) ==
  /\ identity.serviceGeneration =
       rrServiceGeneration[identity.owner][identity.source]
  /\ identity.streamEpoch =
       rrRequesterStreamEpoch[identity.owner][identity.source]
  /\ identity.semanticSequence =
       rrSemanticSequence[identity.owner][identity.semantic]
  /\ ~ReplyAttemptOccurrenceCancelled(identity)

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

ReplyCanonicalCloseIdentity(serviceGeneration, streamEpoch, closedThrough,
                            requester, responder) ==
  [version |-> ReplyProtocolVersion,
   serviceGeneration |-> serviceGeneration,
   streamEpoch |-> streamEpoch,
   closedThrough |-> closedThrough,
   requesterPeer |-> requester,
   responderPeer |-> responder]

ReplyCloseIdentitySet ==
  [version: {ReplyProtocolVersion},
   serviceGeneration: ReplyServiceGenerations,
   streamEpoch: ReplyStreamEpochs,
   closedThrough: 0..ReplyDeliveryOrdinalLimit,
   requesterPeer: ReplyOwners,
   responderPeer: ReplySources]

ReplyCloseWitness(requester, authenticatedRequester, responder,
                  authenticatedResponder, serviceGeneration,
                  streamEpoch, closedThrough, closeIdentity,
                  bindingRequester, bindingResponder,
                  bindingServiceGeneration, bindingStreamEpoch,
                  bindingClosedThrough, bindingCloseIdentity) ==
  [version |-> ReplyProtocolVersion,
   requester |-> requester,
   authenticatedRequester |-> authenticatedRequester,
   responder |-> responder,
   authenticatedResponder |-> authenticatedResponder,
   serviceGeneration |-> serviceGeneration,
   streamEpoch |-> streamEpoch,
   closedThrough |-> closedThrough,
   closeIdentity |-> closeIdentity,
   bindingRequester |-> bindingRequester,
   bindingResponder |-> bindingResponder,
   bindingServiceGeneration |-> bindingServiceGeneration,
   bindingStreamEpoch |-> bindingStreamEpoch,
   bindingClosedThrough |-> bindingClosedThrough,
   bindingCloseIdentity |-> bindingCloseIdentity]

ReplyCloseWitnessSet ==
  [version: {ReplyProtocolVersion},
   requester: ReplyOwners,
   authenticatedRequester: ReplyOwners,
   responder: ReplySources,
   authenticatedResponder: ReplySources,
   serviceGeneration: ReplyServiceGenerations,
   streamEpoch: ReplyStreamEpochs,
   closedThrough: 0..ReplyDeliveryOrdinalLimit,
   closeIdentity: ReplyCloseIdentitySet,
   bindingRequester: ReplyOwners,
   bindingResponder: ReplySources,
   bindingServiceGeneration: ReplyServiceGenerations,
   bindingStreamEpoch: ReplyStreamEpochs,
   bindingClosedThrough: 0..ReplyDeliveryOrdinalLimit,
   bindingCloseIdentity: ReplyCloseIdentitySet]

ReplyCloseWitnessValid(witness) ==
  /\ witness \in ReplyCloseWitnessSet
  /\ witness.version = ReplyProtocolVersion
  /\ witness.authenticatedRequester = witness.requester
  /\ witness.authenticatedResponder = witness.responder
  /\ witness.serviceGeneration =
       rrServiceGeneration[witness.requester][witness.responder]
  /\ witness.streamEpoch =
       rrCloseStreamEpoch[witness.requester][witness.responder]
  /\ witness.closeIdentity =
       ReplyCanonicalCloseIdentity(
         witness.serviceGeneration, witness.streamEpoch,
         witness.closedThrough, witness.requester, witness.responder)
  /\ witness.bindingRequester = witness.requester
  /\ witness.bindingResponder = witness.responder
  /\ witness.bindingServiceGeneration = witness.serviceGeneration
  /\ witness.bindingStreamEpoch = witness.streamEpoch
  /\ witness.bindingClosedThrough = witness.closedThrough
  /\ witness.bindingCloseIdentity = witness.closeIdentity

ReplyCanonicalCloseWitness(requester, responder, closedThrough) ==
  LET serviceGeneration == rrServiceGeneration[requester][responder]
      streamEpoch == rrCloseStreamEpoch[requester][responder]
      closeIdentity ==
        ReplyCanonicalCloseIdentity(
          serviceGeneration, streamEpoch, closedThrough,
          requester, responder)
  IN ReplyCloseWitness(
       requester, requester, responder, responder,
       serviceGeneration, streamEpoch, closedThrough, closeIdentity,
       requester, responder, serviceGeneration, streamEpoch,
       closedThrough, closeIdentity)

ReplyCloseAcknowledgement(requester, responder, authenticatedResponder,
                          serviceGeneration, streamEpoch,
                          closedThrough, closeIdentity,
                          bindingRequester, bindingResponder,
                          bindingServiceGeneration, bindingStreamEpoch,
                          bindingClosedThrough, bindingCloseIdentity) ==
  [version |-> ReplyProtocolVersion,
   requester |-> requester,
   responder |-> responder,
   authenticatedResponder |-> authenticatedResponder,
   serviceGeneration |-> serviceGeneration,
   streamEpoch |-> streamEpoch,
   closedThrough |-> closedThrough,
   closeIdentity |-> closeIdentity,
   bindingRequester |-> bindingRequester,
   bindingResponder |-> bindingResponder,
   bindingServiceGeneration |-> bindingServiceGeneration,
   bindingStreamEpoch |-> bindingStreamEpoch,
   bindingClosedThrough |-> bindingClosedThrough,
   bindingCloseIdentity |-> bindingCloseIdentity]

ReplyCloseAcknowledgementSet ==
  [version: {ReplyProtocolVersion},
   requester: ReplyOwners,
   responder: ReplySources,
   authenticatedResponder: ReplySources,
   serviceGeneration: ReplyServiceGenerations,
   streamEpoch: ReplyStreamEpochs,
   closedThrough: 0..ReplyDeliveryOrdinalLimit,
   closeIdentity: ReplyCloseIdentitySet,
   bindingRequester: ReplyOwners,
   bindingResponder: ReplySources,
   bindingServiceGeneration: ReplyServiceGenerations,
   bindingStreamEpoch: ReplyStreamEpochs,
   bindingClosedThrough: 0..ReplyDeliveryOrdinalLimit,
   bindingCloseIdentity: ReplyCloseIdentitySet]

ReplyCloseAcknowledgementValid(acknowledgement) ==
  /\ acknowledgement \in ReplyCloseAcknowledgementSet
  /\ acknowledgement.version = ReplyProtocolVersion
  /\ acknowledgement.authenticatedResponder =
       acknowledgement.responder
  /\ acknowledgement.serviceGeneration =
       rrServiceGeneration
         [acknowledgement.requester][acknowledgement.responder]
  /\ acknowledgement.streamEpoch =
       rrCloseStreamEpoch
         [acknowledgement.requester][acknowledgement.responder]
  /\ acknowledgement.closeIdentity =
       ReplyCanonicalCloseIdentity(
         acknowledgement.serviceGeneration,
         acknowledgement.streamEpoch,
         acknowledgement.closedThrough,
         acknowledgement.requester,
         acknowledgement.responder)
  /\ acknowledgement.bindingRequester = acknowledgement.requester
  /\ acknowledgement.bindingResponder = acknowledgement.responder
  /\ acknowledgement.bindingServiceGeneration =
       acknowledgement.serviceGeneration
  /\ acknowledgement.bindingStreamEpoch = acknowledgement.streamEpoch
  /\ acknowledgement.bindingClosedThrough =
       acknowledgement.closedThrough
  /\ acknowledgement.bindingCloseIdentity =
       acknowledgement.closeIdentity

ReplyCanonicalCloseAcknowledgement(requester, responder, closedThrough) ==
  LET serviceGeneration == rrServiceGeneration[requester][responder]
      streamEpoch == rrCloseStreamEpoch[requester][responder]
      closeIdentity ==
        ReplyCanonicalCloseIdentity(
          serviceGeneration, streamEpoch, closedThrough,
          requester, responder)
  IN ReplyCloseAcknowledgement(
       requester, responder, responder, serviceGeneration, streamEpoch,
       closedThrough, closeIdentity, requester, responder,
       serviceGeneration, streamEpoch, closedThrough, closeIdentity)

ReplyGenerationHintKinds == {"Request", "Close"}

ReplyGenerationHint(
    requester, responder, authenticatedResponder, messageKind, semantic,
    observedGeneration, currentGeneration, observedMessageHash,
    bindingRequester, bindingResponder, bindingObservedGeneration,
    bindingCurrentGeneration, bindingObservedMessageHash) ==
  [version |-> ReplyProtocolVersion,
   requester |-> requester,
   responder |-> responder,
   authenticatedResponder |-> authenticatedResponder,
   messageKind |-> messageKind,
   semantic |-> semantic,
   observedGeneration |-> observedGeneration,
   currentGeneration |-> currentGeneration,
   observedMessageHash |-> observedMessageHash,
   bindingRequester |-> bindingRequester,
   bindingResponder |-> bindingResponder,
   bindingObservedGeneration |-> bindingObservedGeneration,
   bindingCurrentGeneration |-> bindingCurrentGeneration,
   bindingObservedMessageHash |-> bindingObservedMessageHash]

ReplyGenerationHintSet ==
  [version: {ReplyProtocolVersion},
   requester: ReplyOwners,
   responder: ReplySources,
   authenticatedResponder: ReplySources,
   messageKind: ReplyGenerationHintKinds,
   semantic: ReplySemantics,
   observedGeneration: ReplyServiceGenerations,
   currentGeneration: ReplyServiceGenerations,
   observedMessageHash:
     SUBSET (ReplyRequestIdentitySet \cup ReplyCloseIdentitySet),
   bindingRequester: ReplyOwners,
   bindingResponder: ReplySources,
   bindingObservedGeneration: ReplyServiceGenerations,
   bindingCurrentGeneration: ReplyServiceGenerations,
   bindingObservedMessageHash:
     SUBSET (ReplyRequestIdentitySet \cup ReplyCloseIdentitySet)]

ReplyOutstandingRequestHash(requester, semantic, responder) ==
  IF ReplyAttemptLifecycleIdentityOwned(
       requester, semantic, responder)
     /\ \E attempt \in rrAttempts:
          /\ attempt.owner = requester
          /\ attempt.semantic = semantic
          /\ attempt.source = responder
          /\ \/ attempt.messageCursor # ReplyMessageCount
             \/ attempt.chunkCursor # ReplyChunkCount
     /\ ~ReplyAttemptOccurrenceCancelled(
          ReplyAttemptLifecycleIdentityFor(
            requester, semantic, responder))
  THEN {ReplyAttemptLifecycleIdentityFor(
          requester, semantic, responder).requestIdentity}
  ELSE {}

ReplyOutstandingCloseHash(requester, responder) ==
  IF ReplyCloseWorkPending(requester, responder)
  THEN {ReplyCanonicalCloseIdentity(
          rrServiceGeneration[requester][responder],
          rrCloseStreamEpoch[requester][responder],
          rrClosePendingThrough[requester][responder],
          requester, responder)}
  ELSE {}

ReplyGenerationHintExactTrigger(hint) ==
  /\ hint.observedMessageHash # {}
  /\ CASE hint.messageKind = "Request" ->
            hint.observedMessageHash =
              ReplyOutstandingRequestHash(
                hint.requester, hint.semantic, hint.responder)
       [] hint.messageKind = "Close" ->
            hint.observedMessageHash =
              ReplyOutstandingCloseHash(
                hint.requester, hint.responder)

ReplyGenerationHintValid(hint) ==
  /\ hint \in ReplyGenerationHintSet
  /\ hint.version = ReplyProtocolVersion
  /\ hint.authenticatedResponder = hint.responder
  /\ hint.bindingRequester = hint.requester
  /\ hint.bindingResponder = hint.responder
  /\ hint.bindingObservedGeneration = hint.observedGeneration
  /\ hint.bindingCurrentGeneration = hint.currentGeneration
  /\ hint.bindingObservedMessageHash = hint.observedMessageHash
  /\ hint.observedGeneration =
       rrServiceGeneration[hint.requester][hint.responder]
  /\ hint.currentGeneration = rrResponderGeneration[hint.responder]
  /\ hint.currentGeneration > hint.observedGeneration
  /\ ReplyGenerationHintExactTrigger(hint)

ReplyHintReset(
    requester, responder, messageKind, semantic,
    oldGeneration, newGeneration, oldEpoch, newEpoch,
    observedMessageHash) ==
  [requester |-> requester,
   responder |-> responder,
   messageKind |-> messageKind,
   semantic |-> semantic,
   oldGeneration |-> oldGeneration,
   newGeneration |-> newGeneration,
   oldEpoch |-> oldEpoch,
   newEpoch |-> newEpoch,
   observedMessageHash |-> observedMessageHash]

ReplyHintResetSet ==
  [requester: ReplyOwners,
   responder: ReplySources,
   messageKind: ReplyGenerationHintKinds,
   semantic: ReplySemantics,
   oldGeneration: ReplyServiceGenerations,
   newGeneration: ReplyServiceGenerations,
   oldEpoch: ReplyStreamEpochs,
   newEpoch: ReplyStreamEpochs,
   observedMessageHash:
     SUBSET (ReplyRequestIdentitySet \cup ReplyCloseIdentitySet)]

ReplyResetMatchesLifecycleIdentity(reset, identity) ==
  /\ identity.owner = reset.requester
  /\ identity.source = reset.responder
  /\ identity.serviceGeneration = reset.oldGeneration
  /\ identity.streamEpoch = reset.oldEpoch

ReplyLifecycleIdentityAfterHint(reset, identity) ==
  ReplyAttemptLifecycleIdentity(
    identity.owner, identity.semantic, identity.source,
    reset.newGeneration, reset.newEpoch, identity.semanticSequence)

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

ReplyAttemptCoveredByClosedPrefix(
    attempt, requester, serviceGeneration, streamEpoch, closedThrough) ==
  /\ attempt.owner = requester
  /\ ReplyAttemptLifecycleIdentityOwned(
       attempt.owner, attempt.semantic, attempt.source)
  /\ LET identity ==
           ReplyAttemptLifecycleIdentityFor(
             attempt.owner, attempt.semantic, attempt.source)
     IN ReplyCoordinateAtOrBefore(
          ReplyLifecycleIdentityCoordinate(identity),
          ReplyOccurrenceCoordinate(
            serviceGeneration, streamEpoch, closedThrough))

ReplyAttemptsAfterClose(
    requester, serviceGeneration, streamEpoch, closedThrough) ==
  {attempt \in rrAttempts:
     ~ReplyAttemptCoveredByClosedPrefix(
       attempt, requester, serviceGeneration, streamEpoch, closedThrough)}

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
        ReplyAttemptsAfterClose(
          requester, witness.serviceGeneration,
          witness.streamEpoch, closedThrough)
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

(***************************************************************************
V2 lifecycle composition.

The route kernel above is the established cursor/tenure projection.  These
actions couple it to the sole durable first-release lifecycle.  Stream epochs
are reserved in their own persistence step before an occurrence may be used.
Generation-hint acceptance likewise persists a fresh epoch and strictly
advances the expected generation before the old partial identity is discarded
in a later step.  Rejected future-generation input and route-free hint emission
are fail-atomic stutters.
***************************************************************************)
ReplyRouteV2Init ==
  /\ ReplyRouteInit
  /\ rrServiceGeneration =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> 1]]
  /\ rrResponderGeneration =
       [source \in ReplySources |-> 1]
  /\ rrDurableResponderGeneration =
       [source \in ReplySources |-> 1]
  /\ rrRequesterNextStreamEpoch =
       [owner \in ReplyOwners |-> 1]
  /\ rrRequesterStreamEpoch =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> NoReplyStreamEpoch]]
  /\ rrCloseStreamEpoch =
       [owner \in ReplyOwners |->
          [source \in ReplySources |-> NoReplyStreamEpoch]]
  /\ rrClosedPrefix =
       [owner \in ReplyOwners |->
          [source \in ReplySources |->
             ReplyOccurrenceCoordinate(
               NoReplyServiceGeneration,
               NoReplyStreamEpoch,
               NoReplySemanticSequence)]]
  /\ rrAttemptLifecycleIdentities = {}
  /\ rrPendingHintResets = {}
  /\ rrDiscardedPartialIdentities = {}

PersistFreshRequesterStreamEpoch(owner, source) ==
  LET streamEpoch == rrRequesterNextStreamEpoch[owner]
  IN /\ owner \in ReplyOwners
     /\ source \in ReplySources
     /\ rrRequesterStreamEpoch[owner][source] = NoReplyStreamEpoch
     /\ ~\E attempt \in rrAttempts:
          /\ attempt.owner = owner
          /\ attempt.source = source
     /\ streamEpoch \in ReplyStreamEpochs
     /\ rrRequesterStreamEpoch' =
          [rrRequesterStreamEpoch EXCEPT
             ![owner][source] = streamEpoch]
     /\ rrCloseStreamEpoch' =
          [rrCloseStreamEpoch EXCEPT
             ![owner][source] = streamEpoch]
     /\ rrRequesterNextStreamEpoch' =
          [rrRequesterNextStreamEpoch EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<ReplyRouteVars, rrServiceGeneration,
                    rrResponderGeneration,
                    rrDurableResponderGeneration,
                    rrClosedPrefix,
                    rrAttemptLifecycleIdentities,
                    rrPendingHintResets,
                    rrDiscardedPartialIdentities>>

ObserveNewReplySourceV2(owner, semantic, source) ==
  LET semanticSequence ==
        IF ReplySemanticBound(owner, semantic)
        THEN rrSemanticSequence[owner][semantic]
        ELSE rrRequesterNextSequence[owner]
      identity ==
        ReplyAttemptLifecycleIdentity(
          owner, semantic, source,
          rrServiceGeneration[owner][source],
          rrRequesterStreamEpoch[owner][source],
          semanticSequence)
  IN /\ rrRequesterStreamEpoch[owner][source]
          \in ReplyStreamEpochs
     /\ ~ReplyAttemptLifecycleIdentityOwned(
          owner, semantic, source)
     /\ ObserveNewReplySource(owner, semantic, source)
     /\ rrAttemptLifecycleIdentities' =
          rrAttemptLifecycleIdentities \cup {identity}
     /\ UNCHANGED <<rrServiceGeneration,
                    rrResponderGeneration,
                    rrDurableResponderGeneration,
                    rrRequesterNextStreamEpoch,
                    rrRequesterStreamEpoch,
                    rrCloseStreamEpoch, rrClosedPrefix,
                    rrPendingHintResets,
                    rrDiscardedPartialIdentities>>

ObserveLaterReplyDeliveryV2(owner, semantic, source) ==
  /\ ReplyAttemptLifecycleIdentityOwned(owner, semantic, source)
  /\ ReplyAttemptOccurrenceCurrent(
       ReplyAttemptLifecycleIdentityFor(owner, semantic, source))
  /\ ObserveLaterReplyDelivery(owner, semantic, source)
  /\ UNCHANGED ReplyCoordinateVars

RetryExactReplySourceV2(owner, semantic, source) ==
  /\ ReplyAttemptLifecycleIdentityOwned(owner, semantic, source)
  /\ ReplyAttemptOccurrenceCurrent(
       ReplyAttemptLifecycleIdentityFor(owner, semantic, source))
  /\ RetryExactReplySource(owner, semantic, source)
  /\ UNCHANGED ReplyCoordinateVars

RetireReplySourceV2(owner, source) ==
  /\ RetireReplySource(owner, source)
  /\ UNCHANGED ReplyCoordinateVars

ReconnectReplySourceV2(owner, semantic, source) ==
  /\ ReplyAttemptLifecycleIdentityOwned(owner, semantic, source)
  /\ ReplyAttemptOccurrenceCurrent(
       ReplyAttemptLifecycleIdentityFor(owner, semantic, source))
  /\ ReconnectReplySource(owner, semantic, source)
  /\ UNCHANGED ReplyCoordinateVars

AcquireReplyTicketV2(owner, semantic, source) ==
  /\ ReplyAttemptLifecycleIdentityOwned(owner, semantic, source)
  /\ ReplyAttemptOccurrenceCurrent(
       ReplyAttemptLifecycleIdentityFor(owner, semantic, source))
  /\ AcquireReplyTicket(owner, semantic, source)
  /\ UNCHANGED ReplyCoordinateVars

ServiceReplyRouteV2(owner, semantic) ==
  /\ ServiceReplyRoute(owner, semantic)
  /\ UNCHANGED ReplyCoordinateVars

AdvanceCurrentReplyAttemptV2(owner, semantic, source) ==
  /\ ReplyAttemptLifecycleIdentityOwned(owner, semantic, source)
  /\ ReplyAttemptOccurrenceCurrent(
       ReplyAttemptLifecycleIdentityFor(owner, semantic, source))
  /\ AdvanceCurrentReplyAttempt(owner, semantic, source)
  /\ UNCHANGED ReplyCoordinateVars

ReplyCloseCoordinate(witness) ==
  ReplyOccurrenceCoordinate(
    witness.serviceGeneration,
    witness.streamEpoch,
    witness.closedThrough)

ReplyClosedPrefixUpdate(witness) ==
  LET oldFloor ==
        rrClosedPrefix[witness.requester][witness.responder]
      newFloor == ReplyCloseCoordinate(witness)
      cancelled ==
        {identity \in rrAttemptLifecycleIdentities:
           /\ identity.owner = witness.requester
           /\ ReplyCoordinateAtOrBefore(
                ReplyLifecycleIdentityCoordinate(identity),
                newFloor)}
  IN /\ ReplyCloseWitnessValid(witness)
     /\ ReplyCoordinateAtOrBefore(oldFloor, newFloor)
     /\ rrClosedPrefix' =
          [rrClosedPrefix EXCEPT
             ![witness.requester] =
               [source \in ReplySources |-> newFloor]]
     /\ rrDiscardedPartialIdentities' =
          rrDiscardedPartialIdentities \cup cancelled
     /\ rrAttemptLifecycleIdentities' =
          rrAttemptLifecycleIdentities \ cancelled
     /\ UNCHANGED <<rrServiceGeneration,
                    rrResponderGeneration,
                    rrDurableResponderGeneration,
                    rrRequesterNextStreamEpoch,
                    rrRequesterStreamEpoch,
                    rrCloseStreamEpoch,
                    rrPendingHintResets>>

CoalesceAuthenticatedClosedPrefix(witness) ==
  /\ ReplyClosedPrefixUpdate(witness)
  /\ UNCHANGED ReplyRouteVars

CloseSemanticRequestV2(witness) ==
  /\ CloseSemanticRequest(witness)
  /\ ReplyClosedPrefixUpdate(witness)

PiggybackCloseSemanticRequestV2(witness) ==
  CloseSemanticRequestV2(witness)

RetryCloseSemanticRequestV2(witness) ==
  /\ RetryCloseSemanticRequest(witness)
  /\ UNCHANGED ReplyCoordinateVars

AcknowledgeCloseSemanticRequestV2(acknowledgement) ==
  /\ AcknowledgeCloseSemanticRequest(acknowledgement)
  /\ UNCHANGED ReplyCoordinateVars

PersistFreshEpochForGenerationHint(hint) ==
  LET freshEpoch == rrRequesterNextStreamEpoch[hint.requester]
      oldEpoch ==
        rrRequesterStreamEpoch[hint.requester][hint.responder]
      reset ==
        ReplyHintReset(
          hint.requester, hint.responder, hint.messageKind, hint.semantic,
          hint.observedGeneration, hint.currentGeneration,
          oldEpoch, freshEpoch, hint.observedMessageHash)
  IN /\ ReplyGenerationHintValid(hint)
     /\ freshEpoch \in ReplyStreamEpochs
     /\ oldEpoch \in ReplyStreamEpochs
     /\ \A pending \in rrPendingHintResets:
          /\ pending.requester = hint.requester
          /\ pending.responder = hint.responder
          => FALSE
     /\ rrServiceGeneration' =
          [rrServiceGeneration EXCEPT
             ![hint.requester][hint.responder] =
               hint.currentGeneration]
     /\ rrRequesterNextStreamEpoch' =
          [rrRequesterNextStreamEpoch EXCEPT
             ![hint.requester] = @ + 1]
     /\ rrRequesterStreamEpoch' =
          [rrRequesterStreamEpoch EXCEPT
             ![hint.requester][hint.responder] = freshEpoch]
     /\ rrCloseStreamEpoch' =
          [rrCloseStreamEpoch EXCEPT
             ![hint.requester][hint.responder] = freshEpoch]
     /\ rrPendingHintResets' = rrPendingHintResets \cup {reset}
     /\ UNCHANGED <<ReplyRouteVars,
                    rrResponderGeneration,
                    rrDurableResponderGeneration,
                    rrClosedPrefix,
                    rrAttemptLifecycleIdentities,
                    rrDiscardedPartialIdentities>>

DiscardPersistedHintPartialState(reset) ==
  LET discarded ==
        {identity \in rrAttemptLifecycleIdentities:
           ReplyResetMatchesLifecycleIdentity(reset, identity)}
      successorIdentities ==
        {IF ReplyResetMatchesLifecycleIdentity(reset, identity)
         THEN ReplyLifecycleIdentityAfterHint(reset, identity)
         ELSE identity:
           identity \in rrAttemptLifecycleIdentities}
  IN /\ reset \in rrPendingHintResets
     /\ rrServiceGeneration[reset.requester][reset.responder] =
          reset.newGeneration
     /\ rrRequesterStreamEpoch
          [reset.requester][reset.responder] = reset.newEpoch
     /\ rrCloseStreamEpoch
          [reset.requester][reset.responder] = reset.newEpoch
     /\ rrAttemptLifecycleIdentities' = successorIdentities
     /\ rrDiscardedPartialIdentities' =
          rrDiscardedPartialIdentities \cup discarded
     /\ rrPendingHintResets' = rrPendingHintResets \ {reset}
     /\ UNCHANGED <<ReplyRouteVars, rrServiceGeneration,
                    rrResponderGeneration,
                    rrDurableResponderGeneration,
                    rrRequesterNextStreamEpoch,
                    rrRequesterStreamEpoch,
                    rrCloseStreamEpoch, rrClosedPrefix>>

ReplyResponderStateTerminal(source) ==
  /\ \A attempt \in rrAttempts:
       attempt.source = source =>
         /\ ReplyAttemptComplete(attempt)
         /\ ReplyAttemptHasNoTicket(attempt)
  /\ \A owner \in ReplyOwners:
       ~ReplyCloseWorkPending(owner, source)
  /\ \A reset \in rrPendingHintResets:
       reset.responder # source

PersistTerminalResponderGeneration(source) ==
  /\ source \in ReplySources
  /\ ReplyResponderStateTerminal(source)
  /\ rrResponderGeneration[source] =
       rrDurableResponderGeneration[source]
  /\ rrDurableResponderGeneration[source]
       < ReplyDeliveryOrdinalLimit
  /\ rrDurableResponderGeneration' =
       [rrDurableResponderGeneration EXCEPT ![source] = @ + 1]
  /\ UNCHANGED <<ReplyRouteVars, rrServiceGeneration,
                 rrResponderGeneration,
                 rrRequesterNextStreamEpoch,
                 rrRequesterStreamEpoch, rrCloseStreamEpoch,
                 rrClosedPrefix, rrAttemptLifecycleIdentities,
                 rrPendingHintResets, rrDiscardedPartialIdentities>>

InstallPersistedResponderGeneration(source) ==
  /\ source \in ReplySources
  /\ ReplyResponderStateTerminal(source)
  /\ rrDurableResponderGeneration[source] =
       rrResponderGeneration[source] + 1
  /\ rrResponderGeneration' =
       [rrResponderGeneration EXCEPT
          ![source] = rrDurableResponderGeneration[source]]
  /\ UNCHANGED <<ReplyRouteVars, rrServiceGeneration,
                 rrDurableResponderGeneration,
                 rrRequesterNextStreamEpoch,
                 rrRequesterStreamEpoch, rrCloseStreamEpoch,
                 rrClosedPrefix, rrAttemptLifecycleIdentities,
                 rrPendingHintResets, rrDiscardedPartialIdentities>>

RejectFutureGenerationWithoutMutation(
    requester, responder, inputGeneration) ==
  /\ requester \in ReplyOwners
  /\ responder \in ReplySources
  /\ inputGeneration \in ReplyServiceGenerations
  /\ inputGeneration > rrResponderGeneration[responder]
  /\ UNCHANGED ReplyRouteV2Vars

RejectRequesterEpochOverflowWithoutMutation(requester) ==
  /\ requester \in ReplyOwners
  /\ rrRequesterNextStreamEpoch[requester] =
       ReplyDeliveryOrdinalLimit + 1
  /\ UNCHANGED ReplyRouteV2Vars

RejectNonTerminalResponderCompactionWithoutMutation(source) ==
  /\ source \in ReplySources
  /\ ~ReplyResponderStateTerminal(source)
  /\ UNCHANGED ReplyRouteV2Vars

ReturnOlderGenerationHintWithoutRoute(
    requester, responder, observedMessageHash) ==
  /\ requester \in ReplyOwners
  /\ responder \in ReplySources
  /\ rrServiceGeneration[requester][responder]
       < rrResponderGeneration[responder]
  /\ observedMessageHash
       \in {ReplyOutstandingCloseHash(requester, responder)}
            \cup
            {ReplyOutstandingRequestHash(
               requester, semantic, responder):
               semantic \in ReplySemantics}
  /\ UNCHANGED ReplyRouteV2Vars

RejectResponderGenerationOverflow(source) ==
  /\ source \in ReplySources
  /\ rrDurableResponderGeneration[source] =
       ReplyDeliveryOrdinalLimit
  /\ UNCHANGED ReplyRouteV2Vars

ReplyRouteV2Next ==
  \/ \E owner \in ReplyOwners, source \in ReplySources:
       PersistFreshRequesterStreamEpoch(owner, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveNewReplySourceV2(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveLaterReplyDeliveryV2(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       RetryExactReplySourceV2(owner, semantic, source)
  \/ \E owner \in ReplyOwners, source \in ReplySources:
       RetireReplySourceV2(owner, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ReconnectReplySourceV2(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       AcquireReplyTicketV2(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics:
       ServiceReplyRouteV2(owner, semantic)
  \/ \E witness \in ReplyCloseWitnessSet:
       CloseSemanticRequestV2(witness)
  \/ \E witness \in ReplyCloseWitnessSet:
       PiggybackCloseSemanticRequestV2(witness)
  \/ \E witness \in ReplyCloseWitnessSet:
       RetryCloseSemanticRequestV2(witness)
  \/ \E acknowledgement \in ReplyCloseAcknowledgementSet:
       AcknowledgeCloseSemanticRequestV2(acknowledgement)
  \/ \E hint \in ReplyGenerationHintSet:
       PersistFreshEpochForGenerationHint(hint)
  \/ \E reset \in ReplyHintResetSet:
       DiscardPersistedHintPartialState(reset)
  \/ \E source \in ReplySources:
       PersistTerminalResponderGeneration(source)
  \/ \E source \in ReplySources:
       InstallPersistedResponderGeneration(source)
  \/ \E requester \in ReplyOwners, responder \in ReplySources,
       inputGeneration \in ReplyServiceGenerations:
       RejectFutureGenerationWithoutMutation(
         requester, responder, inputGeneration)
  \/ \E requester \in ReplyOwners:
       RejectRequesterEpochOverflowWithoutMutation(requester)
  \/ \E source \in ReplySources:
       RejectNonTerminalResponderCompactionWithoutMutation(source)
  \/ \E requester \in ReplyOwners, responder \in ReplySources,
       observedMessageHash
         \in SUBSET (ReplyRequestIdentitySet \cup ReplyCloseIdentitySet):
       ReturnOlderGenerationHintWithoutRoute(
         requester, responder, observedMessageHash)
  \/ \E source \in ReplySources:
       RejectResponderGenerationOverflow(source)

(***************************************************************************
The V2 lifecycle retains the exact local scheduling premises needed by the
per-source cursor and cumulative-close arguments.  The vector is the complete
V2 state: coordinate-only persistence, hint cleanup, and generation rollover
steps therefore cannot be used to satisfy fairness for ticket acquisition,
exact-item service, close retry, or close acknowledgement.

These are explicit environment/runtime premises.  They do not assert network
responsiveness, create an admission ticket, or turn a route-free generation
hint into reply authority.
***************************************************************************)
ReplyRouteV2Fairness ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       WF_ReplyRouteV2Vars(
         AcquireReplyTicketV2(owner, semantic, source))
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics:
       WF_ReplyRouteV2Vars(ServiceReplyRouteV2(owner, semantic))
  /\ \A witness \in ReplyCloseWitnessSet:
       WF_ReplyRouteV2Vars(RetryCloseSemanticRequestV2(witness))
  /\ \A acknowledgement \in ReplyCloseAcknowledgementSet:
       WF_ReplyRouteV2Vars(
         AcknowledgeCloseSemanticRequestV2(acknowledgement))

ReplyRouteV2Spec ==
  ReplyRouteV2Init
    /\ [][ReplyRouteV2Next]_ReplyRouteV2Vars
    /\ ReplyRouteV2Fairness

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
  \E identity \in rrAttemptLifecycleIdentities:
    /\ identity.owner = attempt.owner
    /\ identity.semantic = attempt.semantic
    /\ identity.source = attempt.source
    /\ ~ReplyAttemptOccurrenceCancelled(identity)
    /\ ReplyCoordinateAtOrBefore(
         ReplyLifecycleIdentityCoordinate(identity),
         rrClosedPrefix'[attempt.owner][attempt.source])
    /\ identity \notin rrAttemptLifecycleIdentities'
    /\ identity \in rrDiscardedPartialIdentities'

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
  [][ReplyTenureAwareReplayStep]_ReplyRouteV2Vars

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
  [][ReplySourceIsolationStep]_ReplyRouteV2Vars

ReplyRouteSafetyInvariant ==
  /\ ReplyRouteTypeInvariant
  /\ ReplyRouteOwnershipInvariant

ReplyRouteLifecycleInvariant ==
  /\ ReplyLifecycleTypeInvariant
  /\ ReplyLifecycleOwnershipInvariant

ReplyRouteFullSafetyInvariant ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplyRouteLifecycleInvariant

ReplyRouteV2CoordinateTypeInvariant ==
  /\ rrServiceGeneration
       \in [ReplyOwners ->
             [ReplySources -> ReplyServiceGenerations]]
  /\ rrResponderGeneration
       \in [ReplySources -> ReplyServiceGenerations]
  /\ rrDurableResponderGeneration
       \in [ReplySources -> ReplyServiceGenerations]
  /\ rrRequesterNextStreamEpoch
       \in [ReplyOwners -> 1..(ReplyDeliveryOrdinalLimit + 1)]
  /\ rrRequesterStreamEpoch
       \in [ReplyOwners ->
             [ReplySources -> 0..ReplyDeliveryOrdinalLimit]]
  /\ rrCloseStreamEpoch
       \in [ReplyOwners ->
             [ReplySources -> 0..ReplyDeliveryOrdinalLimit]]
  /\ rrClosedPrefix
       \in [ReplyOwners ->
             [ReplySources -> ReplyOccurrenceCoordinateSet]]
  /\ rrAttemptLifecycleIdentities
       \subseteq ReplyAttemptLifecycleIdentitySet
  /\ rrPendingHintResets \subseteq ReplyHintResetSet
  /\ rrDiscardedPartialIdentities
       \subseteq ReplyAttemptLifecycleIdentitySet

ReplyAttemptLifecycleIdentityInvariant ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       /\ IsFiniteSet(
            ReplyAttemptLifecycleIdentitiesFor(
              owner, semantic, source))
       /\ Cardinality(
            ReplyAttemptLifecycleIdentitiesFor(
              owner, semantic, source)) <= 1
       /\ ReplyAttemptOwned(owner, semantic, source) =>
            ReplyAttemptLifecycleIdentityOwned(
              owner, semantic, source)
  /\ \A identity \in rrAttemptLifecycleIdentities:
       /\ ReplyLifecycleIdentityMatchesCanonicalRequest(identity)
       /\ ReplyAttemptOwned(
            identity.owner, identity.semantic, identity.source)
            \/ identity \in rrDiscardedPartialIdentities
            \/ ReplyAttemptOccurrenceCancelled(identity)
       /\ ReplyAttemptOccurrenceCurrent(identity)
            \/ identity \in rrDiscardedPartialIdentities
            \/ \E reset \in rrPendingHintResets:
                 ReplyResetMatchesLifecycleIdentity(reset, identity)
  /\ \A identity \in rrAttemptLifecycleIdentities:
       identity \notin rrDiscardedPartialIdentities
  /\ \A discarded \in rrDiscardedPartialIdentities:
       \A current \in rrAttemptLifecycleIdentities:
         discarded.requestIdentity # current.requestIdentity

ReplyRequesterEpochPersistenceInvariant ==
  /\ \A owner \in ReplyOwners, source \in ReplySources:
       /\ rrRequesterStreamEpoch[owner][source] =
            rrCloseStreamEpoch[owner][source]
       /\ rrRequesterStreamEpoch[owner][source] # NoReplyStreamEpoch
            => rrRequesterStreamEpoch[owner][source]
                 < rrRequesterNextStreamEpoch[owner]
       /\ (\E attempt \in rrAttempts:
             /\ attempt.owner = owner
             /\ attempt.source = source)
            => rrRequesterStreamEpoch[owner][source]
                 \in ReplyStreamEpochs
  /\ \A reset \in rrPendingHintResets:
       /\ rrServiceGeneration[reset.requester][reset.responder] =
            reset.newGeneration
       /\ reset.newGeneration > reset.oldGeneration
       /\ reset.newEpoch < rrRequesterNextStreamEpoch[reset.requester]
       /\ rrRequesterStreamEpoch
            [reset.requester][reset.responder] = reset.newEpoch
       /\ rrCloseStreamEpoch
            [reset.requester][reset.responder] = reset.newEpoch

ReplyClosedPrefixLexicographicInvariant ==
  /\ \A owner \in ReplyOwners, source \in ReplySources:
       rrClosedPrefix[owner][source]
         \in ReplyOccurrenceCoordinateSet
  /\ \A owner \in ReplyOwners, left, right \in ReplySources:
       rrClosedPrefix[owner][left] = rrClosedPrefix[owner][right]
  /\ \A identity \in rrAttemptLifecycleIdentities:
       ReplyAttemptOccurrenceCancelled(identity) =>
         /\ identity \in rrDiscardedPartialIdentities
            \/ ~ReplyAttemptOwned(
                 identity.owner, identity.semantic, identity.source)
            \/ ReplyAttemptComplete(
                 ReplyAttemptFor(
                   identity.owner, identity.semantic, identity.source))

ReplyTerminalRolloverCompositionInvariant ==
  \A source \in ReplySources:
    /\ rrResponderGeneration[source]
         <= rrDurableResponderGeneration[source]
    /\ rrDurableResponderGeneration[source]
         <= rrResponderGeneration[source] + 1
    /\ rrDurableResponderGeneration[source]
         > rrResponderGeneration[source]
         => ReplyResponderStateTerminal(source)

ReplyRouteV2CoordinateSafetyInvariant ==
  /\ ReplyRouteV2CoordinateTypeInvariant
  /\ ReplyAttemptLifecycleIdentityInvariant
  /\ ReplyRequesterEpochPersistenceInvariant
  /\ ReplyClosedPrefixLexicographicInvariant
  /\ ReplyTerminalRolloverCompositionInvariant

ReplyRouteV2SafetyInvariant ==
  /\ ReplyRouteFullSafetyInvariant
  /\ ReplyRouteV2CoordinateSafetyInvariant

ReplyStaleArtifactCannotAffectSuccessor ==
  \A stale \in rrDiscardedPartialIdentities,
     successor \in rrAttemptLifecycleIdentities:
    /\ stale.owner = successor.owner
    /\ stale.semantic = successor.semantic
    /\ stale.source = successor.source
    => /\ stale.requestIdentity # successor.requestIdentity
       /\ ReplyCoordinateStrictlyBefore(
            ReplyLifecycleIdentityCoordinate(stale),
            ReplyLifecycleIdentityCoordinate(successor))

ReplyFutureGenerationRejectIsAtomic ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     inputGeneration \in ReplyServiceGenerations:
    RejectFutureGenerationWithoutMutation(
      requester, responder, inputGeneration)
      => UNCHANGED ReplyRouteV2Vars

ReplyCapacityRejectIsAtomic ==
  /\ \A requester \in ReplyOwners:
       RejectRequesterEpochOverflowWithoutMutation(requester)
         => UNCHANGED ReplyRouteV2Vars
  /\ \A source \in ReplySources:
       /\ RejectNonTerminalResponderCompactionWithoutMutation(source)
            => UNCHANGED ReplyRouteV2Vars
       /\ RejectResponderGenerationOverflow(source)
            => UNCHANGED ReplyRouteV2Vars

ReplyHintPersistencePrecedesPartialDiscard ==
  \A reset \in ReplyHintResetSet:
    DiscardPersistedHintPartialState(reset) =>
      /\ rrServiceGeneration[reset.requester][reset.responder] =
           reset.newGeneration
      /\ reset.newEpoch <
           rrRequesterNextStreamEpoch[reset.requester]

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
