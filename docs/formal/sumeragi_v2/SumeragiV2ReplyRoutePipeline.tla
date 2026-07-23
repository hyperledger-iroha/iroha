---- MODULE SumeragiV2ReplyRoutePipeline ----
EXTENDS SumeragiV2ReplyRouteOwnership

(***************************************************************************
Concrete local pipeline around bounded reply-route ownership.

Authenticated delivery observation is separated from fair local attachment.
Every exact output item then owns a stable FIFO ordinal within one
authenticated source/output-class lane.  Actor tickets bind that source,
class, connection tenure, and canonical immutable item.  Ordinary output
linearizes at actor admission.  Flush-required output passes through explicit
ticketed, admitted, and flushed phases and advances its source cursor only
after the flush receipt is applied.

A physical reconnect may be observed while an old writer admission is still
unresolved.  The abstract retire/reconnect is deliberately delayed: a
successful old flush linearizes first, while a failed writer closes first and
then permits retirement.  Consequently a reconnect retry remains represented
by a pending attachment and cannot become actor-admitted concurrently with the
old occurrence.
***************************************************************************)

CONSTANTS
  ReplyOutputClasses,
  ReplyItemClass(_, _, _),
  ReplyItemRequiresFlush(_, _, _)

ReplyAttachmentKinds == {"New", "Exact", "Later", "Reconnect"}
ReplyPipelinePhases == {"Queued", "Ticketed", "Admitted", "Flushed"}
NoReplyPipelineTicket == 0

ReplyPipelineOrdinalLimit ==
  ReplyDeliveryOrdinalLimit
    * ReplySourceCapacity
    * Cardinality(ReplySemantics)
    * (ReplyMessageCount + ReplyChunkCount + 1)

ReplyPipelineConfiguration ==
  /\ ReplyRouteConfiguration
  /\ IsFiniteSet(ReplyOutputClasses)
  /\ ReplyOutputClasses # {}
  /\ \A semantic \in ReplySemantics,
       messageCursor \in 0..ReplyMessageCount,
       chunkCursor \in 0..ReplyChunkCount:
       /\ ReplyItemClass(semantic, messageCursor, chunkCursor)
            \in ReplyOutputClasses
       /\ ReplyItemRequiresFlush(
            semantic, messageCursor, chunkCursor) \in BOOLEAN
  /\ ReplyPipelineOrdinalLimit \in Nat \ {0}

ReplyAttachment(owner, semantic, source, kind) ==
  [owner |-> owner, semantic |-> semantic, source |-> source,
   kind |-> kind]

ReplyAttachmentSet ==
  [owner: ReplyOwners, semantic: ReplySemantics,
   source: ReplySources, kind: ReplyAttachmentKinds]

ReplyPipelinePayload(semantic, messageCursor, chunkCursor) ==
  [semantic |-> semantic,
   target |-> ReplySemanticTarget(semantic),
   messageCursor |-> messageCursor,
   chunkCursor |-> chunkCursor,
   outputClass |-> ReplyItemClass(
                       semantic, messageCursor, chunkCursor)]

(***************************************************************************
The payload carrier is typed fieldwise.  Reachable ticket payloads remain
the exact canonical records above: `ReplyPipelineItemPhaseBinding` fixes the
singleton payload for every live ticket.  Keeping the carrier fieldwise is
therefore equivalent on reachable items while avoiding a dependent image-set
whose three bound coordinates are not supported by every strict TLAPS backend.
***************************************************************************)
ReplyPipelinePayloads ==
  [semantic: ReplySemantics,
   target: ReplyTargets,
   messageCursor: 0..ReplyMessageCount,
   chunkCursor: 0..ReplyChunkCount,
   outputClass: ReplyOutputClasses]

ReplyPipelineRawItem(owner, semantic, source, messageCursor, chunkCursor,
                     outputClass, flushRequired, fifoOrdinal,
                     routeTenure, phase, ticketId, ticketTenure,
                     ticketPayload) ==
  [owner |-> owner, semantic |-> semantic, source |-> source,
   messageCursor |-> messageCursor, chunkCursor |-> chunkCursor,
   outputClass |-> outputClass, flushRequired |-> flushRequired,
   fifoOrdinal |-> fifoOrdinal, routeTenure |-> routeTenure,
   phase |-> phase, ticketId |-> ticketId,
   ticketTenure |-> ticketTenure, ticketPayload |-> ticketPayload]

