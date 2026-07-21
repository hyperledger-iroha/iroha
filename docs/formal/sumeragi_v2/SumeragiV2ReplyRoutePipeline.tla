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

ReplyPipelinePayloads ==
  {ReplyPipelinePayload(semantic, messageCursor, chunkCursor):
     semantic \in ReplySemantics,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount}

ReplyPipelineItem(owner, semantic, source, messageCursor, chunkCursor,
                  fifoOrdinal, routeTenure, phase, ticketId,
                  ticketTenure, ticketPayload) ==
  [owner |-> owner, semantic |-> semantic, source |-> source,
   messageCursor |-> messageCursor, chunkCursor |-> chunkCursor,
   outputClass |-> ReplyItemClass(
                       semantic, messageCursor, chunkCursor),
   flushRequired |-> ReplyItemRequiresFlush(
                        semantic, messageCursor, chunkCursor),
   fifoOrdinal |-> fifoOrdinal, routeTenure |-> routeTenure,
   phase |-> phase, ticketId |-> ticketId,
   ticketTenure |-> ticketTenure, ticketPayload |-> ticketPayload]

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

ReplyPipelineItemWithoutTicket(item) ==
  [item EXCEPT
     !.phase = "Queued",
     !.ticketId = NoReplyPipelineTicket,
     !.ticketTenure = NoReplyTicketTenure,
     !.ticketPayload = {}]

ReplyPipelineItemWithTicket(item, ticketId) ==
  [item EXCEPT
     !.phase = "Ticketed",
     !.ticketId = ticketId,
     !.ticketTenure = item.routeTenure,
     !.ticketPayload = {ReplyPipelinePayloadForItem(item)}]

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

ReplyPipelineItemsAfterRouteRebind(owner, semantic, source,
                                   connectionTenure) ==
  {IF item.owner = owner
        /\ item.semantic = semantic
        /\ item.source = source
        /\ item.phase = "Queued"
   THEN [item EXCEPT !.routeTenure = connectionTenure]
   ELSE item: item \in rpItems}

ReplyPendingAfterReconnectAttach(selected) ==
  {IF attachment.owner = selected.owner
        /\ attachment.source = selected.source
        /\ attachment # selected
        /\ attachment.kind = "Reconnect"
   THEN [attachment EXCEPT !.kind = "Later"]
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
     /\ ~ReplyPendingAttachmentOwned(owner, semantic, source)
     /\ (kind = "Reconnect"
          \/ ~ReplyReconnectPendingForSource(owner, source))
     /\ IF kind = "New"
        THEN /\ ~ReplyAttemptOwned(owner, semantic, source)
             /\ rrSourceActive[owner][source]
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

AttachPendingReplyDelivery(owner, semantic, source) ==
  LET attachment == ReplyPendingAttachmentFor(owner, semantic, source)
      kind == attachment.kind
      attachAction ==
        CASE kind = "New" ->
               ObserveNewReplySource(owner, semantic, source)
          [] kind = "Exact" ->
               RetryExactReplySource(owner, semantic, source)
          [] kind = "Later" ->
               ObserveLaterReplyDelivery(owner, semantic, source)
          [] kind = "Reconnect" ->
               ReconnectReplySource(owner, semantic, source)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyPendingAttachmentOwned(owner, semantic, source)
     /\ attachAction
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
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ ReplyPipelineItemMatchesAttempt(item, attempt)
     /\ item.routeTenure = attempt.connectionTenure
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
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ ReplyPipelineItemMatchesAttempt(item, attempt)
     /\ item.routeTenure = attempt.connectionTenure
     /\ IF item.flushRequired
        THEN /\ rpItems' =
                   ReplyPipelineReplaceItem(
                     item, [item EXCEPT !.phase = "Admitted"])
             /\ UNCHANGED ReplyRouteVars
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
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ ReplyPipelineItemMatchesAttempt(item, attempt)
     /\ item.routeTenure = attempt.connectionTenure
     /\ rpItems' =
          ReplyPipelineReplaceItem(
            item, [item EXCEPT !.phase = "Flushed"])
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
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ ReplyPipelineItemMatchesAttempt(item, attempt)
     /\ item.routeTenure = attempt.connectionTenure
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
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(attempt)
     /\ ~ReplyAttemptComplete(attempt)
     /\ ReplyPipelineItemMatchesAttempt(item, attempt)
     /\ item.routeTenure = attempt.connectionTenure
     /\ ReplyPipelineAdvanceAttempt(item)
     /\ rpItems' = rpItems \ {item}
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
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources, kind \in ReplyAttachmentKinds:
       ObserveAuthenticatedReplyDelivery(owner, semantic, source, kind)
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
       ApplyFlushedReplyPipelineItem(owner, semantic, source)

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
  /\ rpItems \subseteq ReplyPipelineItemSet
  /\ rpNextFifoOrdinal
       \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
  /\ rpNextTicketId
       \in [ReplyOwners -> Nat \ {0}]

ReplyPipelineOwnershipInvariant ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       /\ Cardinality(
            ReplyPendingAttachmentsFor(owner, semantic, source)) <= 1
       /\ Cardinality(
            ReplyPipelineItemsFor(owner, semantic, source)) <= 1
  /\ \A left, right \in rpItems:
       /\ left.owner = right.owner
       /\ left.fifoOrdinal = right.fifoOrdinal
       => left = right
  /\ \A left, right \in rpItems:
       /\ left.owner = right.owner
       /\ left.ticketId # NoReplyPipelineTicket
       /\ left.ticketId = right.ticketId
       => left = right
  /\ \A item \in rpItems:
       LET attempt ==
             ReplyAttemptFor(item.owner, item.semantic, item.source)
       IN /\ ReplyAttemptOwned(
                item.owner, item.semantic, item.source)
          /\ ReplyPipelineItemMatchesAttempt(item, attempt)
          /\ ~ReplyAttemptComplete(attempt)
          /\ item.outputClass =
               ReplyItemClass(
                 item.semantic, item.messageCursor, item.chunkCursor)
          /\ item.flushRequired =
               ReplyItemRequiresFlush(
                 item.semantic, item.messageCursor, item.chunkCursor)
          /\ (ReplyAttemptCurrent(attempt)
               \/ ReplyRouteRebindPending(
                    item.owner, item.semantic, item.source))
          /\ IF item.phase = "Queued"
             THEN ReplyPipelineQueuedItem(item)
             ELSE /\ ReplyPipelineTicketValid(item)
                  /\ ReplyPipelineItemIsFifoHead(item)
                  /\ (item.phase \in {"Admitted", "Flushed"}
                       => item.flushRequired)
  /\ \A owner \in ReplyOwners, source \in ReplySources:
       ReplyReconnectPendingForSource(owner, source) =>
         /\ \A item \in rpItems:
              (item.owner = owner /\ item.source = source) =>
                item.phase # "Ticketed"
         /\ (ReplyPipelineHasUnresolvedWriter(owner, source) =>
               rrSourceActive[owner][source])

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