ReplyPipelineItem(owner, semantic, source, messageCursor, chunkCursor,
                  fifoOrdinal, routeTenure, phase, ticketId,
                  ticketTenure, ticketPayload) ==
  ReplyPipelineRawItem(
    owner, semantic, source, messageCursor, chunkCursor,
    ReplyItemClass(semantic, messageCursor, chunkCursor),
    ReplyItemRequiresFlush(semantic, messageCursor, chunkCursor),
    fifoOrdinal, routeTenure, phase, ticketId,
    ticketTenure, ticketPayload)

ReplyPipelineItemSet ==
  [owner: ReplyOwners, semantic: ReplySemantics, source: ReplySources,
   messageCursor: 0..ReplyMessageCount,
   chunkCursor: 0..ReplyChunkCount,
   outputClass: ReplyOutputClasses, flushRequired: BOOLEAN,
   fifoOrdinal: 1..ReplyPipelineOrdinalLimit,
   routeTenure: ReplyConnectionTenures,
   phase: ReplyPipelinePhases,
   ticketId: Nat,
   ticketTenure: 0..ReplyDeliveryOrdinalLimit,
   ticketPayload: SUBSET ReplyPipelinePayloads]

ReplyPipelineItemHasType(item) ==
  /\ item.owner \in ReplyOwners
  /\ item.semantic \in ReplySemantics
  /\ item.source \in ReplySources
  /\ item.messageCursor \in 0..ReplyMessageCount
  /\ item.chunkCursor \in 0..ReplyChunkCount
  /\ item.outputClass \in ReplyOutputClasses
  /\ item.flushRequired \in BOOLEAN
  /\ item.fifoOrdinal \in 1..ReplyPipelineOrdinalLimit
  /\ item.routeTenure \in ReplyConnectionTenures
  /\ item.phase \in ReplyPipelinePhases
  /\ item.ticketId \in Nat
  /\ item.ticketTenure \in 0..ReplyDeliveryOrdinalLimit
  /\ item.ticketPayload \in SUBSET ReplyPipelinePayloads

VARIABLES
  rpPendingAttachments,
  rpItems,
  rpNextFifoOrdinal,
  rpNextTicketId

ReplyPipelineLocalVars ==
  <<rpPendingAttachments, rpItems,
    rpNextFifoOrdinal, rpNextTicketId>>

ReplyPipelineVars == <<ReplyRouteVars, ReplyPipelineLocalVars>>

ReplyPendingAttachmentsFor(owner, semantic, source) ==
  {attachment \in rpPendingAttachments:
     /\ attachment.owner = owner
     /\ attachment.semantic = semantic
     /\ attachment.source = source}

ReplyPendingAttachmentOwned(owner, semantic, source) ==
  ReplyPendingAttachmentsFor(owner, semantic, source) # {}

ReplyPendingAttachmentFor(owner, semantic, source) ==
  CHOOSE attachment \in
    ReplyPendingAttachmentsFor(owner, semantic, source): TRUE

ReplyReconnectPending(owner, semantic, source) ==
  \E attachment \in ReplyPendingAttachmentsFor(owner, semantic, source):
    attachment.kind = "Reconnect"

ReplyRouteRebindPending(owner, semantic, source) ==
  \E attachment \in ReplyPendingAttachmentsFor(owner, semantic, source):
    attachment.kind \in {"Later", "Reconnect"}

ReplyReconnectPendingForSource(owner, source) ==
  \E semantic \in ReplySemantics:
    ReplyReconnectPending(owner, semantic, source)

ReplyOwnerOrdinalReservations(owner) ==
  Cardinality(
    {attachment \in rpPendingAttachments:
       /\ attachment.owner = owner
       /\ attachment.kind # "Exact"})

ReplyPipelineItemsFor(owner, semantic, source) ==
  {item \in rpItems:
     /\ item.owner = owner
     /\ item.semantic = semantic
     /\ item.source = source}

ReplyPipelineItemOwned(owner, semantic, source) ==
  ReplyPipelineItemsFor(owner, semantic, source) # {}

ReplyPipelineItemFor(owner, semantic, source) ==
  CHOOSE item \in ReplyPipelineItemsFor(owner, semantic, source): TRUE

ReplyPipelinePayloadForItem(item) ==
  ReplyPipelinePayload(
    item.semantic, item.messageCursor, item.chunkCursor)

ReplyPipelineItemMatchesAttempt(item, attempt) ==
  /\ item.owner = attempt.owner
  /\ item.semantic = attempt.semantic
  /\ item.source = attempt.source
  /\ item.messageCursor = attempt.messageCursor
  /\ item.chunkCursor = attempt.chunkCursor

ReplyPipelineLiveCurrentCursor(
    owner, semantic, source, item, attempt) ==
  /\ ReplyAttemptOwned(owner, semantic, source)
  /\ ReplyAttemptCurrent(attempt)
  /\ rrSourceActive[owner][source]
  /\ attempt.connectionTenure =
       rrConnectionTenure[owner][source]
  /\ ~ReplyAttemptComplete(attempt)
  /\ ReplyPipelineItemMatchesAttempt(item, attempt)
  /\ item.routeTenure = attempt.connectionTenure

ReplyPipelineQueuedItem(item) ==
  /\ item.phase = "Queued"
  /\ item.ticketId = NoReplyPipelineTicket
  /\ item.ticketTenure = NoReplyTicketTenure
  /\ item.ticketPayload = {}

ReplyPipelineTicketValid(item) ==
  /\ item.phase \in {"Ticketed", "Admitted", "Flushed"}
  /\ item.ticketId \in Nat \ {0}
  /\ item.ticketId < rpNextTicketId[item.owner]
  /\ item.ticketTenure = item.routeTenure
  /\ item.ticketPayload = {ReplyPipelinePayloadForItem(item)}

ReplyPipelineExactTicketAuthority(
    owner, semantic, source, item, attempt) ==
  /\ ReplyPipelineLiveCurrentCursor(
       owner, semantic, source, item, attempt)
  /\ ReplyPipelineTicketValid(item)

ReplyPipelineItemWithoutTicket(item) ==
  ReplyPipelineRawItem(
    item.owner, item.semantic, item.source,
    item.messageCursor, item.chunkCursor,
    item.outputClass, item.flushRequired,
    item.fifoOrdinal, item.routeTenure, "Queued",
    NoReplyPipelineTicket, NoReplyTicketTenure, {})

ReplyPipelineItemWithTicket(item, ticketId) ==
  ReplyPipelineRawItem(
    item.owner, item.semantic, item.source,
    item.messageCursor, item.chunkCursor,
    item.outputClass, item.flushRequired,
    item.fifoOrdinal, item.routeTenure, "Ticketed",
    ticketId, item.routeTenure,
    {ReplyPipelinePayloadForItem(item)})

ReplyPipelineItemWithRouteTenure(item, connectionTenure) ==
  ReplyPipelineRawItem(
    item.owner, item.semantic, item.source,
    item.messageCursor, item.chunkCursor,
    item.outputClass, item.flushRequired,
    item.fifoOrdinal, connectionTenure, item.phase,
    item.ticketId, item.ticketTenure, item.ticketPayload)

ReplyPipelineReplaceItem(oldItem, newItem) ==
  (rpItems \ {oldItem}) \cup {newItem}

ReplyPipelineItemsInLane(owner, source, outputClass) ==
  {item \in rpItems:
     /\ item.owner = owner
     /\ item.source = source
     /\ item.outputClass = outputClass}

ReplyPipelineItemIsFifoHead(item) ==
  \A other \in ReplyPipelineItemsInLane(
                 item.owner, item.source, item.outputClass):
    item.fifoOrdinal <= other.fifoOrdinal

ReplyPipelineHasUnresolvedWriter(owner, source) ==
  \E item \in rpItems:
    /\ item.owner = owner
    /\ item.source = source
    /\ item.phase \in {"Admitted", "Flushed"}

ReplyPipelineEverySourceAttemptHasRebind(owner, source) ==
  \A attempt \in rrAttempts:
    (attempt.owner = owner /\ attempt.source = source) =>
      ReplyRouteRebindPending(
        owner, attempt.semantic, source)

(***************************************************************************
A new physical reconnect cannot supersede a sibling Later capability while
that capability is still the only route by which the sibling's queued item
can become current.  Fair attachment first consumes such Later work; an
already-pending Reconnect remains valid across the common source teardown.
***************************************************************************)
ReplyPipelineReconnectObservationReady(owner, source) ==
  \A item \in rpItems:
    (item.owner = owner /\ item.source = source) =>
      LET attempt ==
            ReplyAttemptFor(item.owner, item.semantic, item.source)
      IN ReplyAttemptCurrent(attempt)
           \/ ReplyReconnectPending(
                item.owner, item.semantic, item.source)

ReplyPipelineItemsAfterReconnectObservation(owner, source) ==
  {IF item.owner = owner
        /\ item.source = source
        /\ item.phase = "Ticketed"
   THEN ReplyPipelineItemWithoutTicket(item)
   ELSE item: item \in rpItems}

ReplyPendingAfterReconnectObservation(owner, source, attachment) ==
  {pending \in rpPendingAttachments:
     \/ pending.owner # owner
     \/ pending.source # source
     \/ pending.kind = "Reconnect"}
    \cup {attachment}

ReplyPendingAttachmentAsLater(attachment) ==
  ReplyAttachment(
    attachment.owner, attachment.semantic,
    attachment.source, "Later")

ReplyPipelineItemsAfterRouteRebind(owner, semantic, source,
                                   connectionTenure) ==
  {IF item.owner = owner
        /\ item.semantic = semantic
        /\ item.source = source
        /\ item.phase = "Queued"
   THEN ReplyPipelineItemWithRouteTenure(item, connectionTenure)
   ELSE item: item \in rpItems}

ReplyPendingAfterReconnectAttach(selected) ==
  {IF attachment.owner = selected.owner
        /\ attachment.source = selected.source
        /\ attachment # selected
        /\ attachment.kind = "Reconnect"
   THEN ReplyPendingAttachmentAsLater(attachment)
   ELSE attachment:
     attachment \in rpPendingAttachments \ {selected}}

ReplyPipelineInit ==
  /\ ReplyRouteInit
  /\ ReplyPipelineConfiguration
  /\ rpPendingAttachments = {}
  /\ rpItems = {}
  /\ rpNextFifoOrdinal = [owner \in ReplyOwners |-> 1]
  /\ rpNextTicketId = [owner \in ReplyOwners |-> 1]

(***************************************************************************
Environment observation.  The opaque actor delivery ordinal is unobservable
outside the local capability, so the refinement safely linearizes its
allocation at fair attachment.  A bounded reservation prevents another
pending delivery from exhausting that ordinal first.
***************************************************************************)
ObserveAuthenticatedReplyDelivery(owner, semantic, source, kind) ==
  LET attachment == ReplyAttachment(owner, semantic, source, kind)
      consumesOrdinal == kind # "Exact"
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ kind \in ReplyAttachmentKinds
     /\ rrSourceActive[owner][source]
     /\ (kind # "Reconnect"
          \/ ReplyPipelineReconnectObservationReady(owner, source))
     /\ ~ReplyPendingAttachmentOwned(owner, semantic, source)
     /\ (kind = "Reconnect"
          \/ ~ReplyReconnectPendingForSource(owner, source))
     /\ IF kind = "New"
        THEN /\ ~ReplyAttemptOwned(owner, semantic, source)
        ELSE /\ ReplyAttemptOwned(owner, semantic, source)
             /\ IF kind = "Reconnect"
                THEN rrConnectionTenure[owner][source]
                       < ReplyDeliveryOrdinalLimit
                ELSE ReplyAttemptCurrent(
                       ReplyAttemptFor(owner, semantic, source))
     /\ (~consumesOrdinal
          \/ rrNextDeliveryOrdinal[owner]
               + ReplyOwnerOrdinalReservations(owner)
               <= ReplyDeliveryOrdinalLimit)
     /\ rpPendingAttachments' =
          IF kind = "Reconnect"
          THEN ReplyPendingAfterReconnectObservation(
                 owner, source, attachment)
          ELSE rpPendingAttachments \cup {attachment}
     /\ rpItems' =
          IF kind = "Reconnect"
          THEN ReplyPipelineItemsAfterReconnectObservation(owner, source)
          ELSE rpItems
     /\ UNCHANGED <<ReplyRouteVars,
                    rpNextFifoOrdinal, rpNextTicketId>>

RetirePendingReconnectSource(owner, source) ==
  /\ owner \in ReplyOwners
  /\ source \in ReplySources
  /\ ReplyReconnectPendingForSource(owner, source)
  /\ rrSourceActive[owner][source]
  /\ ~ReplyPipelineHasUnresolvedWriter(owner, source)
  /\ ReplyPipelineEverySourceAttemptHasRebind(owner, source)
  /\ RetireReplySource(owner, source)
  /\ UNCHANGED ReplyPipelineLocalVars

ReplyAttachmentRouteAction(owner, semantic, source, kind) ==
  CASE kind = "New" ->
         ObserveNewReplySource(owner, semantic, source)
    [] kind = "Exact" ->
         RetryExactReplySource(owner, semantic, source)
    [] kind = "Later" ->
         ObserveLaterReplyDelivery(owner, semantic, source)
    [] kind = "Reconnect" ->
         ReconnectReplySource(owner, semantic, source)

AttachPendingReplyDelivery(owner, semantic, source) ==
  LET attachment == ReplyPendingAttachmentFor(owner, semantic, source)
      kind == attachment.kind
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPendingAttachmentOwned(owner, semantic, source)
     /\ attachment \in ReplyAttachmentSet
     /\ kind \in ReplyAttachmentKinds
     /\ ReplyAttachmentRouteAction(owner, semantic, source, kind)
     /\ rpPendingAttachments' =
          IF kind = "Reconnect"
          THEN ReplyPendingAfterReconnectAttach(attachment)
          ELSE rpPendingAttachments \ {attachment}
     /\ rpItems' =
          IF kind \in {"Later", "Reconnect"}
          THEN ReplyPipelineItemsAfterRouteRebind(
                 owner, semantic, source,
                 rrConnectionTenure'[owner][source])
          ELSE rpItems
     /\ UNCHANGED <<rpNextFifoOrdinal, rpNextTicketId>>

EnqueueCurrentReplyItem(owner, semantic, source) ==
  LET attempt == ReplyAttemptFor(owner, semantic, source)
      fifoOrdinal == rpNextFifoOrdinal[owner]
      item ==
        ReplyPipelineItem(
          owner, semantic, source,
          attempt.messageCursor, attempt.chunkCursor,
          fifoOrdinal, attempt.connectionTenure,
          "Queued", NoReplyPipelineTicket,
          NoReplyTicketTenure, {})
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ ~ReplyPipelineItemOwned(owner, semantic, source)
     /\ ~ReplyReconnectPending(owner, semantic, source)
     /\ fifoOrdinal \in 1..ReplyPipelineOrdinalLimit
     /\ rpItems' = rpItems \cup {item}
     /\ rpNextFifoOrdinal' =
          [rpNextFifoOrdinal EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<ReplyRouteVars, rpPendingAttachments,
                    rpNextTicketId>>

AcquireReplyPipelineTicket(owner, semantic, source) ==
  LET item == ReplyPipelineItemFor(owner, semantic, source)
      attempt == ReplyAttemptFor(owner, semantic, source)
      ticketId == rpNextTicketId[owner]
      ticketed == ReplyPipelineItemWithTicket(item, ticketId)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPipelineItemOwned(owner, semantic, source)
     /\ ReplyPipelineQueuedItem(item)
     /\ ReplyPipelineItemIsFifoHead(item)
     /\ ReplyPipelineLiveCurrentCursor(
          owner, semantic, source, item, attempt)
     /\ ~ReplyReconnectPendingForSource(owner, source)
     /\ ticketId \in Nat \ {0}
     /\ rpItems' = ReplyPipelineReplaceItem(item, ticketed)
     /\ rpNextTicketId' =
          [rpNextTicketId EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<ReplyRouteVars, rpPendingAttachments,
                    rpNextFifoOrdinal>>

ReplyPipelineAdvanceAttempt(item) ==
  LET oldAttempt ==
        ReplyAttemptFor(item.owner, item.semantic, item.source)
  IN /\ ReplyAttemptOwned(item.owner, item.semantic, item.source)
     /\ ReplyAttemptCurrent(oldAttempt)
     /\ ~ReplyAttemptComplete(oldAttempt)
     /\ ReplyPipelineItemMatchesAttempt(item, oldAttempt)
     /\ AdvanceCurrentReplyAttempt(
          item.owner, item.semantic, item.source)

ReplyPipelineItemWithPhase(item, phase) ==
  ReplyPipelineRawItem(
    item.owner, item.semantic, item.source,
    item.messageCursor, item.chunkCursor,
    item.outputClass, item.flushRequired,
    item.fifoOrdinal, item.routeTenure,
    phase, item.ticketId, item.ticketTenure,
    item.ticketPayload)

ReplyPipelineFlushAdmission(item) ==
  /\ rpItems' =
       ReplyPipelineReplaceItem(
         item, ReplyPipelineItemWithPhase(item, "Admitted"))
  /\ UNCHANGED ReplyRouteVars

ReplyPipelineFlushedApplication(item) ==
  /\ ReplyPipelineAdvanceAttempt(item)
  /\ rpItems' = rpItems \ {item}

AdmitReplyPipelineItem(owner, semantic, source) ==
  LET item == ReplyPipelineItemFor(owner, semantic, source)
      attempt == ReplyAttemptFor(owner, semantic, source)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPipelineItemOwned(owner, semantic, source)
     /\ item.phase = "Ticketed"
     /\ ReplyPipelineTicketValid(item)
     /\ ReplyPipelineItemIsFifoHead(item)
     /\ ~ReplyReconnectPendingForSource(owner, source)
     /\ ReplyPipelineExactTicketAuthority(
          owner, semantic, source, item, attempt)
     /\ IF item.flushRequired
        THEN ReplyPipelineFlushAdmission(item)
        ELSE /\ ReplyPipelineAdvanceAttempt(item)
             /\ rpItems' = rpItems \ {item}
     /\ UNCHANGED <<rpPendingAttachments,
                    rpNextFifoOrdinal, rpNextTicketId>>

FlushAdmittedReplyPipelineItem(owner, semantic, source) ==
  LET item == ReplyPipelineItemFor(owner, semantic, source)
      attempt == ReplyAttemptFor(owner, semantic, source)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPipelineItemOwned(owner, semantic, source)
     /\ item.phase = "Admitted"
     /\ item.flushRequired
     /\ ReplyPipelineTicketValid(item)
     /\ ReplyPipelineItemIsFifoHead(item)
     /\ ReplyPipelineExactTicketAuthority(
          owner, semantic, source, item, attempt)
     /\ rpItems' =
          ReplyPipelineReplaceItem(
            item, ReplyPipelineItemWithPhase(item, "Flushed"))
     /\ UNCHANGED <<ReplyRouteVars, rpPendingAttachments,
                    rpNextFifoOrdinal, rpNextTicketId>>

CloseAdmittedReplyPipelineItem(owner, semantic, source) ==
  LET item == ReplyPipelineItemFor(owner, semantic, source)
      attempt == ReplyAttemptFor(owner, semantic, source)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPipelineItemOwned(owner, semantic, source)
     /\ item.phase = "Admitted"
     /\ item.flushRequired
     /\ ReplyPipelineTicketValid(item)
     /\ ReplyPipelineExactTicketAuthority(
          owner, semantic, source, item, attempt)
     /\ rpItems' =
          ReplyPipelineReplaceItem(
            item, ReplyPipelineItemWithoutTicket(item))
     /\ UNCHANGED <<ReplyRouteVars, rpPendingAttachments,
                    rpNextFifoOrdinal, rpNextTicketId>>

ApplyFlushedReplyPipelineItem(owner, semantic, source) ==
  LET item == ReplyPipelineItemFor(owner, semantic, source)
      attempt == ReplyAttemptFor(owner, semantic, source)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPipelineItemOwned(owner, semantic, source)
     /\ item.phase = "Flushed"
     /\ item.flushRequired
     /\ ReplyPipelineTicketValid(item)
     /\ ReplyPipelineItemIsFifoHead(item)
     /\ ReplyPipelineExactTicketAuthority(
          owner, semantic, source, item, attempt)
     /\ ReplyPipelineFlushedApplication(item)
     /\ UNCHANGED <<rpPendingAttachments,
                    rpNextFifoOrdinal, rpNextTicketId>>

FlushAdmittedReplyPipelineClassItem(
    owner, semantic, source, outputClass) ==
  /\ outputClass \in ReplyOutputClasses
  /\ IF ReplyPipelineItemOwned(owner, semantic, source)
     THEN /\ ReplyPipelineItemFor(
                  owner, semantic, source).outputClass = outputClass
          /\ FlushAdmittedReplyPipelineItem(owner, semantic, source)
     ELSE FALSE

(***************************************************************************
Writer responsiveness is a source/output-class property, not a property of
only the semantic request whose progress is being observed.  Strong fairness
excludes an older sibling repeatedly cycling Admit/Close/Queue while the
flush action is enabled infinitely often.
***************************************************************************)
ReplyPipelineResponsiveOutputClass(owner, source, outputClass) ==
  \A semantic \in ReplySemantics:
    SF_ReplyPipelineVars(
      FlushAdmittedReplyPipelineClassItem(
        owner, semantic, source, outputClass))

ReplyPipelineResponsiveSource(owner, source) ==
  \A outputClass \in ReplyOutputClasses:
    ReplyPipelineResponsiveOutputClass(
      owner, source, outputClass)

ReplyPipelineNext ==
  /\ ReplyRouteSafetyInvariant
  /\ (\/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources, kind \in ReplyAttachmentKinds:
          ObserveAuthenticatedReplyDelivery(
            owner, semantic, source, kind)
      \/ \E owner \in ReplyOwners, source \in ReplySources:
          RetirePendingReconnectSource(owner, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          AttachPendingReplyDelivery(owner, semantic, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          EnqueueCurrentReplyItem(owner, semantic, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          AcquireReplyPipelineTicket(owner, semantic, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          AdmitReplyPipelineItem(owner, semantic, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          FlushAdmittedReplyPipelineItem(owner, semantic, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          CloseAdmittedReplyPipelineItem(owner, semantic, source)
      \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
          ApplyFlushedReplyPipelineItem(owner, semantic, source))

(***************************************************************************
Exact route projection of every pipeline transition.  Local queue/ticket/
writer transitions stutter the route carrier.  Attachment uses only the five
route ownership actions below, while ordinary admission and flushed
application share `AdvanceCurrentReplyAttempt`.  Keeping this projection in
the production model makes the temporal replay and source-isolation proof a
call-path obligation rather than an imported alias of `ReplyRouteNext`.
***************************************************************************)
ReplyPipelineRouteStep ==
  \/ UNCHANGED ReplyRouteVars
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveNewReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       RetryExactReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveLaterReplyDelivery(owner, semantic, source)
  \/ \E owner \in ReplyOwners, source \in ReplySources:
       RetireReplySource(owner, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ReconnectReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       AdvanceCurrentReplyAttempt(owner, semantic, source)

ReplyPipelineFairness ==
  /\ \A owner \in ReplyOwners, source \in ReplySources:
       WF_ReplyPipelineVars(
         RetirePendingReconnectSource(owner, source))
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       /\ WF_ReplyPipelineVars(
            AttachPendingReplyDelivery(owner, semantic, source))
       /\ WF_ReplyPipelineVars(
            EnqueueCurrentReplyItem(owner, semantic, source))
       /\ WF_ReplyPipelineVars(
            AcquireReplyPipelineTicket(owner, semantic, source))
       /\ WF_ReplyPipelineVars(
            AdmitReplyPipelineItem(owner, semantic, source))
       /\ WF_ReplyPipelineVars(
            \/ FlushAdmittedReplyPipelineItem(
                 owner, semantic, source)
            \/ CloseAdmittedReplyPipelineItem(
                 owner, semantic, source))
       /\ WF_ReplyPipelineVars(
            ApplyFlushedReplyPipelineItem(owner, semantic, source))

ReplyPipelineSpec ==
  ReplyPipelineInit
    /\ [][ReplyPipelineNext]_ReplyPipelineVars
    /\ ReplyPipelineFairness

ReplyPipelineTypeInvariant ==
  /\ rpPendingAttachments \subseteq ReplyAttachmentSet
  /\ \A item \in rpItems: ReplyPipelineItemHasType(item)
  /\ rpNextFifoOrdinal
       \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
  /\ rpNextTicketId
       \in [ReplyOwners -> Nat \ {0}]

ReplyPipelinePendingPerIdentityInvariant ==
  \A left, right \in rpPendingAttachments:
    /\ left.owner = right.owner
    /\ left.semantic = right.semantic
    /\ left.source = right.source
    => left = right

ReplyPipelineItemPerIdentityInvariant ==
  \A left, right \in rpItems:
    /\ left.owner = right.owner
    /\ left.semantic = right.semantic
    /\ left.source = right.source
    => left = right

ReplyPipelinePerIdentityInvariant ==
  /\ ReplyPipelinePendingPerIdentityInvariant
  /\ ReplyPipelineItemPerIdentityInvariant

ReplyPipelineFifoOrdinalInvariant ==
  /\ \A left, right \in rpItems:
       /\ left.owner = right.owner
       /\ left.fifoOrdinal = right.fifoOrdinal
       => left = right
  /\ \A item \in rpItems:
       item.fifoOrdinal < rpNextFifoOrdinal[item.owner]

ReplyPipelineTicketIdentityInvariant ==
  \A left, right \in rpItems:
    /\ left.owner = right.owner
    /\ left.ticketId # NoReplyPipelineTicket
    /\ left.ticketId = right.ticketId
    => left = right

ReplyPipelineItemCoreBinding(item) ==
  LET attempt ==
        ReplyAttemptFor(item.owner, item.semantic, item.source)
  IN /\ ReplyAttemptOwned(item.owner, item.semantic, item.source)
     /\ ReplyPipelineItemMatchesAttempt(item, attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ item.outputClass =
          ReplyItemClass(
            item.semantic, item.messageCursor, item.chunkCursor)
     /\ item.flushRequired =
          ReplyItemRequiresFlush(
            item.semantic, item.messageCursor, item.chunkCursor)

ReplyPipelineItemRouteBinding(item) ==
  LET attempt ==
        ReplyAttemptFor(item.owner, item.semantic, item.source)
  IN ReplyAttemptCurrent(attempt)
       \/ ReplyRouteRebindPending(
            item.owner, item.semantic, item.source)

ReplyPipelineItemPhaseBinding(item) ==
  IF item.phase = "Queued"
  THEN ReplyPipelineQueuedItem(item)
  ELSE /\ ReplyPipelineTicketValid(item)
       /\ ReplyPipelineItemIsFifoHead(item)
       /\ (item.phase \in {"Admitted", "Flushed"}
            => item.flushRequired)

ReplyPipelineItemBindingInvariant ==
  \A item \in rpItems:
    /\ ReplyPipelineItemCoreBinding(item)
    /\ ReplyPipelineItemRouteBinding(item)
    /\ ReplyPipelineItemPhaseBinding(item)

ReplyPipelineReconnectNoTicketedInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    ReplyReconnectPendingForSource(owner, source) =>
      \A item \in rpItems:
        (item.owner = owner /\ item.source = source) =>
          item.phase # "Ticketed"

ReplyPipelineReconnectWriterActiveInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyReconnectPendingForSource(owner, source)
    /\ ReplyPipelineHasUnresolvedWriter(owner, source)
    => rrSourceActive[owner][source]

ReplyPipelineReconnectBarrierInvariant ==
  /\ ReplyPipelineReconnectNoTicketedInvariant
  /\ ReplyPipelineReconnectWriterActiveInvariant

ReplyPipelineOwnershipInvariant ==
  /\ ReplyPipelinePerIdentityInvariant
  /\ ReplyPipelineFifoOrdinalInvariant
  /\ ReplyPipelineTicketIdentityInvariant
  /\ ReplyPipelineItemBindingInvariant
  /\ ReplyPipelineReconnectBarrierInvariant

ReplyPipelineSafetyInvariant ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplyPipelineTypeInvariant
  /\ ReplyPipelineOwnershipInvariant

ReplyPendingAttachmentEventuallyConsumed(owner, semantic, source) ==
  ReplyPendingAttachmentOwned(owner, semantic, source)
    ~> ~ReplyPendingAttachmentOwned(owner, semantic, source)

ReplyPipelineItemEventuallyAdvances(owner, semantic, source) ==
  \A messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    (/\ ReplySourceRouteStable(owner, semantic, source)
     /\ ReplySourceAtCursor(
          owner, semantic, source, messageCursor, chunkCursor)
     /\ ~ReplyReconnectPendingForSource(owner, source))
      ~> ReplySourceAdvancedFrom(
           owner, semantic, source, messageCursor, chunkCursor)

=============================================================================
