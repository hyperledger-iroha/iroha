---- MODULE SumeragiV2ReplyRoutePipelineProofs ----
EXTENDS SumeragiV2ReplyRoutePipeline,
        SumeragiV2ReplyRouteOwnershipProofs,
        SumeragiV2TemporalLemmas,
        FiniteSetTheorems, TLAPS

(***************************************************************************
Deductive boundary for authenticated reply attachment and exact-output FIFO.

The action leaves below separate three facts which production must preserve:
an observed capability becomes durable local work, a ticket is issued only to
the oldest item in its authenticated source/output-class lane, and a
flush-required cursor advances only after its exact flushed item is applied.
Reconnect observation invalidates queued tickets source-wide but leaves an
already admitted old writer as the sole terminal owner until it flushes or
closes.
***************************************************************************)

ReplyPipelineInductiveInvariant ==
  /\ ReplyPipelineConfiguration
  /\ ReplyRouteInductiveInvariant
  /\ ReplyPipelineTypeInvariant
  /\ ReplyPipelineOwnershipInvariant

THEOREM ReplyCurrentAttemptEstablishesRouteBindingPrime ==
  \A item:
    ReplyAttemptCurrent(
      ReplyAttemptFor(
        item.owner, item.semantic, item.source))'
    => ReplyPipelineItemRouteBinding(item)'
BY SMTT(10)
   DEF ReplyPipelineItemRouteBinding

THEOREM ReplyQueuedItemEstablishesPhaseBindingPrime ==
  \A item:
    ReplyPipelineQueuedItem(item)'
    => ReplyPipelineItemPhaseBinding(item)'
BY SMTT(10)
   DEF ReplyPipelineItemPhaseBinding,
       ReplyPipelineQueuedItem

THEOREM ReplyPipelineItemWithoutTicketHasType ==
  ReplyPipelineConfiguration =>
    \A item:
      ReplyPipelineItemHasType(item) =>
        ReplyPipelineItemHasType(
          ReplyPipelineItemWithoutTicket(item))
PROOF
  <1>1. ASSUME ReplyPipelineConfiguration,
                NEW item,
                ReplyPipelineItemHasType(item)
         PROVE ReplyPipelineItemHasType(
                 ReplyPipelineItemWithoutTicket(item))
    <2>1. /\ item.owner \in ReplyOwners
           /\ item.semantic \in ReplySemantics
           /\ item.source \in ReplySources
           /\ item.messageCursor \in 0..ReplyMessageCount
           /\ item.chunkCursor \in 0..ReplyChunkCount
           /\ item.outputClass \in ReplyOutputClasses
           /\ item.flushRequired \in BOOLEAN
           /\ item.fifoOrdinal \in 1..ReplyPipelineOrdinalLimit
           /\ item.routeTenure \in ReplyConnectionTenures
      BY <1>1 DEF ReplyPipelineItemHasType
    <2>2. LET queued == ReplyPipelineItemWithoutTicket(item)
           IN /\ queued.owner = item.owner
              /\ queued.semantic = item.semantic
              /\ queued.source = item.source
              /\ queued.messageCursor = item.messageCursor
              /\ queued.chunkCursor = item.chunkCursor
              /\ queued.outputClass = item.outputClass
              /\ queued.flushRequired = item.flushRequired
              /\ queued.fifoOrdinal = item.fifoOrdinal
              /\ queued.routeTenure = item.routeTenure
              /\ queued.phase = "Queued"
              /\ queued.ticketId = NoReplyPipelineTicket
              /\ queued.ticketTenure = NoReplyTicketTenure
              /\ queued.ticketPayload = {}
      BY SMTT(10)
         DEF ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem
    <2>3. "Queued" \in ReplyPipelinePhases
      BY SMTT(5) DEF ReplyPipelinePhases
    <2>4. NoReplyPipelineTicket \in Nat
      BY SMTT(5) DEF NoReplyPipelineTicket
    <2>5. NoReplyTicketTenure \in 0..ReplyDeliveryOrdinalLimit
      BY <1>1, SMTT(5)
         DEF NoReplyTicketTenure, ReplyPipelineConfiguration,
             ReplyRouteConfiguration
    <2>6. {} \in SUBSET ReplyPipelinePayloads
      BY SMTT(5)
    <2> QED BY ONLY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                       SMTT(10)
         DEF ReplyPipelineItemWithoutTicket,
             ReplyPipelineItemHasType, ReplyPipelineRawItem,
             ReplyPipelinePhases,
             NoReplyPipelineTicket, NoReplyTicketTenure,
             ReplyPipelineConfiguration
  <1> QED BY <1>1

THEOREM ReplyPipelineItemWithoutTicketPreservesCoreBinding ==
  \A item:
    ReplyPipelineItemCoreBinding(item) =>
      ReplyPipelineItemCoreBinding(
        ReplyPipelineItemWithoutTicket(item))
PROOF
  <1>1. ASSUME NEW item,
                ReplyPipelineItemCoreBinding(item)
         PROVE ReplyPipelineItemCoreBinding(
                 ReplyPipelineItemWithoutTicket(item))
    <2>1. LET queued == ReplyPipelineItemWithoutTicket(item)
           IN /\ queued.owner = item.owner
              /\ queued.semantic = item.semantic
              /\ queued.source = item.source
              /\ queued.messageCursor = item.messageCursor
              /\ queued.chunkCursor = item.chunkCursor
              /\ queued.outputClass = item.outputClass
              /\ queued.flushRequired = item.flushRequired
      BY SMTT(10)
         DEF ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem
    <2> QED BY <1>1, <2>1, SMTT(10)
         DEF ReplyPipelineItemCoreBinding,
             ReplyPipelineItemMatchesAttempt
  <1> QED BY <1>1

THEOREM ReplyRouteStutterPreservesItemCoreBinding ==
  \A item:
    /\ ReplyPipelineItemCoreBinding(item)
    /\ UNCHANGED ReplyRouteVars
    => ReplyPipelineItemCoreBinding(item)'
PROOF
  <1>1. ASSUME NEW item,
                ReplyPipelineItemCoreBinding(item),
                UNCHANGED ReplyRouteVars
         PROVE ReplyPipelineItemCoreBinding(item)'
    <2>1. rrAttempts' = rrAttempts
      BY <1>1, SMTT(5) DEF ReplyRouteVars
    <2>2. ReplyAttemptsForSource(
             item.owner, item.semantic, item.source)' =
           ReplyAttemptsForSource(
             item.owner, item.semantic, item.source)
      BY <2>1, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3. ReplyAttemptFor(
             item.owner, item.semantic, item.source)' =
           ReplyAttemptFor(item.owner, item.semantic, item.source)
      BY <2>2 DEF ReplyAttemptFor
    <2>4. ReplyAttemptOwned(
             item.owner, item.semantic, item.source)' <=>
           ReplyAttemptOwned(item.owner, item.semantic, item.source)
      BY <2>2 DEF ReplyAttemptOwned
    <2> QED BY <1>1, <2>3, <2>4, SMTT(10)
         DEF ReplyPipelineItemCoreBinding,
             ReplyPipelineItemMatchesAttempt
  <1> QED BY <1>1

THEOREM ReplyRouteStutterPreservesWithoutTicketCoreBinding ==
  \A item:
    /\ ReplyPipelineItemCoreBinding(item)
    /\ UNCHANGED ReplyRouteVars
    => ReplyPipelineItemCoreBinding(
         ReplyPipelineItemWithoutTicket(item))'
BY ReplyPipelineItemWithoutTicketPreservesCoreBinding,
   ReplyRouteStutterPreservesItemCoreBinding, SMTT(10)

THEOREM ReplyRouteStutterPreservesAttemptCurrentView ==
  \A owner, semantic, source:
    UNCHANGED ReplyRouteVars =>
      (ReplyAttemptCurrent(
         ReplyAttemptFor(owner, semantic, source))' <=>
       ReplyAttemptCurrent(
         ReplyAttemptFor(owner, semantic, source)))
PROOF
  <1>1. ASSUME NEW owner, NEW semantic, NEW source,
                UNCHANGED ReplyRouteVars
         PROVE ReplyAttemptCurrent(
                   ReplyAttemptFor(owner, semantic, source))' <=>
                 ReplyAttemptCurrent(
                   ReplyAttemptFor(owner, semantic, source))
    <2>1. /\ rrAttempts' = rrAttempts
           /\ rrConnectionTenure' = rrConnectionTenure
           /\ rrSourceActive' = rrSourceActive
      BY <1>1, SMTT(5) DEF ReplyRouteVars
    <2>2. ReplyAttemptsForSource(owner, semantic, source)' =
           ReplyAttemptsForSource(owner, semantic, source)
      BY <2>1, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3. ReplyAttemptFor(owner, semantic, source)' =
           ReplyAttemptFor(owner, semantic, source)
      BY <2>2 DEF ReplyAttemptFor
    <2> QED BY <2>1, <2>3, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyPipelineItemWithoutTicketPreservesRouteBinding ==
  \A item:
    ReplyPipelineItemRouteBinding(
      ReplyPipelineItemWithoutTicket(item)) <=>
      ReplyPipelineItemRouteBinding(item)
BY SMTT(10)
   DEF ReplyPipelineItemRouteBinding,
       ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem

THEOREM ReplyPipelineItemWithoutTicketPreservesRouteBindingPrime ==
  \A item:
    ReplyPipelineItemRouteBinding(
      ReplyPipelineItemWithoutTicket(item))' <=>
      ReplyPipelineItemRouteBinding(item)'
BY SMTT(10)
   DEF ReplyPipelineItemRouteBinding,
       ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem

THEOREM ReplyPipelineInitEstablishesInvariant ==
  ReplyPipelineInit => ReplyPipelineInductiveInvariant
PROOF
  <1>1. ReplyPipelineInit => ReplyRouteInductiveInvariant
    BY ReplyRouteInitEstablishesInductiveInvariant
       DEF ReplyPipelineInit
  <1>2. ReplyPipelineInit => ReplyPipelineConfiguration
    BY SMTT(5) DEF ReplyPipelineInit
  <1>3. ReplyPipelineInit => ReplyPipelineTypeInvariant
    BY SMTT(30)
       DEF ReplyPipelineInit, ReplyPipelineTypeInvariant,
           ReplyPipelineConfiguration, ReplyPipelineOrdinalLimit
  <1>4. ReplyPipelineInit => ReplyPipelinePerIdentityInvariant
    BY FS_EmptySet, SMTT(10)
       DEF ReplyPipelineInit, ReplyPipelinePerIdentityInvariant,
           ReplyPipelinePendingPerIdentityInvariant,
           ReplyPipelineItemPerIdentityInvariant
  <1>5. ReplyPipelineInit => ReplyPipelineFifoOrdinalInvariant
    BY SMTT(5)
       DEF ReplyPipelineInit, ReplyPipelineFifoOrdinalInvariant
  <1>6. ReplyPipelineInit => ReplyPipelineTicketIdentityInvariant
    BY SMTT(5)
       DEF ReplyPipelineInit, ReplyPipelineTicketIdentityInvariant
  <1>7. ReplyPipelineInit => ReplyPipelineItemBindingInvariant
    BY SMTT(5)
       DEF ReplyPipelineInit, ReplyPipelineItemBindingInvariant
  <1>8. ReplyPipelineInit => ReplyPipelineReconnectBarrierInvariant
    BY SMTT(10)
       DEF ReplyPipelineInit, ReplyPipelineReconnectBarrierInvariant,
           ReplyPipelineReconnectNoTicketedInvariant,
           ReplyPipelineReconnectWriterActiveInvariant,
           ReplyPipelineHasUnresolvedWriter,
           ReplyReconnectPendingForSource,
           ReplyPendingAttachmentsFor
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7, <1>8
       DEF ReplyPipelineOwnershipInvariant,
           ReplyPipelineInductiveInvariant

THEOREM ReplyObservedDeliveryPreservesTypeInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineConfiguration
    /\ ReplyPipelineTypeInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineTypeInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineConfiguration,
                ReplyPipelineTypeInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE ReplyPipelineTypeInvariant'
    <2>1. rpPendingAttachments' \subseteq ReplyAttachmentSet
      BY <1>1, SMTT(60)
         DEF ReplyPipelineTypeInvariant,
             ObserveAuthenticatedReplyDelivery,
             ReplyPendingAfterReconnectObservation,
             ReplyAttachment, ReplyAttachmentSet
    <2>2. \A item \in rpItems': ReplyPipelineItemHasType(item)
      BY <1>1, ReplyPipelineItemWithoutTicketHasType,
         SMTT(30)
         DEF ReplyPipelineTypeInvariant,
             ObserveAuthenticatedReplyDelivery,
             ReplyPipelineItemsAfterReconnectObservation,
             ReplyPipelineItemWithoutTicket,
             ReplyPipelineItemHasType
    <2>3. rpNextFifoOrdinal'
             \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
      BY <1>1, SMTT(5)
         DEF ReplyPipelineTypeInvariant,
             ObserveAuthenticatedReplyDelivery
    <2>4. rpNextTicketId' \in [ReplyOwners -> Nat \ {0}]
      BY <1>1, SMTT(5)
         DEF ReplyPipelineTypeInvariant,
             ObserveAuthenticatedReplyDelivery
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyPipelineTypeInvariant
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesPerIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineTypeInvariant
    /\ ReplyPipelinePerIdentityInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelinePerIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineTypeInvariant,
                ReplyPipelinePerIdentityInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE ReplyPipelinePerIdentityInvariant'
    <2>1. ReplyPipelinePendingPerIdentityInvariant'
      BY <1>1, SMTT(60)
         DEF ReplyPipelinePerIdentityInvariant,
             ReplyPipelinePendingPerIdentityInvariant,
             ObserveAuthenticatedReplyDelivery,
             ReplyPendingAfterReconnectObservation,
             ReplyPendingAttachmentOwned,
             ReplyPendingAttachmentsFor,
             ReplyAttachment
    <2>2. ReplyPipelineItemPerIdentityInvariant'
      BY <1>1, SMTT(60)
         DEF ReplyPipelinePerIdentityInvariant,
             ReplyPipelineItemPerIdentityInvariant,
             ObserveAuthenticatedReplyDelivery,
             ReplyPipelineItemsAfterReconnectObservation,
             ReplyPipelineItemWithoutTicket,
             ReplyPipelineRawItem
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelinePerIdentityInvariant
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesFifoOrdinalInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineFifoOrdinalInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineFifoOrdinalInvariant'
BY SMTT(60)
   DEF ReplyPipelineFifoOrdinalInvariant,
       ObserveAuthenticatedReplyDelivery,
       ReplyPipelineItemsAfterReconnectObservation,
       ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem

THEOREM ReplyObservedDeliveryPreservesTicketIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineTicketIdentityInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineTicketIdentityInvariant'
BY SMTT(60)
   DEF ReplyPipelineTicketIdentityInvariant,
       ObserveAuthenticatedReplyDelivery,
       ReplyPipelineItemsAfterReconnectObservation,
       ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem,
       NoReplyPipelineTicket

THEOREM ReplyObservedDeliveryPreservesItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineItemBindingInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => \A item \in rpItems': ReplyPipelineItemCoreBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineItemBindingInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE \A item \in rpItems':
                 ReplyPipelineItemCoreBinding(item)'
    <2>1. UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(5) DEF ObserveAuthenticatedReplyDelivery
    <2>2. CASE kind # "Reconnect"
      BY <1>1, <2>1, <2>2,
         ReplyRouteStutterPreservesItemCoreBinding, SMTT(20)
         DEF ReplyPipelineItemBindingInvariant,
             ObserveAuthenticatedReplyDelivery
    <2>3. CASE kind = "Reconnect"
      BY <1>1, <2>1, <2>3,
         ReplyRouteStutterPreservesItemCoreBinding,
         ReplyRouteStutterPreservesWithoutTicketCoreBinding,
         SMTT(30)
         DEF ReplyPipelineItemBindingInvariant,
             ObserveAuthenticatedReplyDelivery,
             ReplyPipelineItemsAfterReconnectObservation
    <2> QED BY <2>2, <2>3, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds,
     item \in rpItems:
    /\ ReplyPipelineItemRouteBinding(item)
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                NEW item \in rpItems,
                ReplyPipelineItemRouteBinding(item),
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(5) DEF ObserveAuthenticatedReplyDelivery
    <2>2. ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))' <=>
           ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))
      BY <2>1, ReplyRouteStutterPreservesAttemptCurrentView
    <2>3. CASE kind # "Reconnect"
      <3>1. rpPendingAttachments' =
              rpPendingAttachments \cup
                {ReplyAttachment(owner, semantic, source, kind)}
        BY <1>1, <2>3, SMTT(5)
           DEF ObserveAuthenticatedReplyDelivery
      <3> QED BY <1>1, <2>2, <3>1, SMTT(20)
           DEF ReplyPipelineItemRouteBinding,
               ReplyRouteRebindPending,
               ReplyPendingAttachmentsFor
    <2>4. CASE kind = "Reconnect"
      <3>1. /\ ReplyPipelineReconnectObservationReady(owner, source)
             /\ rpPendingAttachments' =
                  ReplyPendingAfterReconnectObservation(
                    owner, source,
                    ReplyAttachment(owner, semantic, source, kind))
        BY <1>1, <2>4, SMTT(5)
           DEF ObserveAuthenticatedReplyDelivery
      <3> QED BY <1>1, <2>2, <3>1, SMTT(40)
           DEF ReplyPipelineItemRouteBinding,
               ReplyPipelineReconnectObservationReady,
               ReplyPendingAfterReconnectObservation,
               ReplyRouteRebindPending, ReplyReconnectPending,
               ReplyPendingAttachmentsFor
    <2> QED BY <2>3, <2>4, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineItemBindingInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => \A item \in rpItems': ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineItemBindingInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE \A item \in rpItems':
                 ReplyPipelineItemRouteBinding(item)'
    <2>1. \A item \in rpItems:
             ReplyPipelineItemRouteBinding(item)'
      BY <1>1,
         ReplyObservedDeliveryPreservesExistingItemRouteBinding,
         SMTT(10)
         DEF ReplyPipelineItemBindingInvariant
    <2>2. CASE kind # "Reconnect"
      BY <1>1, <2>1, <2>2, SMTT(10)
         DEF ObserveAuthenticatedReplyDelivery
    <2>3. CASE kind = "Reconnect"
      BY <1>1, <2>1, <2>3,
         ReplyPipelineItemWithoutTicketPreservesRouteBindingPrime,
         SMTT(20)
         DEF ObserveAuthenticatedReplyDelivery,
             ReplyPipelineItemsAfterReconnectObservation
    <2> QED BY <2>2, <2>3, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesItemPhaseBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineItemBindingInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => \A item \in rpItems': ReplyPipelineItemPhaseBinding(item)'
BY SMTT(60)
   DEF ReplyPipelineItemBindingInvariant,
       ReplyPipelineItemPhaseBinding,
       ObserveAuthenticatedReplyDelivery,
       ReplyPipelineItemsAfterReconnectObservation,
       ReplyPipelineItemWithoutTicket, ReplyPipelineRawItem,
       ReplyPipelineQueuedItem, ReplyPipelineTicketValid,
       ReplyPipelineItemIsFifoHead, ReplyPipelineItemsInLane

THEOREM ReplyObservedDeliveryPreservesItemBindingInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineItemBindingInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineItemBindingInvariant'
BY ReplyObservedDeliveryPreservesItemCoreBinding,
   ReplyObservedDeliveryPreservesItemRouteBinding,
   ReplyObservedDeliveryPreservesItemPhaseBinding, SMTT(10)
   DEF ReplyPipelineItemBindingInvariant

THEOREM ReplyObservedDeliveryPreservesReconnectNoTicketedInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineReconnectNoTicketedInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineReconnectNoTicketedInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineReconnectNoTicketedInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE ReplyPipelineReconnectNoTicketedInvariant'
    <2>1. CASE kind # "Reconnect"
      <3>1. ASSUME NEW barrierOwner \in ReplyOwners,
                    NEW barrierSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      barrierOwner, barrierSource)'
             PROVE \A item \in rpItems':
                     (item.owner = barrierOwner
                       /\ item.source = barrierSource) =>
                       item.phase # "Ticketed"
        <4>1. /\ rpItems' = rpItems
               /\ rpPendingAttachments' =
                    rpPendingAttachments \cup
                      {ReplyAttachment(
                         owner, semantic, source, kind)}
          BY <1>1, <2>1, SMTT(5)
             DEF ObserveAuthenticatedReplyDelivery
        <4>2. ReplyReconnectPendingForSource(
                 barrierOwner, barrierSource)
          BY <1>1, <2>1, <3>1, <4>1, SMTT(20)
             DEF ReplyPendingAttachmentsFor,
                 ReplyReconnectPending,
                 ReplyReconnectPendingForSource,
                 ReplyAttachment
        <4>3. \A item \in rpItems:
                 (item.owner = barrierOwner
                   /\ item.source = barrierSource) =>
                   item.phase # "Ticketed"
          BY <1>1, <4>2, SMTT(10)
             DEF ReplyPipelineReconnectNoTicketedInvariant
        <4> QED BY <4>1, <4>3, SMTT(10)
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectNoTicketedInvariant
    <2>2. CASE kind = "Reconnect"
      <3>1. ASSUME NEW barrierOwner \in ReplyOwners,
                    NEW barrierSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      barrierOwner, barrierSource)'
             PROVE \A item \in rpItems':
                     (item.owner = barrierOwner
                       /\ item.source = barrierSource) =>
                       item.phase # "Ticketed"
        <4>1. CASE barrierOwner = owner
                    /\ barrierSource = source
          BY <1>1, <2>2, <3>1, <4>1, SMTT(30)
             DEF ObserveAuthenticatedReplyDelivery,
                 ReplyPipelineItemsAfterReconnectObservation,
                 ReplyPipelineItemWithoutTicket,
                 ReplyPipelineRawItem
        <4>2. CASE barrierOwner # owner
                    \/ barrierSource # source
          <5>1. /\ rpPendingAttachments' =
                       ReplyPendingAfterReconnectObservation(
                         owner, source,
                         ReplyAttachment(
                           owner, semantic, source, kind))
                  /\ rpItems' =
                       ReplyPipelineItemsAfterReconnectObservation(
                         owner, source)
            BY <1>1, <2>2, SMTT(5)
               DEF ObserveAuthenticatedReplyDelivery
          <5>2. ReplyReconnectPendingForSource(
                   barrierOwner, barrierSource)
            BY <3>1, <4>2, <5>1, SMTT(20)
               DEF ReplyPendingAfterReconnectObservation,
                   ReplyPendingAttachmentsFor,
                   ReplyReconnectPending,
                   ReplyReconnectPendingForSource,
                   ReplyAttachment
          <5>3. \A item \in rpItems:
                   (item.owner = barrierOwner
                     /\ item.source = barrierSource) =>
                     item.phase # "Ticketed"
            BY <1>1, <5>2, SMTT(10)
               DEF ReplyPipelineReconnectNoTicketedInvariant
          <5>4. \A item \in rpItems':
                   (item.owner = barrierOwner
                     /\ item.source = barrierSource) =>
                     item \in rpItems
            BY <4>2, <5>1, SMTT(20)
               DEF ReplyPipelineItemsAfterReconnectObservation,
                   ReplyPipelineItemWithoutTicket,
                   ReplyPipelineRawItem
          <5> QED BY <5>3, <5>4, SMTT(10)
        <4> QED BY <4>1, <4>2, SMTT(5)
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectNoTicketedInvariant
    <2> QED BY <2>1, <2>2, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesReconnectWriterActiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineReconnectWriterActiveInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineReconnectWriterActiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineReconnectWriterActiveInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE ReplyPipelineReconnectWriterActiveInvariant'
    <2>1. CASE kind # "Reconnect"
      <3>1. ASSUME NEW barrierOwner \in ReplyOwners,
                    NEW barrierSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      barrierOwner, barrierSource)',
                    ReplyPipelineHasUnresolvedWriter(
                      barrierOwner, barrierSource)'
             PROVE rrSourceActive'[barrierOwner][barrierSource]
        <4>1. /\ rpItems' = rpItems
               /\ rpPendingAttachments' =
                    rpPendingAttachments \cup
                      {ReplyAttachment(
                         owner, semantic, source, kind)}
               /\ rrSourceActive' = rrSourceActive
          BY <1>1, <2>1, SMTT(5)
             DEF ObserveAuthenticatedReplyDelivery,
                 ReplyRouteVars
        <4>2. ReplyReconnectPendingForSource(
                 barrierOwner, barrierSource)
          BY <1>1, <2>1, <3>1, <4>1, SMTT(20)
             DEF ReplyPendingAttachmentsFor,
                 ReplyReconnectPending,
                 ReplyReconnectPendingForSource,
                 ReplyAttachment
        <4>3. ReplyPipelineHasUnresolvedWriter(
                 barrierOwner, barrierSource)
          BY <3>1, <4>1, SMTT(10)
             DEF ReplyPipelineHasUnresolvedWriter
        <4>4. rrSourceActive[barrierOwner][barrierSource]
          BY <1>1, <4>2, <4>3, SMTT(10)
             DEF ReplyPipelineReconnectWriterActiveInvariant
        <4> QED BY <4>1, <4>4
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectWriterActiveInvariant
    <2>2. CASE kind = "Reconnect"
      <3>1. ASSUME NEW barrierOwner \in ReplyOwners,
                    NEW barrierSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      barrierOwner, barrierSource)',
                    ReplyPipelineHasUnresolvedWriter(
                      barrierOwner, barrierSource)'
             PROVE rrSourceActive'[barrierOwner][barrierSource]
        <4>1. CASE barrierOwner = owner
                    /\ barrierSource = source
          BY <1>1, <2>2, <3>1, <4>1, SMTT(10)
             DEF ObserveAuthenticatedReplyDelivery,
                 ReplyRouteVars
        <4>2. CASE barrierOwner # owner
                    \/ barrierSource # source
          <5>1. /\ rpPendingAttachments' =
                       ReplyPendingAfterReconnectObservation(
                         owner, source,
                         ReplyAttachment(
                           owner, semantic, source, kind))
                  /\ rpItems' =
                       ReplyPipelineItemsAfterReconnectObservation(
                         owner, source)
                  /\ rrSourceActive' = rrSourceActive
            BY <1>1, <2>2, SMTT(5)
               DEF ObserveAuthenticatedReplyDelivery,
                   ReplyRouteVars
          <5>2. ReplyReconnectPendingForSource(
                   barrierOwner, barrierSource)
            BY <3>1, <4>2, <5>1, SMTT(20)
               DEF ReplyPendingAfterReconnectObservation,
                   ReplyPendingAttachmentsFor,
                   ReplyReconnectPending,
                   ReplyReconnectPendingForSource,
                   ReplyAttachment
          <5>3. ReplyPipelineHasUnresolvedWriter(
                   barrierOwner, barrierSource)
            BY <3>1, <5>1, SMTT(20)
               DEF ReplyPipelineHasUnresolvedWriter,
                   ReplyPipelineItemsAfterReconnectObservation,
                   ReplyPipelineItemWithoutTicket,
                   ReplyPipelineRawItem
          <5>4. rrSourceActive[barrierOwner][barrierSource]
            BY <1>1, <5>2, <5>3, SMTT(10)
               DEF ReplyPipelineReconnectWriterActiveInvariant
          <5> QED BY <5>1, <5>4
        <4> QED BY <4>1, <4>2, SMTT(5)
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectWriterActiveInvariant
    <2> QED BY <2>1, <2>2, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyObservedDeliveryPreservesReconnectBarrierInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineReconnectBarrierInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineReconnectBarrierInvariant'
BY ReplyObservedDeliveryPreservesReconnectNoTicketedInvariant,
   ReplyObservedDeliveryPreservesReconnectWriterActiveInvariant,
   SMTT(10)
   DEF ReplyPipelineReconnectBarrierInvariant

THEOREM ReplyObservedDeliveryPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyPipelineInductiveInvariant
    /\ ObserveAuthenticatedReplyDelivery(
         owner, semantic, source, kind)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyPipelineInductiveInvariant,
                ObserveAuthenticatedReplyDelivery(
                  owner, semantic, source, kind)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStutterPreservesInductiveInvariant
         DEF ObserveAuthenticatedReplyDelivery,
             ReplyPipelineInductiveInvariant, ReplyRouteVars
    <2>2. ReplyPipelineConfiguration'
      BY <1>1 DEF ReplyPipelineInductiveInvariant
    <2>3. ReplyPipelineTypeInvariant'
      BY <1>1, ReplyObservedDeliveryPreservesTypeInvariant
         DEF ReplyPipelineInductiveInvariant
    <2>4. ReplyPipelinePerIdentityInvariant'
      BY <1>1,
         ReplyObservedDeliveryPreservesPerIdentityInvariant
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>5. ReplyPipelineFifoOrdinalInvariant'
      BY <1>1,
         ReplyObservedDeliveryPreservesFifoOrdinalInvariant
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>6. ReplyPipelineTicketIdentityInvariant'
      BY <1>1,
         ReplyObservedDeliveryPreservesTicketIdentityInvariant
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>7. ReplyPipelineItemBindingInvariant'
      BY <1>1,
         ReplyObservedDeliveryPreservesItemBindingInvariant
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>8. ReplyPipelineReconnectBarrierInvariant'
      BY <1>1,
         ReplyObservedDeliveryPreservesReconnectBarrierInvariant
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                  <2>5, <2>6, <2>7, <2>8
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
  <1> QED BY <1>1

THEOREM ReplyAttemptTicketClearPreservesIdentityAndCursor ==
  \A attempt \in ReplyAttemptSet:
    /\ SameReplyAttemptIdentity(
         attempt, ReplyAttemptWithoutTicket(attempt))
    /\ ReplyAttemptCursor(ReplyAttemptWithoutTicket(attempt)) =
         ReplyAttemptCursor(attempt)
BY SMTT(10)
   DEF ReplyAttemptWithoutTicket, SameReplyAttemptIdentity,
       ReplyAttemptCursor, ReplyAttemptSet,
       ReplyDeliveryOrdinals, ReplyConnectionTenures

THEOREM ReplyAttemptRouteUpdatePreservesIdentityAndCursor ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    LET routed ==
          ReplyAttemptWithRoute(
            attempt, deliveryOrdinal, connectionTenure)
    IN /\ SameReplyAttemptIdentity(attempt, routed)
       /\ ReplyAttemptCursor(routed) = ReplyAttemptCursor(attempt)
BY SMTT(10)
   DEF ReplyAttemptWithRoute, SameReplyAttemptIdentity,
       ReplyAttemptCursor, ReplyAttemptSet,
       ReplyDeliveryOrdinals, ReplyConnectionTenures

THEOREM ReplyRouteSafetyUniqueAttemptIdentity ==
  ReplyRouteSafetyInvariant
  => \A left, right \in rrAttempts:
       SameReplyAttemptIdentity(left, right) => left = right
PROOF
  <1>1. ASSUME ReplyRouteSafetyInvariant
         PROVE \A left, right \in rrAttempts:
                 SameReplyAttemptIdentity(left, right) => left = right
    <2>1. ASSUME NEW left \in rrAttempts,
                  NEW right \in rrAttempts,
                  SameReplyAttemptIdentity(left, right)
           PROVE left = right
      <3>1. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)
             IN IsFiniteSet(attempts)
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyRouteOwnershipInvariant, ReplyAttemptSet
      <3>2. /\ left.owner \in ReplyOwners
             /\ left.semantic \in ReplySemantics
             /\ left.source \in ReplySources
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyAttemptSet
      <3>3. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)
             IN /\ left \in attempts
                /\ right \in attempts
        BY <2>1
           DEF ReplyAttemptsForSource, ReplyAttemptsFor,
               SameReplyAttemptIdentity
      <3>4. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)
             IN Cardinality(attempts) <= 1
        BY <1>1, <3>2
           DEF ReplyRouteSafetyInvariant,
               ReplyRouteOwnershipInvariant
      <3>5. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)
             IN Cardinality(attempts) = 1
        BY <3>1, <3>3, <3>4,
           FS_CardinalityType, FS_EmptySet, SMTT(10)
      <3>6. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)
             IN \E attempt: attempts = {attempt}
        BY <3>1, <3>5, FS_Singleton
      <3> QED BY <3>3, <3>6, SMTT(5)
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyRouteSafetyOwnedAttemptSingleton ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyAttemptOwned(owner, semantic, source)
    => ReplyAttemptsForSource(owner, semantic, source) =
         {ReplyAttemptFor(owner, semantic, source)}
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyAttemptOwned(owner, semantic, source)
         PROVE ReplyAttemptsForSource(owner, semantic, source) =
                 {ReplyAttemptFor(owner, semantic, source)}
    <2>1. ReplyAttemptFor(owner, semantic, source) \in
            ReplyAttemptsForSource(owner, semantic, source)
      BY <1>1, SMTT(10)
         DEF ReplyAttemptOwned, ReplyAttemptFor
    <2>2. \A candidate \in
                 ReplyAttemptsForSource(owner, semantic, source):
             candidate = ReplyAttemptFor(owner, semantic, source)
      BY <1>1, <2>1,
         ReplyRouteSafetyUniqueAttemptIdentity, SMTT(20)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource,
             SameReplyAttemptIdentity
    <2> QED BY <2>1, <2>2, SMTT(10)
  <1> QED BY <1>1

THEOREM ReplyRouteSafetyUniqueAttemptIdentityPrime ==
  ReplyRouteSafetyInvariant'
  => \A left, right \in rrAttempts':
       SameReplyAttemptIdentity(left, right) => left = right
PROOF
  <1>1. ASSUME ReplyRouteSafetyInvariant'
         PROVE \A left, right \in rrAttempts':
                 SameReplyAttemptIdentity(left, right) => left = right
    <2>1. ASSUME NEW left \in rrAttempts',
                  NEW right \in rrAttempts',
                  SameReplyAttemptIdentity(left, right)
           PROVE left = right
      <3>1. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
             IN IsFiniteSet(attempts)
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyRouteOwnershipInvariant, ReplyAttemptSet
      <3>2. /\ left.owner \in ReplyOwners
             /\ left.semantic \in ReplySemantics
             /\ left.source \in ReplySources
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyAttemptSet
      <3>3. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
             IN /\ left \in attempts
                /\ right \in attempts
        BY <2>1
           DEF ReplyAttemptsForSource, ReplyAttemptsFor,
               SameReplyAttemptIdentity
      <3>4. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
             IN Cardinality(attempts) <= 1
        BY <1>1, <3>2
           DEF ReplyRouteSafetyInvariant,
               ReplyRouteOwnershipInvariant
      <3>5. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
             IN Cardinality(attempts) = 1
        BY <3>1, <3>3, <3>4,
           FS_CardinalityType, FS_EmptySet, SMTT(10)
      <3>6. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
             IN \E attempt: attempts = {attempt}
        BY <3>1, <3>5, FS_Singleton
      <3> QED BY <3>3, <3>6, SMTT(5)
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyRouteSafetyOwnedAttemptSingletonPrime ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyAttemptOwned(owner, semantic, source)'
    => ReplyAttemptsForSource(owner, semantic, source)' =
         {ReplyAttemptFor(owner, semantic, source)'}
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant',
                ReplyAttemptOwned(owner, semantic, source)'
         PROVE ReplyAttemptsForSource(
                 owner, semantic, source)' =
                 {ReplyAttemptFor(owner, semantic, source)'}
    <2>1. ReplyAttemptFor(owner, semantic, source)' \in
            ReplyAttemptsForSource(owner, semantic, source)'
      BY <1>1, SMTT(10)
         DEF ReplyAttemptOwned, ReplyAttemptFor
    <2>2. \A candidate \in
                 ReplyAttemptsForSource(owner, semantic, source)':
             candidate = ReplyAttemptFor(owner, semantic, source)'
      BY <1>1, <2>1,
         ReplyRouteSafetyUniqueAttemptIdentityPrime, SMTT(20)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource,
             SameReplyAttemptIdentity
    <2> QED BY <2>1, <2>2, SMTT(10)
  <1> QED BY <1>1

THEOREM ReplyRetireAttemptsEffect ==
  \A owner \in ReplyOwners, source \in ReplySources:
    RetireReplySource(owner, source) =>
      rrAttempts' =
        {IF attempt.owner = owner /\ attempt.source = source
         THEN ReplyAttemptWithoutTicket(attempt)
         ELSE attempt: attempt \in rrAttempts}
BY SMTT(5) DEF RetireReplySource

THEOREM ReplyRetireOwnedAttemptProjection ==
  \A owner \in ReplyOwners, source \in ReplySources,
     checkedOwner \in ReplyOwners,
     checkedSemantic \in ReplySemantics,
     checkedSource \in ReplySources:
    LET oldAttempt ==
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource)
        retiredAttempt ==
          ReplyAttemptAfterRetire(owner, source, oldAttempt)
    IN /\ ReplyRouteSafetyInvariant
       /\ ReplyAttemptOwned(
            checkedOwner, checkedSemantic, checkedSource)
       /\ RetireReplySource(owner, source)
       => /\ ReplyAttemptsForSource(
                checkedOwner, checkedSemantic, checkedSource)' =
              {retiredAttempt}
          /\ ReplyAttemptFor(
               checkedOwner, checkedSemantic, checkedSource)' =
               retiredAttempt
          /\ SameReplyAttemptIdentity(oldAttempt, retiredAttempt)
          /\ ReplyAttemptCursor(retiredAttempt) =
               ReplyAttemptCursor(oldAttempt)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSemantic \in ReplySemantics,
                NEW checkedSource \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyAttemptOwned(
                  checkedOwner, checkedSemantic, checkedSource),
                RetireReplySource(owner, source)
         PROVE LET oldAttempt ==
                     ReplyAttemptFor(
                       checkedOwner, checkedSemantic, checkedSource)
                   retiredAttempt ==
                     ReplyAttemptAfterRetire(
                       owner, source, oldAttempt)
               IN /\ ReplyAttemptsForSource(
                        checkedOwner,
                        checkedSemantic,
                        checkedSource)' = {retiredAttempt}
                  /\ ReplyAttemptFor(
                       checkedOwner,
                       checkedSemantic,
                       checkedSource)' = retiredAttempt
                  /\ SameReplyAttemptIdentity(
                       oldAttempt, retiredAttempt)
                  /\ ReplyAttemptCursor(retiredAttempt) =
                       ReplyAttemptCursor(oldAttempt)
    <2>1. ReplyAttemptsForSource(
             checkedOwner, checkedSemantic, checkedSource) =
           {ReplyAttemptFor(
              checkedOwner, checkedSemantic, checkedSource)}
      BY <1>1, ReplyRouteSafetyOwnedAttemptSingleton
    <2>2. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
             \in ReplyAttemptSet
      BY <1>1, SMTT(20)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
               retiredAttempt ==
                 ReplyAttemptAfterRetire(owner, source, oldAttempt)
           IN /\ SameReplyAttemptIdentity(
                    oldAttempt, retiredAttempt)
              /\ ReplyAttemptCursor(retiredAttempt) =
                   ReplyAttemptCursor(oldAttempt)
      BY <2>2,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         SMTT(10)
         DEF ReplyAttemptAfterRetire,
             SameReplyAttemptIdentity,
             ReplyAttemptCursor
    <2>4. rrAttempts' =
           {ReplyAttemptAfterRetire(owner, source, attempt):
              attempt \in rrAttempts}
      BY <1>1, ReplyRetireAttemptsEffect, SMTT(10)
         DEF ReplyAttemptAfterRetire
    <2>5. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
               retiredAttempt ==
                 ReplyAttemptAfterRetire(owner, source, oldAttempt)
           IN ReplyAttemptsForSource(
                checkedOwner, checkedSemantic, checkedSource)' =
                {retiredAttempt}
      BY <1>1, <2>1, <2>3, <2>4,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         SMTT(60)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptAfterRetire,
             SameReplyAttemptIdentity, ReplyAttemptSet
    <2>6. LET retiredAttempt ==
                 ReplyAttemptAfterRetire(
                   owner, source,
                   ReplyAttemptFor(
                     checkedOwner, checkedSemantic, checkedSource))
           IN ReplyAttemptFor(
                checkedOwner, checkedSemantic, checkedSource)' =
                retiredAttempt
      BY <2>5 DEF ReplyAttemptFor
    <2> QED BY <2>3, <2>5, <2>6
  <1> QED BY <1>1

THEOREM ReplyAttemptAfterRetirePreservesOtherSource ==
  \A owner, source, attempt:
    (attempt.owner # owner \/ attempt.source # source) =>
      ReplyAttemptAfterRetire(owner, source, attempt) = attempt
BY SMTT(5) DEF ReplyAttemptAfterRetire

THEOREM ReplyNestedActiveUpdatePreservesOtherOwner ==
  \A active \in [ReplyOwners -> [ReplySources -> BOOLEAN]],
     owner, checkedOwner \in ReplyOwners,
     source, checkedSource \in ReplySources,
     value \in BOOLEAN:
    checkedOwner # owner =>
      [active EXCEPT ![owner][source] = value]
        [checkedOwner][checkedSource] =
          active[checkedOwner][checkedSource]
BY SMTT(10)

THEOREM ReplyNestedActiveUpdatePreservesOtherSource ==
  \A active \in [ReplyOwners -> [ReplySources -> BOOLEAN]],
     owner \in ReplyOwners,
     source, checkedSource \in ReplySources,
     value \in BOOLEAN:
    checkedSource # source =>
      [active EXCEPT ![owner][source] = value]
        [owner][checkedSource] = active[owner][checkedSource]
BY SMTT(10)

THEOREM ReplyNestedTenureUpdatePreservesOtherOwner ==
  \A tenures \in
       [ReplyOwners -> [ReplySources -> ReplyConnectionTenures]],
     owner, checkedOwner \in ReplyOwners,
     source, checkedSource \in ReplySources,
     connectionTenure \in ReplyConnectionTenures:
    checkedOwner # owner =>
      [tenures EXCEPT ![owner][source] = connectionTenure]
        [checkedOwner][checkedSource] =
          tenures[checkedOwner][checkedSource]
BY SMTT(10)

THEOREM ReplyNestedTenureUpdatePreservesOtherSource ==
  \A tenures \in
       [ReplyOwners -> [ReplySources -> ReplyConnectionTenures]],
     owner \in ReplyOwners,
     source, checkedSource \in ReplySources,
     connectionTenure \in ReplyConnectionTenures:
    checkedSource # source =>
      [tenures EXCEPT ![owner][source] = connectionTenure]
        [owner][checkedSource] = tenures[owner][checkedSource]
BY SMTT(10)

THEOREM ReplyRetirePreservesConnectionTenures ==
  \A owner \in ReplyOwners, source \in ReplySources:
    RetireReplySource(owner, source) =>
      rrConnectionTenure' = rrConnectionTenure
BY SMTT(5) DEF RetireReplySource

THEOREM ReplyRetirePreservesOtherSourceActivity ==
  \A owner, checkedOwner \in ReplyOwners,
     source, checkedSource \in ReplySources:
    /\ rrSourceActive
         \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
    /\ RetireReplySource(owner, source)
    /\ (checkedOwner # owner \/ checkedSource # source)
    => rrSourceActive'[checkedOwner][checkedSource] =
         rrSourceActive[checkedOwner][checkedSource]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW checkedOwner \in ReplyOwners,
                NEW source \in ReplySources,
                NEW checkedSource \in ReplySources,
                rrSourceActive
                  \in [ReplyOwners -> [ReplySources -> BOOLEAN]],
                RetireReplySource(owner, source),
                checkedOwner # owner \/ checkedSource # source
         PROVE rrSourceActive'[checkedOwner][checkedSource] =
                 rrSourceActive[checkedOwner][checkedSource]
    <2>1. CASE checkedOwner # owner
      BY <1>1, <2>1,
         ReplyNestedActiveUpdatePreservesOtherOwner,
         SMTT(5)
         DEF RetireReplySource
    <2>2. CASE checkedOwner = owner
                /\ checkedSource # source
      BY <1>1, <2>2,
         ReplyNestedActiveUpdatePreservesOtherSource,
         SMTT(5)
         DEF RetireReplySource
    <2> QED BY <1>1, <2>1, <2>2, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyRetirePreservesOtherAttemptCurrent ==
  \A owner \in ReplyOwners, source \in ReplySources,
     checkedOwner \in ReplyOwners,
     checkedSemantic \in ReplySemantics,
     checkedSource \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyAttemptOwned(
         checkedOwner, checkedSemantic, checkedSource)
    /\ RetireReplySource(owner, source)
    /\ (checkedOwner # owner \/ checkedSource # source)
    => (ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource))' <=>
        ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource)))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSemantic \in ReplySemantics,
                NEW checkedSource \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyAttemptOwned(
                  checkedOwner, checkedSemantic, checkedSource),
                RetireReplySource(owner, source),
                checkedOwner # owner \/ checkedSource # source
         PROVE ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))' <=>
                 ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))
    <2>1. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN ReplyAttemptFor(
                checkedOwner, checkedSemantic, checkedSource)' =
                ReplyAttemptAfterRetire(owner, source, oldAttempt)
      BY <1>1, ReplyRetireOwnedAttemptProjection, SMTT(5)
    <2>2. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN /\ oldAttempt.owner = checkedOwner
              /\ oldAttempt.semantic = checkedSemantic
              /\ oldAttempt.source = checkedSource
      BY <1>1, SMTT(15)
         DEF ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN ReplyAttemptAfterRetire(owner, source, oldAttempt) =
                oldAttempt
      BY <1>1, <2>2,
         ReplyAttemptAfterRetirePreservesOtherSource,
         SMTT(5)
    <2>4. rrConnectionTenure' = rrConnectionTenure
      BY <1>1, ReplyRetirePreservesConnectionTenures
    <2>5. rrSourceActive'[checkedOwner][checkedSource] =
           rrSourceActive[checkedOwner][checkedSource]
      BY <1>1, ReplyRetirePreservesOtherSourceActivity,
         SMTT(10)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyRetirePendingPreservesItemCoreBinding ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => \A item \in rpItems': ReplyPipelineItemCoreBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                RetirePendingReconnectSource(owner, source)
         PROVE \A item \in rpItems':
                 ReplyPipelineItemCoreBinding(item)'
    <2>1. /\ rpItems' = rpItems
           /\ RetireReplySource(owner, source)
      BY <1>1, SMTT(5)
         DEF RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>2. /\ ReplyRouteSafetyInvariant
           /\ rrSourceActive
                \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>3. ASSUME NEW item \in rpItems'
           PROVE ReplyPipelineItemCoreBinding(item)'
      <3>1. /\ item \in rpItems
             /\ ReplyPipelineItemCoreBinding(item)
             /\ item.owner \in ReplyOwners
             /\ item.semantic \in ReplySemantics
             /\ item.source \in ReplySources
        BY <1>1, <2>1, <2>3, SMTT(30)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineItemBindingInvariant,
               ReplyPipelineTypeInvariant,
               ReplyPipelineItemHasType
      <3>2. LET oldAttempt ==
                   ReplyAttemptFor(
                     item.owner, item.semantic, item.source)
                 retiredAttempt ==
                   ReplyAttemptAfterRetire(
                     owner, source, oldAttempt)
             IN /\ ReplyAttemptsForSource(
                      item.owner, item.semantic, item.source)' =
                    {retiredAttempt}
                /\ ReplyAttemptFor(
                     item.owner, item.semantic, item.source)' =
                     retiredAttempt
                /\ SameReplyAttemptIdentity(
                     oldAttempt, retiredAttempt)
                /\ ReplyAttemptCursor(retiredAttempt) =
                     ReplyAttemptCursor(oldAttempt)
        BY <2>1, <2>2, <3>1,
           ReplyRetireOwnedAttemptProjection, SMTT(10)
           DEF ReplyPipelineItemCoreBinding
      <3> QED BY <3>1, <3>2, SMTT(20)
           DEF ReplyPipelineItemCoreBinding,
               ReplyPipelineItemMatchesAttempt,
               ReplyAttemptOwned,
               ReplyAttemptComplete,
               SameReplyAttemptIdentity,
               ReplyAttemptCursor
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM ReplyRetirePendingPreservesItemRouteBinding ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => \A item \in rpItems': ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                RetirePendingReconnectSource(owner, source)
         PROVE \A item \in rpItems':
                 ReplyPipelineItemRouteBinding(item)'
    <2>1. /\ rpItems' = rpItems
           /\ rpPendingAttachments' = rpPendingAttachments
           /\ ReplyPipelineEverySourceAttemptHasRebind(
                owner, source)
           /\ RetireReplySource(owner, source)
      BY <1>1, SMTT(10)
         DEF RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>2. /\ ReplyRouteSafetyInvariant
           /\ rrSourceActive
                \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>3. ASSUME NEW item \in rpItems'
           PROVE ReplyPipelineItemRouteBinding(item)'
      <3>1. /\ item \in rpItems
             /\ ReplyPipelineItemCoreBinding(item)
             /\ ReplyPipelineItemRouteBinding(item)
             /\ ReplyAttemptOwned(
                  item.owner, item.semantic, item.source)
             /\ item.owner \in ReplyOwners
             /\ item.semantic \in ReplySemantics
             /\ item.source \in ReplySources
        <4>1. item \in rpItems
          BY <2>1, <2>3
        <4>2. /\ ReplyPipelineItemCoreBinding(item)
               /\ ReplyPipelineItemRouteBinding(item)
          BY <1>1, <4>1, SMTT(10)
             DEF ReplyPipelineInductiveInvariant,
                 ReplyPipelineOwnershipInvariant,
                 ReplyPipelineItemBindingInvariant
        <4>3. ReplyAttemptOwned(
                 item.owner, item.semantic, item.source)
          BY <4>2 DEF ReplyPipelineItemCoreBinding
        <4>4. /\ item.owner \in ReplyOwners
               /\ item.semantic \in ReplySemantics
               /\ item.source \in ReplySources
          BY <1>1, <4>1, SMTT(10)
             DEF ReplyPipelineInductiveInvariant,
                 ReplyPipelineTypeInvariant,
                 ReplyPipelineItemHasType
        <4> QED BY <4>1, <4>2, <4>3, <4>4
      <3>2. LET oldAttempt ==
                   ReplyAttemptFor(
                     item.owner, item.semantic, item.source)
                 retiredAttempt ==
                   ReplyAttemptAfterRetire(
                     owner, source, oldAttempt)
             IN /\ ReplyAttemptFor(
                      item.owner, item.semantic, item.source)' =
                    retiredAttempt
                /\ SameReplyAttemptIdentity(
                     oldAttempt, retiredAttempt)
        BY <2>1, <2>2, <3>1,
           ReplyRetireOwnedAttemptProjection, SMTT(10)
           DEF ReplyPipelineItemCoreBinding
      <3>3. CASE item.owner = owner
                    /\ item.source = source
        <4>1. ReplyRouteRebindPending(
                 item.owner, item.semantic, item.source)
          BY <2>1, <3>1, <3>3, SMTT(20)
             DEF ReplyPipelineEverySourceAttemptHasRebind,
                 ReplyPipelineItemCoreBinding,
                 ReplyAttemptOwned, ReplyAttemptFor,
                 ReplyAttemptsFor, ReplyAttemptsForSource
        <4>2. ReplyRouteRebindPending(
                 item.owner, item.semantic, item.source)'
          BY <2>1, <4>1, SMTT(10)
             DEF ReplyRouteRebindPending,
                 ReplyPendingAttachmentsFor
        <4> QED BY <4>2
             DEF ReplyPipelineItemRouteBinding
      <3>4. CASE item.owner # owner
                    \/ item.source # source
        <4>1. ReplyAttemptCurrent(
                 ReplyAttemptFor(
                   item.owner, item.semantic, item.source))' <=>
               ReplyAttemptCurrent(
                 ReplyAttemptFor(
                   item.owner, item.semantic, item.source))
          BY <2>1, <2>2, <3>1, <3>4,
             ReplyRetirePreservesOtherAttemptCurrent
        <4>2. ReplyRouteRebindPending(
                 item.owner, item.semantic, item.source)' <=>
               ReplyRouteRebindPending(
                 item.owner, item.semantic, item.source)
          BY <2>1, SMTT(10)
             DEF ReplyRouteRebindPending,
                 ReplyPendingAttachmentsFor
        <4> QED BY <3>1, <4>1, <4>2, SMTT(5)
             DEF ReplyPipelineItemRouteBinding
      <3> QED BY <3>3, <3>4, SMTT(5)
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM ReplyRetirePendingPreservesItemPhaseBinding ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => \A item \in rpItems': ReplyPipelineItemPhaseBinding(item)'
BY SMTT(20)
   DEF ReplyPipelineInductiveInvariant,
       ReplyPipelineOwnershipInvariant,
       ReplyPipelineItemBindingInvariant,
       ReplyPipelineItemPhaseBinding,
       RetirePendingReconnectSource,
       ReplyPipelineLocalVars,
       ReplyPipelineQueuedItem, ReplyPipelineTicketValid,
       ReplyPipelineItemIsFifoHead, ReplyPipelineItemsInLane

THEOREM ReplyRetirePendingPreservesItemBindingInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => ReplyPipelineItemBindingInvariant'
BY ReplyRetirePendingPreservesItemCoreBinding,
   ReplyRetirePendingPreservesItemRouteBinding,
   ReplyRetirePendingPreservesItemPhaseBinding, SMTT(10)
   DEF ReplyPipelineItemBindingInvariant

THEOREM ReplyRetirePendingPreservesReconnectNoTicketedInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => ReplyPipelineReconnectNoTicketedInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                RetirePendingReconnectSource(owner, source)
         PROVE ReplyPipelineReconnectNoTicketedInvariant'
    <2>1. /\ rpPendingAttachments' = rpPendingAttachments
           /\ rpItems' = rpItems
      BY <1>1, SMTT(5)
         DEF RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>2. ReplyPipelineReconnectNoTicketedInvariant
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineReconnectBarrierInvariant
    <2> QED BY <2>1, <2>2, SMTT(20)
         DEF ReplyPipelineReconnectNoTicketedInvariant,
             ReplyReconnectPendingForSource,
             ReplyReconnectPending,
             ReplyPendingAttachmentsFor
  <1> QED BY <1>1

THEOREM ReplyRetirePendingPreservesReconnectWriterActiveInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => ReplyPipelineReconnectWriterActiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                RetirePendingReconnectSource(owner, source)
         PROVE ReplyPipelineReconnectWriterActiveInvariant'
    <2>1. /\ rpPendingAttachments' = rpPendingAttachments
           /\ rpItems' = rpItems
           /\ ~ReplyPipelineHasUnresolvedWriter(owner, source)
           /\ RetireReplySource(owner, source)
      BY <1>1, SMTT(10)
         DEF RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>2. ReplyPipelineReconnectWriterActiveInvariant
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineReconnectBarrierInvariant
    <2>3. rrSourceActive
             \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>4. ASSUME NEW barrierOwner \in ReplyOwners,
                  NEW barrierSource \in ReplySources,
                  ReplyReconnectPendingForSource(
                    barrierOwner, barrierSource)',
                  ReplyPipelineHasUnresolvedWriter(
                    barrierOwner, barrierSource)'
           PROVE rrSourceActive'[barrierOwner][barrierSource]
      <3>1. /\ ReplyReconnectPendingForSource(
                   barrierOwner, barrierSource)
             /\ ReplyPipelineHasUnresolvedWriter(
                  barrierOwner, barrierSource)
        BY <2>1, <2>4, SMTT(15)
           DEF ReplyReconnectPendingForSource,
               ReplyReconnectPending,
               ReplyPendingAttachmentsFor,
               ReplyPipelineHasUnresolvedWriter
      <3>2. CASE barrierOwner = owner
                  /\ barrierSource = source
        BY <2>1, <3>1, <3>2
      <3>3. CASE barrierOwner # owner
                  \/ barrierSource # source
        <4>1. rrSourceActive[barrierOwner][barrierSource]
          BY <2>2, <3>1, SMTT(10)
             DEF ReplyPipelineReconnectWriterActiveInvariant
        <4>2. rrSourceActive'[barrierOwner][barrierSource] =
               rrSourceActive[barrierOwner][barrierSource]
          BY <2>1, <2>3, <3>3,
             ReplyRetirePreservesOtherSourceActivity
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>2, <3>3, SMTT(5)
    <2> QED BY <2>4
         DEF ReplyPipelineReconnectWriterActiveInvariant
  <1> QED BY <1>1

THEOREM ReplyRetirePendingPreservesReconnectBarrierInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => ReplyPipelineReconnectBarrierInvariant'
BY ReplyRetirePendingPreservesReconnectNoTicketedInvariant,
   ReplyRetirePendingPreservesReconnectWriterActiveInvariant,
   SMTT(10)
   DEF ReplyPipelineReconnectBarrierInvariant

THEOREM ReplyRetirePendingReconnectPreservesInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ RetirePendingReconnectSource(owner, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                RetirePendingReconnectSource(owner, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, RetireReplySourcePreservesInductiveInvariant
         DEF ReplyPipelineInductiveInvariant,
             RetirePendingReconnectSource
    <2>2. ReplyPipelineConfiguration'
      BY <1>1 DEF ReplyPipelineInductiveInvariant
    <2>3. ReplyPipelineTypeInvariant'
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>4. ReplyPipelinePerIdentityInvariant'
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelinePerIdentityInvariant,
             ReplyPipelinePendingPerIdentityInvariant,
             ReplyPipelineItemPerIdentityInvariant,
             RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>5. ReplyPipelineFifoOrdinalInvariant'
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineFifoOrdinalInvariant,
             RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>6. ReplyPipelineTicketIdentityInvariant'
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineTicketIdentityInvariant,
             RetirePendingReconnectSource,
             ReplyPipelineLocalVars
    <2>7. ReplyPipelineItemBindingInvariant'
      BY <1>1,
         ReplyRetirePendingPreservesItemBindingInvariant
    <2>8. ReplyPipelineReconnectBarrierInvariant'
      BY <1>1,
         ReplyRetirePendingPreservesReconnectBarrierInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                  <2>5, <2>6, <2>7, <2>8
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
  <1> QED BY <1>1

THEOREM ReplyPipelineItemWithRouteTenureProjection ==
  \A item, connectionTenure:
    LET rebound ==
          ReplyPipelineItemWithRouteTenure(item, connectionTenure)
    IN /\ rebound.owner = item.owner
       /\ rebound.semantic = item.semantic
       /\ rebound.source = item.source
       /\ rebound.messageCursor = item.messageCursor
       /\ rebound.chunkCursor = item.chunkCursor
       /\ rebound.outputClass = item.outputClass
       /\ rebound.flushRequired = item.flushRequired
       /\ rebound.fifoOrdinal = item.fifoOrdinal
       /\ rebound.routeTenure = connectionTenure
       /\ rebound.phase = item.phase
       /\ rebound.ticketId = item.ticketId
       /\ rebound.ticketTenure = item.ticketTenure
       /\ rebound.ticketPayload = item.ticketPayload
BY SMTT(10)
   DEF ReplyPipelineItemWithRouteTenure,
       ReplyPipelineRawItem

THEOREM ReplyPipelineItemWithRouteTenureHasType ==
  ReplyPipelineConfiguration =>
    \A item, connectionTenure:
      /\ connectionTenure \in ReplyConnectionTenures
      /\ ReplyPipelineItemHasType(item)
      =>
        ReplyPipelineItemHasType(
          ReplyPipelineItemWithRouteTenure(
            item, connectionTenure))
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(20)
   DEF ReplyPipelineItemHasType,
       ReplyPipelineConfiguration,
       ReplyRouteConfiguration

THEOREM ReplyPendingAttachmentAsLaterProjection ==
  \A attachment:
    LET later == ReplyPendingAttachmentAsLater(attachment)
    IN /\ later.owner = attachment.owner
       /\ later.semantic = attachment.semantic
       /\ later.source = attachment.source
       /\ later.kind = "Later"
BY SMTT(10)
   DEF ReplyPendingAttachmentAsLater, ReplyAttachment

THEOREM ReplyPendingAttachmentAsLaterHasType ==
  \A attachment \in ReplyAttachmentSet:
    ReplyPendingAttachmentAsLater(attachment)
      \in ReplyAttachmentSet
BY SMTT(30)
   DEF ReplyPendingAttachmentAsLater, ReplyAttachment,
       ReplyAttachmentSet, ReplyAttachmentKinds

THEOREM ReplyPipelineRouteRebindPreservesItemTypes ==
  \A owner, semantic, source, connectionTenure:
    /\ ReplyPipelineConfiguration
    /\ \A item \in rpItems: ReplyPipelineItemHasType(item)
    /\ connectionTenure \in ReplyConnectionTenures
    => \A item \in ReplyPipelineItemsAfterRouteRebind(
                     owner, semantic, source, connectionTenure):
         ReplyPipelineItemHasType(item)
BY ReplyPipelineItemWithRouteTenureHasType, SMTT(30)
   DEF ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyReconnectAttachPreservesPendingTypes ==
  \A selected:
    rpPendingAttachments \subseteq ReplyAttachmentSet =>
      ReplyPendingAfterReconnectAttach(selected)
        \subseteq ReplyAttachmentSet
BY ReplyPendingAttachmentAsLaterHasType, SMTT(30)
   DEF ReplyPendingAfterReconnectAttach

THEOREM ReplyPipelineRouteRebindPreservesItemPerIdentity ==
  \A owner, semantic, source, connectionTenure:
    ReplyPipelineItemPerIdentityInvariant =>
      LET rebound ==
            ReplyPipelineItemsAfterRouteRebind(
              owner, semantic, source, connectionTenure)
      IN \A left, right \in rebound:
           /\ left.owner = right.owner
           /\ left.semantic = right.semantic
           /\ left.source = right.source
           => left = right
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(40)
   DEF ReplyPipelineItemPerIdentityInvariant,
       ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyReconnectAttachPreservesPendingPerIdentity ==
  \A selected:
    ReplyPipelinePendingPerIdentityInvariant =>
      LET pending == ReplyPendingAfterReconnectAttach(selected)
      IN \A left, right \in pending:
           /\ left.owner = right.owner
           /\ left.semantic = right.semantic
           /\ left.source = right.source
           => left = right
BY ReplyPendingAttachmentAsLaterProjection, SMTT(40)
   DEF ReplyPipelinePendingPerIdentityInvariant,
       ReplyPendingAfterReconnectAttach

THEOREM ReplyPendingRemovalPreservesPerIdentity ==
  \A selected:
    ReplyPipelinePendingPerIdentityInvariant =>
      \A left, right \in rpPendingAttachments \ {selected}:
        /\ left.owner = right.owner
        /\ left.semantic = right.semantic
        /\ left.source = right.source
        => left = right
BY SMTT(20)
   DEF ReplyPipelinePendingPerIdentityInvariant

THEOREM ReplyPipelineRouteRebindPreservesFifoOrdinal ==
  \A owner, semantic, source, connectionTenure:
    ReplyPipelineFifoOrdinalInvariant =>
      LET rebound ==
            ReplyPipelineItemsAfterRouteRebind(
              owner, semantic, source, connectionTenure)
      IN /\ \A left, right \in rebound:
              /\ left.owner = right.owner
              /\ left.fifoOrdinal = right.fifoOrdinal
              => left = right
         /\ \A item \in rebound:
              item.fifoOrdinal < rpNextFifoOrdinal[item.owner]
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(40)
   DEF ReplyPipelineFifoOrdinalInvariant,
       ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyPipelineRouteRebindPreservesTicketIdentity ==
  \A owner, semantic, source, connectionTenure:
    ReplyPipelineTicketIdentityInvariant =>
      LET rebound ==
            ReplyPipelineItemsAfterRouteRebind(
              owner, semantic, source, connectionTenure)
      IN \A left, right \in rebound:
           /\ left.owner = right.owner
           /\ left.ticketId # NoReplyPipelineTicket
           /\ left.ticketId = right.ticketId
           => left = right
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(40)
   DEF ReplyPipelineTicketIdentityInvariant,
       ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyAttachmentLocalEffectPreservesTypeInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyPipelineConfiguration
    /\ ReplyPipelineTypeInvariant
    /\ ReplyRouteTypeInvariant'
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    /\ UNCHANGED <<rpNextFifoOrdinal, rpNextTicketId>>
    => ReplyPipelineTypeInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyPipelineConfiguration,
                ReplyPipelineTypeInvariant,
                ReplyRouteTypeInvariant',
                rpPendingAttachments' =
                  IF selected.kind = "Reconnect"
                  THEN ReplyPendingAfterReconnectAttach(selected)
                  ELSE rpPendingAttachments \ {selected},
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems,
                UNCHANGED <<rpNextFifoOrdinal, rpNextTicketId>>
         PROVE ReplyPipelineTypeInvariant'
    <2>1. CASE selected.kind = "Reconnect"
      BY <1>1, <2>1,
         ReplyPipelineRouteRebindPreservesItemTypes,
         ReplyReconnectAttachPreservesPendingTypes,
         SMTT(30)
         DEF ReplyPipelineTypeInvariant, ReplyRouteTypeInvariant
    <2>2. CASE selected.kind = "Later"
      BY <1>1, <2>2,
         ReplyPipelineRouteRebindPreservesItemTypes,
         SMTT(30)
         DEF ReplyPipelineTypeInvariant, ReplyRouteTypeInvariant
    <2>3. CASE selected.kind = "New"
      BY <1>1, <2>3, SMTT(20)
         DEF ReplyPipelineTypeInvariant
    <2>4. CASE selected.kind = "Exact"
      BY <1>1, <2>4, SMTT(20)
         DEF ReplyPipelineTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                  SMTT(10)
         DEF ReplyAttachmentSet, ReplyAttachmentKinds
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesPerIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyPipelinePerIdentityInvariant
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    => ReplyPipelinePerIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyPipelinePerIdentityInvariant,
                rpPendingAttachments' =
                  IF selected.kind = "Reconnect"
                  THEN ReplyPendingAfterReconnectAttach(selected)
                  ELSE rpPendingAttachments \ {selected},
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems
         PROVE ReplyPipelinePerIdentityInvariant'
    <2>1. CASE selected.kind = "Reconnect"
      <3>1. ReplyPipelinePendingPerIdentityInvariant'
        BY <1>1, <2>1,
           ReplyReconnectAttachPreservesPendingPerIdentity,
           SMTT(10)
           DEF ReplyPipelinePerIdentityInvariant,
               ReplyPipelinePendingPerIdentityInvariant
      <3>2. ReplyPipelineItemPerIdentityInvariant'
        BY <1>1, <2>1,
           ReplyPipelineRouteRebindPreservesItemPerIdentity,
           SMTT(10)
           DEF ReplyPipelinePerIdentityInvariant,
               ReplyPipelineItemPerIdentityInvariant
      <3> QED BY <3>1, <3>2
           DEF ReplyPipelinePerIdentityInvariant
    <2>2. CASE selected.kind = "Later"
      <3>1. ReplyPipelinePendingPerIdentityInvariant'
        BY <1>1, <2>2,
           ReplyPendingRemovalPreservesPerIdentity,
           SMTT(10)
           DEF ReplyPipelinePerIdentityInvariant,
               ReplyPipelinePendingPerIdentityInvariant
      <3>2. ReplyPipelineItemPerIdentityInvariant'
        BY <1>1, <2>2,
           ReplyPipelineRouteRebindPreservesItemPerIdentity,
           SMTT(10)
           DEF ReplyPipelinePerIdentityInvariant,
               ReplyPipelineItemPerIdentityInvariant
      <3> QED BY <3>1, <3>2
           DEF ReplyPipelinePerIdentityInvariant
    <2>3. CASE selected.kind \notin {"Later", "Reconnect"}
      <3>1. ReplyPipelinePendingPerIdentityInvariant'
        BY <1>1, <2>3,
           ReplyPendingRemovalPreservesPerIdentity,
           SMTT(10)
           DEF ReplyPipelinePerIdentityInvariant,
               ReplyPipelinePendingPerIdentityInvariant
      <3>2. rpItems' = rpItems
        BY <1>1, <2>3
      <3>3. ReplyPipelineItemPerIdentityInvariant'
        BY <1>1, <3>2
           DEF ReplyPipelinePerIdentityInvariant,
               ReplyPipelineItemPerIdentityInvariant
      <3> QED BY <3>1, <3>3
           DEF ReplyPipelinePerIdentityInvariant
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesFifoOrdinalInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyPipelineFifoOrdinalInvariant
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    /\ rpNextFifoOrdinal' = rpNextFifoOrdinal
    => ReplyPipelineFifoOrdinalInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyPipelineFifoOrdinalInvariant,
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems,
                rpNextFifoOrdinal' = rpNextFifoOrdinal
         PROVE ReplyPipelineFifoOrdinalInvariant'
    <2>1. CASE selected.kind \in {"Later", "Reconnect"}
      BY <1>1, <2>1,
         ReplyPipelineRouteRebindPreservesFifoOrdinal,
         SMTT(20)
         DEF ReplyPipelineFifoOrdinalInvariant
    <2>2. CASE selected.kind \notin {"Later", "Reconnect"}
      BY <1>1, <2>2
         DEF ReplyPipelineFifoOrdinalInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesTicketIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyPipelineTicketIdentityInvariant
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    => ReplyPipelineTicketIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyPipelineTicketIdentityInvariant,
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems
         PROVE ReplyPipelineTicketIdentityInvariant'
    <2>1. CASE selected.kind \in {"Later", "Reconnect"}
      BY <1>1, <2>1,
         ReplyPipelineRouteRebindPreservesTicketIdentity,
         SMTT(20)
         DEF ReplyPipelineTicketIdentityInvariant
    <2>2. CASE selected.kind \notin {"Later", "Reconnect"}
      BY <1>1, <2>2
         DEF ReplyPipelineTicketIdentityInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyAttachmentRouteActionPreservesAttemptIdentityAndCursor ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyAttachmentRouteAction(owner, semantic, source, kind)
    => \A oldAttempt \in rrAttempts:
         \E newAttempt \in rrAttempts':
           /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
           /\ ReplyAttemptCursor(newAttempt) =
                ReplyAttemptCursor(oldAttempt)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyRouteSafetyInvariant,
                ReplyAttachmentRouteAction(
                  owner, semantic, source, kind)
         PROVE \A oldAttempt \in rrAttempts:
                 \E newAttempt \in rrAttempts':
                   /\ SameReplyAttemptIdentity(
                        oldAttempt, newAttempt)
                   /\ ReplyAttemptCursor(newAttempt) =
                        ReplyAttemptCursor(oldAttempt)
    <2>1. CASE kind = "New"
      BY <1>1, <2>1, FS_AddElement, SMTT(40)
         DEF ReplyAttachmentRouteAction,
             ObserveNewReplySource,
             SameReplyAttemptIdentity, ReplyAttemptCursor
    <2>2. CASE kind = "Exact"
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyAttachmentRouteAction,
             RetryExactReplySource, ReplyRouteVars,
             SameReplyAttemptIdentity, ReplyAttemptCursor
    <2>3. CASE kind = "Later"
      BY <1>1, <2>3,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttachmentRouteAction,
             ObserveLaterReplyDelivery,
             ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             SameReplyAttemptIdentity, ReplyAttemptCursor
    <2>4. CASE kind = "Reconnect"
      BY <1>1, <2>4,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttachmentRouteAction,
             ReconnectReplySource,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             SameReplyAttemptIdentity, ReplyAttemptCursor
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ReplyAttachmentKinds
  <1> QED BY <1>1

ReplyAttemptAfterAttachment(oldAttempt) ==
  CHOOSE newAttempt \in rrAttempts':
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ ReplyAttemptCursor(newAttempt) = ReplyAttemptCursor(oldAttempt)

THEOREM ReplyAttachmentRouteActionPreservesOwnedAttemptProjection ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds,
     checkedOwner \in ReplyOwners,
     checkedSemantic \in ReplySemantics,
     checkedSource \in ReplySources:
    LET oldAttempt ==
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource)
    IN /\ ReplyRouteSafetyInvariant
       /\ ReplyRouteSafetyInvariant'
       /\ ReplyAttemptOwned(
            checkedOwner, checkedSemantic, checkedSource)
       /\ ReplyAttachmentRouteAction(
            owner, semantic, source, kind)
       => /\ ReplyAttemptOwned(
                checkedOwner, checkedSemantic, checkedSource)'
          /\ SameReplyAttemptIdentity(
               oldAttempt,
               ReplyAttemptFor(
                 checkedOwner, checkedSemantic, checkedSource)')
          /\ ReplyAttemptCursor(
               ReplyAttemptFor(
                 checkedOwner, checkedSemantic, checkedSource)') =
               ReplyAttemptCursor(oldAttempt)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSemantic \in ReplySemantics,
                NEW checkedSource \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyAttemptOwned(
                  checkedOwner, checkedSemantic, checkedSource),
                ReplyAttachmentRouteAction(
                  owner, semantic, source, kind)
         PROVE LET oldAttempt ==
                     ReplyAttemptFor(
                       checkedOwner, checkedSemantic, checkedSource)
               IN /\ ReplyAttemptOwned(
                        checkedOwner,
                        checkedSemantic,
                        checkedSource)'
                  /\ SameReplyAttemptIdentity(
                       oldAttempt,
                       ReplyAttemptFor(
                         checkedOwner,
                         checkedSemantic,
                         checkedSource)')
                  /\ ReplyAttemptCursor(
                       ReplyAttemptFor(
                         checkedOwner,
                         checkedSemantic,
                         checkedSource)') =
                       ReplyAttemptCursor(oldAttempt)
    <2>1. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
             \in ReplyAttemptsForSource(
                   checkedOwner, checkedSemantic, checkedSource)
      BY <1>1, SMTT(10)
         DEF ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN \E newAttempt \in rrAttempts':
                /\ SameReplyAttemptIdentity(
                     oldAttempt, newAttempt)
                /\ ReplyAttemptCursor(newAttempt) =
                     ReplyAttemptCursor(oldAttempt)
      BY <1>1, <2>1,
         ReplyAttachmentRouteActionPreservesAttemptIdentityAndCursor,
         SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
               newAttempt == ReplyAttemptAfterAttachment(oldAttempt)
           IN /\ newAttempt \in rrAttempts'
              /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
              /\ ReplyAttemptCursor(newAttempt) =
                   ReplyAttemptCursor(oldAttempt)
      BY <2>2, SMTT(10)
         DEF ReplyAttemptAfterAttachment
    <2>4. \A left, right \in rrAttempts':
             SameReplyAttemptIdentity(left, right) => left = right
      BY <1>1, ReplyRouteSafetyUniqueAttemptIdentityPrime
    <2>5. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
               newAttempt == ReplyAttemptAfterAttachment(oldAttempt)
           IN newAttempt \in ReplyAttemptsForSource(
                                  checkedOwner,
                                  checkedSemantic,
                                  checkedSource)'
      BY <2>1, <2>3, SMTT(20)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource,
             SameReplyAttemptIdentity
    <2>6. ReplyAttemptOwned(
             checkedOwner, checkedSemantic, checkedSource)'
      BY <2>5
         DEF ReplyAttemptOwned
    <2>7. ReplyAttemptsForSource(
             checkedOwner, checkedSemantic, checkedSource)' =
           {ReplyAttemptFor(
              checkedOwner, checkedSemantic, checkedSource)'}
      BY <1>1, <2>6,
         ReplyRouteSafetyOwnedAttemptSingletonPrime
    <2>8. LET oldAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN /\ SameReplyAttemptIdentity(
                    oldAttempt,
                    ReplyAttemptFor(
                      checkedOwner,
                      checkedSemantic,
                      checkedSource)')
              /\ ReplyAttemptCursor(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource)') =
                   ReplyAttemptCursor(oldAttempt)
      BY <2>3, <2>5, <2>7, SMTT(10)
    <2> QED BY <2>6, <2>8
  <1> QED BY <1>1

THEOREM ReplyAttachmentRouteActionPreservesExistingItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
   \A item:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyPipelineItemHasType(item)
    /\ ReplyPipelineItemCoreBinding(item)
    /\ ReplyAttachmentRouteAction(owner, semantic, source, kind)
    => ReplyPipelineItemCoreBinding(item)'
BY ReplyAttachmentRouteActionPreservesOwnedAttemptProjection,
   SMTT(30)
   DEF ReplyPipelineItemHasType,
       ReplyPipelineItemCoreBinding,
       ReplyPipelineItemMatchesAttempt,
       ReplyAttemptComplete, ReplyAttemptCursor,
       SameReplyAttemptIdentity

THEOREM ReplyPipelineItemWithRouteTenurePreservesCoreBindingPrime ==
  \A item, connectionTenure:
    ReplyPipelineItemCoreBinding(item)' =>
      ReplyPipelineItemCoreBinding(
        ReplyPipelineItemWithRouteTenure(
          item, connectionTenure))'
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(20)
   DEF ReplyPipelineItemCoreBinding,
       ReplyPipelineItemMatchesAttempt

THEOREM ReplyAttachmentRouteRebindPreservesItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
   \A connectionTenure:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ (\A item \in rpItems:
          ReplyPipelineItemHasType(item))
    /\ (\A item \in rpItems:
          ReplyPipelineItemCoreBinding(item))
    /\ ReplyAttachmentRouteAction(owner, semantic, source, kind)
    => \A item \in ReplyPipelineItemsAfterRouteRebind(
                     owner, semantic, source, connectionTenure):
         ReplyPipelineItemCoreBinding(item)'
BY ReplyAttachmentRouteActionPreservesExistingItemCoreBinding,
   ReplyPipelineItemWithRouteTenurePreservesCoreBindingPrime,
   SMTT(40)
   DEF ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyAttachmentRouteRebindPreservesItemPhaseBinding ==
  \A owner, semantic, source, connectionTenure:
    /\ (\A item \in rpItems:
          ReplyPipelineItemPhaseBinding(item))
    /\ rpItems' = ReplyPipelineItemsAfterRouteRebind(
                    owner, semantic, source, connectionTenure)
    /\ rpNextTicketId' = rpNextTicketId
    => \A item \in rpItems':
         ReplyPipelineItemPhaseBinding(item)'
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(60)
   DEF ReplyPipelineItemPhaseBinding,
       ReplyPipelineItemsAfterRouteRebind,
       ReplyPipelineQueuedItem, ReplyPipelineTicketValid,
       ReplyPipelineItemIsFifoHead, ReplyPipelineItemsInLane

THEOREM ReplyAttachmentExactPreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
   \A item:
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ selected.kind = "Exact"
    /\ ReplyPipelineItemRouteBinding(item)
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         rpPendingAttachments \ {selected}
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                NEW item,
                selected \in rpPendingAttachments,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                selected.kind = "Exact",
                ReplyPipelineItemRouteBinding(item),
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  rpPendingAttachments \ {selected}
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             RetryExactReplySource
    <2>2. ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))' <=>
           ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))
      BY <2>1, ReplyRouteStutterPreservesAttemptCurrentView
    <2>3. ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)' <=>
           ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)
      BY <1>1, SMTT(20)
         DEF ReplyRouteRebindPending,
             ReplyPendingAttachmentsFor
    <2> QED BY <1>1, <2>2, <2>3, SMTT(5)
         DEF ReplyPipelineItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyObserveNewPreservesOwnedAttemptCurrent ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     checkedOwner \in ReplyOwners,
     checkedSemantic \in ReplySemantics,
     checkedSource \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyAttemptOwned(
         checkedOwner, checkedSemantic, checkedSource)
    /\ ObserveNewReplySource(owner, semantic, source)
    => (ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource))' <=>
        ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource)))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSemantic \in ReplySemantics,
                NEW checkedSource \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyAttemptOwned(
                  checkedOwner, checkedSemantic, checkedSource),
                ObserveNewReplySource(owner, semantic, source)
         PROVE ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))' <=>
                 ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))
    <2>1. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
             \in ReplyAttemptsForSource(
                   checkedOwner, checkedSemantic, checkedSource)
      BY <1>1, SMTT(10)
         DEF ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. /\ rrAttempts \subseteq rrAttempts'
           /\ rrConnectionTenure' = rrConnectionTenure
           /\ rrSourceActive' = rrSourceActive
      BY <1>1, SMTT(10)
         DEF ObserveNewReplySource
    <2>3. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
             \in ReplyAttemptsForSource(
                   checkedOwner, checkedSemantic, checkedSource)'
      BY <2>1, <2>2, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. ReplyAttemptOwned(
             checkedOwner, checkedSemantic, checkedSource)'
      BY <2>3 DEF ReplyAttemptOwned
    <2>5. ReplyAttemptsForSource(
             checkedOwner, checkedSemantic, checkedSource)' =
           {ReplyAttemptFor(
              checkedOwner, checkedSemantic, checkedSource)'}
      BY <1>1, <2>4,
         ReplyRouteSafetyOwnedAttemptSingletonPrime
    <2>6. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)' =
           ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
      BY <2>3, <2>5, SMTT(5)
    <2> QED BY <2>2, <2>6, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyAttachmentNewPreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
   \A item:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyPipelineItemHasType(item)
    /\ ReplyPipelineItemCoreBinding(item)
    /\ ReplyPipelineItemRouteBinding(item)
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ selected.kind = "New"
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         rpPendingAttachments \ {selected}
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                NEW item,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyPipelineItemHasType(item),
                ReplyPipelineItemCoreBinding(item),
                ReplyPipelineItemRouteBinding(item),
                selected \in rpPendingAttachments,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                selected.kind = "New",
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  rpPendingAttachments \ {selected}
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. /\ item.owner \in ReplyOwners
           /\ item.semantic \in ReplySemantics
           /\ item.source \in ReplySources
           /\ ReplyAttemptOwned(
                item.owner, item.semantic, item.source)
      BY <1>1
         DEF ReplyPipelineItemHasType,
             ReplyPipelineItemCoreBinding
    <2>2. ObserveNewReplySource(owner, semantic, source)
      BY <1>1, SMTT(5)
         DEF ReplyAttachmentRouteAction
    <2>3. ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))' <=>
           ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))
      BY <1>1, <2>1, <2>2,
         ReplyObserveNewPreservesOwnedAttemptCurrent
    <2>4. ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)' <=>
           ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)
      BY <1>1, SMTT(20)
         DEF ReplyRouteRebindPending,
             ReplyPendingAttachmentsFor
    <2> QED BY <1>1, <2>3, <2>4, SMTT(5)
         DEF ReplyPipelineItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyAttemptRouteUpdateUsesConnectionTenure ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    ReplyAttemptWithRoute(
      attempt, deliveryOrdinal, connectionTenure).connectionTenure =
        connectionTenure
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                NEW deliveryOrdinal \in ReplyDeliveryOrdinals,
                NEW connectionTenure \in ReplyConnectionTenures
         PROVE ReplyAttemptWithRoute(
                 attempt,
                 deliveryOrdinal,
                 connectionTenure).connectionTenure =
                   connectionTenure
    <2>1. CASE connectionTenure = attempt.connectionTenure
      BY <1>1, <2>1, SMTT(15)
         DEF ReplyAttemptWithRoute, ReplyAttemptSet,
             ReplyDeliveryOrdinals, ReplyConnectionTenures
    <2>2. CASE connectionTenure # attempt.connectionTenure
      BY <1>1, <2>2, SMTT(30)
         DEF ReplyAttemptWithRoute, ReplyAttemptSet,
             ReplyDeliveryOrdinals, ReplyConnectionTenures
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyObserveLaterSelectedAttemptCurrent ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ObserveLaterReplyDelivery(owner, semantic, source)
    => ReplyAttemptCurrent(
         ReplyAttemptFor(owner, semantic, source))'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ObserveLaterReplyDelivery(owner, semantic, source)
         PROVE ReplyAttemptCurrent(
                 ReplyAttemptFor(owner, semantic, source))'
    <2>1. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ oldAttempt \in ReplyAttemptSet
              /\ oldAttempt.owner = owner
              /\ oldAttempt.semantic = semantic
              /\ oldAttempt.source = source
              /\ rrNextDeliveryOrdinal[owner]
                   \in ReplyDeliveryOrdinals
              /\ rrConnectionTenure[owner][source]
                   \in ReplyConnectionTenures
      BY <1>1, SMTT(20)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ObserveLaterReplyDelivery,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   rrConnectionTenure[owner][source])
           IN /\ routed.owner = owner
              /\ routed.semantic = semantic
              /\ routed.source = source
              /\ routed.connectionTenure =
                   rrConnectionTenure[owner][source]
      BY <2>1,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         ReplyAttemptRouteUpdateUsesConnectionTenure,
         SMTT(20)
         DEF SameReplyAttemptIdentity
    <2>3. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   rrConnectionTenure[owner][source])
           IN /\ routed \in rrAttempts'
              /\ rrConnectionTenure' = rrConnectionTenure
              /\ rrSourceActive' = rrSourceActive
              /\ rrSourceActive[owner][source]
      BY <1>1, SMTT(20)
         DEF ObserveLaterReplyDelivery,
             ReplaceReplyAttempt
    <2>4. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   rrConnectionTenure[owner][source])
           IN routed \in ReplyAttemptsForSource(
                          owner, semantic, source)'
      BY <2>2, <2>3, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>5. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>4 DEF ReplyAttemptOwned
    <2>6. ReplyAttemptsForSource(owner, semantic, source)' =
           {ReplyAttemptFor(owner, semantic, source)'}
      BY <1>1, <2>5,
         ReplyRouteSafetyOwnedAttemptSingletonPrime
    <2>7. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   rrConnectionTenure[owner][source])
           IN ReplyAttemptFor(owner, semantic, source)' = routed
      BY <2>4, <2>6, SMTT(5)
    <2> QED BY <2>2, <2>3, <2>7, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyObserveLaterPreservesOtherAttemptCurrent ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     checkedOwner \in ReplyOwners,
     checkedSemantic \in ReplySemantics,
     checkedSource \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyAttemptOwned(
         checkedOwner, checkedSemantic, checkedSource)
    /\ ObserveLaterReplyDelivery(owner, semantic, source)
    /\ (checkedOwner # owner
         \/ checkedSemantic # semantic
         \/ checkedSource # source)
    => (ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource))' <=>
        ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource)))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSemantic \in ReplySemantics,
                NEW checkedSource \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyAttemptOwned(
                  checkedOwner, checkedSemantic, checkedSource),
                ObserveLaterReplyDelivery(owner, semantic, source),
                checkedOwner # owner
                  \/ checkedSemantic # semantic
                  \/ checkedSource # source
         PROVE ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))' <=>
                 ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))
    <2>1. LET selectedAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               checkedAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN /\ selectedAttempt \in rrAttempts
              /\ selectedAttempt.owner = owner
              /\ selectedAttempt.semantic = semantic
              /\ selectedAttempt.source = source
              /\ checkedAttempt \in rrAttempts
              /\ checkedAttempt.owner = checkedOwner
              /\ checkedAttempt.semantic = checkedSemantic
              /\ checkedAttempt.source = checkedSource
              /\ checkedAttempt # selectedAttempt
      BY <1>1, SMTT(30)
         DEF ObserveLaterReplyDelivery,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. LET selectedAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               checkedAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN /\ checkedAttempt \in rrAttempts'
              /\ rrConnectionTenure' = rrConnectionTenure
              /\ rrSourceActive' = rrSourceActive
      BY <1>1, <2>1, SMTT(15)
         DEF ObserveLaterReplyDelivery,
             ReplaceReplyAttempt
    <2>3. LET checkedAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN checkedAttempt \in ReplyAttemptsForSource(
                                  checkedOwner,
                                  checkedSemantic,
                                  checkedSource)'
      BY <2>1, <2>2, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. ReplyAttemptOwned(
             checkedOwner, checkedSemantic, checkedSource)'
      BY <2>3 DEF ReplyAttemptOwned
    <2>5. ReplyAttemptsForSource(
             checkedOwner, checkedSemantic, checkedSource)' =
           {ReplyAttemptFor(
              checkedOwner, checkedSemantic, checkedSource)'}
      BY <1>1, <2>4,
         ReplyRouteSafetyOwnedAttemptSingletonPrime
    <2>6. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)' =
           ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
      BY <2>3, <2>5, SMTT(5)
    <2> QED BY <2>2, <2>6, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyPendingRemovalPreservesOtherRouteRebind ==
  \A selected \in ReplyAttachmentSet:
   \A checkedOwner, checkedSemantic, checkedSource:
    /\ selected \in rpPendingAttachments
    /\ (checkedOwner # selected.owner
         \/ checkedSemantic # selected.semantic
         \/ checkedSource # selected.source)
    /\ ReplyRouteRebindPending(
         checkedOwner, checkedSemantic, checkedSource)
    /\ rpPendingAttachments' =
         rpPendingAttachments \ {selected}
    => ReplyRouteRebindPending(
         checkedOwner, checkedSemantic, checkedSource)'
BY SMTT(30)
   DEF ReplyRouteRebindPending,
       ReplyPendingAttachmentsFor

THEOREM ReplyAttachmentLaterPreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
   \A item:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyPipelineItemHasType(item)
    /\ ReplyPipelineItemCoreBinding(item)
    /\ ReplyPipelineItemRouteBinding(item)
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ selected.kind = "Later"
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         rpPendingAttachments \ {selected}
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                NEW item,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyPipelineItemHasType(item),
                ReplyPipelineItemCoreBinding(item),
                ReplyPipelineItemRouteBinding(item),
                selected \in rpPendingAttachments,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                selected.kind = "Later",
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  rpPendingAttachments \ {selected}
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. /\ item.owner \in ReplyOwners
           /\ item.semantic \in ReplySemantics
           /\ item.source \in ReplySources
           /\ ReplyAttemptOwned(
                item.owner, item.semantic, item.source)
           /\ ObserveLaterReplyDelivery(owner, semantic, source)
      BY <1>1, SMTT(10)
         DEF ReplyPipelineItemHasType,
             ReplyPipelineItemCoreBinding,
             ReplyAttachmentRouteAction
    <2>2. CASE /\ item.owner = owner
                /\ item.semantic = semantic
                /\ item.source = source
      BY <1>1, <2>1, <2>2,
         ReplyObserveLaterSelectedAttemptCurrent,
         SMTT(5)
         DEF ReplyPipelineItemRouteBinding
    <2>3. CASE item.owner # owner
                \/ item.semantic # semantic
                \/ item.source # source
      <3>1. ReplyAttemptCurrent(
               ReplyAttemptFor(
                 item.owner, item.semantic, item.source))' <=>
             ReplyAttemptCurrent(
               ReplyAttemptFor(
                 item.owner, item.semantic, item.source))
        BY <1>1, <2>1, <2>3,
           ReplyObserveLaterPreservesOtherAttemptCurrent
      <3>2. ReplyRouteRebindPending(
               item.owner, item.semantic, item.source) =>
             ReplyRouteRebindPending(
               item.owner, item.semantic, item.source)'
        BY <1>1, <2>3,
           ReplyPendingRemovalPreservesOtherRouteRebind,
           SMTT(5)
      <3> QED BY <1>1, <3>1, <3>2, SMTT(5)
           DEF ReplyPipelineItemRouteBinding
    <2> QED BY <2>2, <2>3, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyReconnectAttachPreservesOtherRouteRebind ==
  \A selected \in ReplyAttachmentSet:
   \A checkedOwner, checkedSemantic, checkedSource:
    /\ selected \in rpPendingAttachments
    /\ (checkedOwner # selected.owner
         \/ checkedSemantic # selected.semantic
         \/ checkedSource # selected.source)
    /\ ReplyRouteRebindPending(
         checkedOwner, checkedSemantic, checkedSource)
    /\ rpPendingAttachments' =
         ReplyPendingAfterReconnectAttach(selected)
    => ReplyRouteRebindPending(
         checkedOwner, checkedSemantic, checkedSource)'
BY ReplyPendingAttachmentAsLaterProjection, SMTT(60)
   DEF ReplyPendingAfterReconnectAttach,
       ReplyRouteRebindPending,
       ReplyPendingAttachmentsFor

THEOREM ReplyReconnectSelectedAttemptCurrent ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReconnectReplySource(owner, semantic, source)
    => ReplyAttemptCurrent(
         ReplyAttemptFor(owner, semantic, source))'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReconnectReplySource(owner, semantic, source)
         PROVE ReplyAttemptCurrent(
                 ReplyAttemptFor(owner, semantic, source))'
    <2>1. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               connectionTenure ==
                 rrConnectionTenure[owner][source] + 1
           IN /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ oldAttempt.owner = owner
              /\ oldAttempt.semantic = semantic
              /\ oldAttempt.source = source
              /\ rrNextDeliveryOrdinal[owner]
                   \in ReplyDeliveryOrdinals
              /\ connectionTenure \in ReplyConnectionTenures
      BY <1>1, SMTT(30)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReconnectReplySource,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               connectionTenure ==
                 rrConnectionTenure[owner][source] + 1
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   connectionTenure)
           IN /\ routed.owner = owner
              /\ routed.semantic = semantic
              /\ routed.source = source
              /\ routed.connectionTenure = connectionTenure
      BY <2>1,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         ReplyAttemptRouteUpdateUsesConnectionTenure,
         SMTT(20)
         DEF SameReplyAttemptIdentity
    <2>3. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               connectionTenure ==
                 rrConnectionTenure[owner][source] + 1
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   connectionTenure)
           IN routed \in rrAttempts'
      BY <1>1, <2>1, SMTT(30)
         DEF ReconnectReplySource,
             ReplyAttemptsAfterReconnect
    <2>4. LET connectionTenure ==
                 rrConnectionTenure[owner][source] + 1
           IN /\ rrConnectionTenure'[owner][source] =
                    connectionTenure
              /\ rrSourceActive'[owner][source] = TRUE
      BY <1>1, SMTT(20)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReconnectReplySource
    <2>5. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               connectionTenure ==
                 rrConnectionTenure[owner][source] + 1
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   connectionTenure)
           IN routed \in ReplyAttemptsForSource(
                          owner, semantic, source)'
      BY <2>2, <2>3, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>6. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>5 DEF ReplyAttemptOwned
    <2>7. ReplyAttemptsForSource(owner, semantic, source)' =
           {ReplyAttemptFor(owner, semantic, source)'}
      BY <1>1, <2>6,
         ReplyRouteSafetyOwnedAttemptSingletonPrime
    <2>8. LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               connectionTenure ==
                 rrConnectionTenure[owner][source] + 1
               routed ==
                 ReplyAttemptWithRoute(
                   oldAttempt, rrNextDeliveryOrdinal[owner],
                   connectionTenure)
           IN ReplyAttemptFor(owner, semantic, source)' = routed
      BY <2>5, <2>7, SMTT(5)
    <2> QED BY <2>2, <2>4, <2>8, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyReconnectPreservesOtherSourceAttemptCurrent ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     checkedOwner \in ReplyOwners,
     checkedSemantic \in ReplySemantics,
     checkedSource \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyAttemptOwned(
         checkedOwner, checkedSemantic, checkedSource)
    /\ ReconnectReplySource(owner, semantic, source)
    /\ (checkedOwner # owner \/ checkedSource # source)
    => (ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource))' <=>
        ReplyAttemptCurrent(
          ReplyAttemptFor(
            checkedOwner, checkedSemantic, checkedSource)))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSemantic \in ReplySemantics,
                NEW checkedSource \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyAttemptOwned(
                  checkedOwner, checkedSemantic, checkedSource),
                ReconnectReplySource(owner, semantic, source),
                checkedOwner # owner \/ checkedSource # source
         PROVE ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))' <=>
                 ReplyAttemptCurrent(
                   ReplyAttemptFor(
                     checkedOwner,
                     checkedSemantic,
                     checkedSource))
    <2>1. LET selectedAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               checkedAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN /\ selectedAttempt \in rrAttempts
              /\ selectedAttempt.owner = owner
              /\ selectedAttempt.semantic = semantic
              /\ selectedAttempt.source = source
              /\ checkedAttempt \in rrAttempts
              /\ checkedAttempt.owner = checkedOwner
              /\ checkedAttempt.semantic = checkedSemantic
              /\ checkedAttempt.source = checkedSource
              /\ checkedAttempt # selectedAttempt
              /\ (checkedAttempt.owner # selectedAttempt.owner
                   \/ checkedAttempt.source # selectedAttempt.source)
      BY <1>1, SMTT(30)
         DEF ReconnectReplySource,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. LET checkedAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN checkedAttempt \in rrAttempts'
      BY <1>1, <2>1,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         SMTT(40)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReconnectReplySource,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptWithoutTicket,
             ReplyAttemptSet
    <2>3. LET checkedAttempt ==
                 ReplyAttemptFor(
                   checkedOwner, checkedSemantic, checkedSource)
           IN checkedAttempt \in ReplyAttemptsForSource(
                                  checkedOwner,
                                  checkedSemantic,
                                  checkedSource)'
      BY <2>1, <2>2, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. ReplyAttemptOwned(
             checkedOwner, checkedSemantic, checkedSource)'
      BY <2>3 DEF ReplyAttemptOwned
    <2>5. ReplyAttemptsForSource(
             checkedOwner, checkedSemantic, checkedSource)' =
           {ReplyAttemptFor(
              checkedOwner, checkedSemantic, checkedSource)'}
      BY <1>1, <2>4,
         ReplyRouteSafetyOwnedAttemptSingletonPrime
    <2>6. ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)' =
           ReplyAttemptFor(
             checkedOwner, checkedSemantic, checkedSource)
      BY <2>3, <2>5, SMTT(5)
    <2>7. /\ rrConnectionTenure'[checkedOwner][checkedSource] =
                rrConnectionTenure[checkedOwner][checkedSource]
           /\ rrSourceActive'[checkedOwner][checkedSource] =
                rrSourceActive[checkedOwner][checkedSource]
      <3>1. CASE checkedOwner # owner
        BY <1>1, <3>1,
           ReplyNestedTenureUpdatePreservesOtherOwner,
           ReplyNestedActiveUpdatePreservesOtherOwner,
           SMTT(20)
           DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReconnectReplySource
      <3>2. CASE /\ checkedOwner = owner
                  /\ checkedSource # source
        BY <1>1, <3>2,
           ReplyNestedTenureUpdatePreservesOtherSource,
           ReplyNestedActiveUpdatePreservesOtherSource,
           SMTT(20)
           DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReconnectReplySource
      <3> QED BY <1>1, <3>1, <3>2, SMTT(5)
    <2> QED BY <2>1, <2>6, <2>7, SMTT(10)
         DEF ReplyAttemptCurrent
  <1> QED BY <1>1

THEOREM ReplyAttachmentReconnectPreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
   \A item:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplyPipelineItemHasType(item)
    /\ ReplyPipelineItemCoreBinding(item)
    /\ ReplyPipelineItemRouteBinding(item)
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ selected.kind = "Reconnect"
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         ReplyPendingAfterReconnectAttach(selected)
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                NEW item,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplyPipelineItemHasType(item),
                ReplyPipelineItemCoreBinding(item),
                ReplyPipelineItemRouteBinding(item),
                selected \in rpPendingAttachments,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                selected.kind = "Reconnect",
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  ReplyPendingAfterReconnectAttach(selected)
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. /\ item.owner \in ReplyOwners
           /\ item.semantic \in ReplySemantics
           /\ item.source \in ReplySources
           /\ ReplyAttemptOwned(
                item.owner, item.semantic, item.source)
           /\ ReplyAttemptFor(
                item.owner, item.semantic, item.source).owner =
                item.owner
           /\ ReplyAttemptFor(
                item.owner, item.semantic, item.source).semantic =
                item.semantic
           /\ ReplyAttemptFor(
                item.owner, item.semantic, item.source).source =
                item.source
           /\ ReconnectReplySource(owner, semantic, source)
      BY <1>1, SMTT(20)
         DEF ReplyPipelineItemHasType,
             ReplyPipelineItemCoreBinding,
             ReplyAttachmentRouteAction,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. CASE /\ item.owner = owner
                /\ item.semantic = semantic
                /\ item.source = source
      BY <1>1, <2>1, <2>2,
         ReplyReconnectSelectedAttemptCurrent,
         SMTT(5)
         DEF ReplyPipelineItemRouteBinding
    <2>3. CASE item.owner # owner \/ item.source # source
      <3>1. ReplyAttemptCurrent(
               ReplyAttemptFor(
                 item.owner, item.semantic, item.source))' <=>
             ReplyAttemptCurrent(
               ReplyAttemptFor(
                 item.owner, item.semantic, item.source))
        BY <1>1, <2>1, <2>3,
           ReplyReconnectPreservesOtherSourceAttemptCurrent
      <3>2. ReplyRouteRebindPending(
               item.owner, item.semantic, item.source) =>
             ReplyRouteRebindPending(
               item.owner, item.semantic, item.source)'
        BY <1>1, <2>3,
           ReplyReconnectAttachPreservesOtherRouteRebind,
           SMTT(5)
      <3> QED BY <1>1, <3>1, <3>2, SMTT(5)
           DEF ReplyPipelineItemRouteBinding
    <2>4. CASE /\ item.owner = owner
                /\ item.source = source
                /\ item.semantic # semantic
      <3>1. ~ReplyAttemptCurrent(
               ReplyAttemptFor(
                 item.owner, item.semantic, item.source))
        BY <2>1, <2>4, SMTT(10)
           DEF ReconnectReplySource,
               ReplyAttemptCurrent
      <3>2. ReplyRouteRebindPending(
               item.owner, item.semantic, item.source)
        BY <1>1, <3>1
           DEF ReplyPipelineItemRouteBinding
      <3>3. ReplyRouteRebindPending(
               item.owner, item.semantic, item.source)'
        BY <1>1, <2>4, <3>2,
           ReplyReconnectAttachPreservesOtherRouteRebind,
           SMTT(5)
      <3> QED BY <3>3
           DEF ReplyPipelineItemRouteBinding
    <2> QED BY <2>2, <2>3, <2>4, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
   \A item \in rpItems:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ ReplyPipelineItemHasType(item)
    /\ ReplyPipelineItemCoreBinding(item)
    /\ ReplyPipelineItemRouteBinding(item)
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                NEW item \in rpItems,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                selected \in rpPendingAttachments,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                ReplyPipelineItemHasType(item),
                ReplyPipelineItemCoreBinding(item),
                ReplyPipelineItemRouteBinding(item),
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  IF selected.kind = "Reconnect"
                  THEN ReplyPendingAfterReconnectAttach(selected)
                  ELSE rpPendingAttachments \ {selected}
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. CASE selected.kind = "Exact"
      BY <1>1, <2>1,
         ReplyAttachmentExactPreservesExistingItemRouteBinding,
         SMTT(5)
    <2>2. CASE selected.kind = "New"
      BY <1>1, <2>2,
         ReplyAttachmentNewPreservesExistingItemRouteBinding,
         SMTT(5)
    <2>3. CASE selected.kind = "Later"
      BY <1>1, <2>3,
         ReplyAttachmentLaterPreservesExistingItemRouteBinding,
         SMTT(5)
    <2>4. CASE selected.kind = "Reconnect"
      BY <1>1, <2>4,
         ReplyAttachmentReconnectPreservesExistingItemRouteBinding,
         SMTT(5)
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ReplyAttachmentSet, ReplyAttachmentKinds
  <1> QED BY <1>1

THEOREM ReplyPipelineItemWithRouteTenurePreservesRouteBindingPrime ==
  \A item, connectionTenure:
    ReplyPipelineItemRouteBinding(item)' <=>
      ReplyPipelineItemRouteBinding(
        ReplyPipelineItemWithRouteTenure(
          item, connectionTenure))'
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(10)
   DEF ReplyPipelineItemRouteBinding

THEOREM ReplyAttachmentRouteRebindPreservesItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
   \A connectionTenure:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ (\A item \in rpItems:
          ReplyPipelineItemHasType(item))
    /\ (\A item \in rpItems:
          ReplyPipelineItemCoreBinding(item))
    /\ (\A item \in rpItems:
          ReplyPipelineItemRouteBinding(item))
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    => \A item \in ReplyPipelineItemsAfterRouteRebind(
                     owner, semantic, source, connectionTenure):
         ReplyPipelineItemRouteBinding(item)'
BY ReplyAttachmentLocalEffectPreservesExistingItemRouteBinding,
   ReplyPipelineItemWithRouteTenurePreservesRouteBindingPrime,
   SMTT(50)
   DEF ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyAttachmentLocalEffectPreservesItemBindingInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ (\A item \in rpItems:
          ReplyPipelineItemHasType(item))
    /\ ReplyPipelineItemBindingInvariant
    /\ selected \in rpPendingAttachments
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    /\ rpNextTicketId' = rpNextTicketId
    => ReplyPipelineItemBindingInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                (\A item \in rpItems:
                   ReplyPipelineItemHasType(item)),
                ReplyPipelineItemBindingInvariant,
                selected \in rpPendingAttachments,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  IF selected.kind = "Reconnect"
                  THEN ReplyPendingAfterReconnectAttach(selected)
                  ELSE rpPendingAttachments \ {selected},
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems,
                rpNextTicketId' = rpNextTicketId
         PROVE ReplyPipelineItemBindingInvariant'
    <2>1. CASE selected.kind \in {"Later", "Reconnect"}
      <3>1. \A item \in rpItems':
               ReplyPipelineItemCoreBinding(item)'
        BY <1>1, <2>1,
           ReplyAttachmentRouteRebindPreservesItemCoreBinding,
           SMTT(20)
           DEF ReplyPipelineItemBindingInvariant,
               ReplyAttachmentSet, ReplyAttachmentKinds
      <3>2. \A item \in rpItems':
               ReplyPipelineItemRouteBinding(item)'
        BY <1>1, <2>1,
           ReplyAttachmentRouteRebindPreservesItemRouteBinding,
           SMTT(20)
           DEF ReplyPipelineItemBindingInvariant
      <3>3. \A item \in rpItems':
               ReplyPipelineItemPhaseBinding(item)'
        BY <1>1, <2>1,
           ReplyAttachmentRouteRebindPreservesItemPhaseBinding,
           SMTT(20)
           DEF ReplyPipelineItemBindingInvariant
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyPipelineItemBindingInvariant
    <2>2. CASE selected.kind \notin {"Later", "Reconnect"}
      <3>1. \A item \in rpItems':
               ReplyPipelineItemCoreBinding(item)'
        BY <1>1, <2>2,
           ReplyAttachmentRouteActionPreservesExistingItemCoreBinding,
           SMTT(20)
           DEF ReplyPipelineItemBindingInvariant,
               ReplyAttachmentSet, ReplyAttachmentKinds
      <3>2. \A item \in rpItems':
               ReplyPipelineItemRouteBinding(item)'
        BY <1>1, <2>2,
           ReplyAttachmentLocalEffectPreservesExistingItemRouteBinding,
           SMTT(20)
           DEF ReplyPipelineItemBindingInvariant
      <3>3. \A item \in rpItems':
               ReplyPipelineItemPhaseBinding(item)'
        BY <1>1, <2>2, SMTT(40)
           DEF ReplyPipelineItemBindingInvariant,
               ReplyPipelineItemPhaseBinding,
               ReplyPipelineQueuedItem,
               ReplyPipelineTicketValid,
               ReplyPipelineItemIsFifoHead,
               ReplyPipelineItemsInLane
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyPipelineItemBindingInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyPendingRemovalDoesNotCreateReconnect ==
  \A selected:
    /\ selected.kind # "Reconnect"
    /\ rpPendingAttachments' =
         rpPendingAttachments \ {selected}
    => \A checkedOwner \in ReplyOwners,
          checkedSource \in ReplySources:
         ReplyReconnectPendingForSource(
           checkedOwner, checkedSource)' =>
           ReplyReconnectPendingForSource(
             checkedOwner, checkedSource)
BY SMTT(30)
   DEF ReplyReconnectPendingForSource,
       ReplyReconnectPending,
       ReplyPendingAttachmentsFor

THEOREM ReplyReconnectAttachProjectsReconnectPending ==
  \A selected \in ReplyAttachmentSet:
    /\ selected.kind = "Reconnect"
    /\ rpPendingAttachments' =
         ReplyPendingAfterReconnectAttach(selected)
    => \A checkedOwner \in ReplyOwners,
          checkedSource \in ReplySources:
         ReplyReconnectPendingForSource(
           checkedOwner, checkedSource)' =>
           /\ (checkedOwner # selected.owner
                \/ checkedSource # selected.source)
           /\ ReplyReconnectPendingForSource(
                checkedOwner, checkedSource)
BY ReplyPendingAttachmentAsLaterProjection, SMTT(60)
   DEF ReplyPendingAfterReconnectAttach,
       ReplyReconnectPendingForSource,
       ReplyReconnectPending,
       ReplyPendingAttachmentsFor

THEOREM ReplyPipelineRouteRebindPreservesSourcePhaseExclusion ==
  \A owner, semantic, source, connectionTenure,
     checkedOwner, checkedSource, excludedPhase:
    (\A item \in rpItems:
       (item.owner = checkedOwner
          /\ item.source = checkedSource) =>
         item.phase # excludedPhase)
    => \A item \in ReplyPipelineItemsAfterRouteRebind(
                     owner, semantic, source, connectionTenure):
         (item.owner = checkedOwner
           /\ item.source = checkedSource) =>
           item.phase # excludedPhase
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(40)
   DEF ReplyPipelineItemsAfterRouteRebind

THEOREM ReplyPipelineRouteRebindPreservesUnresolvedWriter ==
  \A owner, semantic, source, connectionTenure,
     checkedOwner, checkedSource:
    rpItems' = ReplyPipelineItemsAfterRouteRebind(
                 owner, semantic, source, connectionTenure)
    => (ReplyPipelineHasUnresolvedWriter(
          checkedOwner, checkedSource)' <=>
        ReplyPipelineHasUnresolvedWriter(
          checkedOwner, checkedSource))
BY ReplyPipelineItemWithRouteTenureProjection, SMTT(50)
   DEF ReplyPipelineItemsAfterRouteRebind,
       ReplyPipelineHasUnresolvedWriter

THEOREM ReplyAttachmentItemsPreserveSourcePhaseExclusion ==
  \A owner, semantic, source, selected,
     checkedOwner, checkedSource, excludedPhase:
    /\ (\A item \in rpItems:
          (item.owner = checkedOwner
            /\ item.source = checkedSource) =>
            item.phase # excludedPhase)
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    => \A item \in rpItems':
         (item.owner = checkedOwner
           /\ item.source = checkedSource) =>
           item.phase # excludedPhase
BY ReplyPipelineRouteRebindPreservesSourcePhaseExclusion,
   SMTT(40)

THEOREM ReplyAttachmentItemsPreserveUnresolvedWriter ==
  \A owner, semantic, source, selected,
     checkedOwner, checkedSource:
    (rpItems' =
       IF selected.kind \in {"Later", "Reconnect"}
       THEN ReplyPipelineItemsAfterRouteRebind(
              owner, semantic, source,
              rrConnectionTenure'[owner][source])
       ELSE rpItems)
    => (ReplyPipelineHasUnresolvedWriter(
          checkedOwner, checkedSource)' <=>
        ReplyPipelineHasUnresolvedWriter(
          checkedOwner, checkedSource))
PROOF
  <1>1. ASSUME NEW owner,
                NEW semantic,
                NEW source,
                NEW selected,
                NEW checkedOwner,
                NEW checkedSource,
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems
         PROVE ReplyPipelineHasUnresolvedWriter(
                 checkedOwner, checkedSource)' <=>
               ReplyPipelineHasUnresolvedWriter(
                 checkedOwner, checkedSource)
    <2>1. CASE selected.kind \in {"Later", "Reconnect"}
      BY <1>1, <2>1,
         ReplyPipelineRouteRebindPreservesUnresolvedWriter,
         SMTT(10)
    <2>2. CASE selected.kind \notin {"Later", "Reconnect"}
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyPipelineHasUnresolvedWriter
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyAttachmentNonReconnectPreservesSourceActivity ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ kind # "Reconnect"
    /\ ReplyAttachmentRouteAction(owner, semantic, source, kind)
    => rrSourceActive' = rrSourceActive
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                kind # "Reconnect",
                ReplyAttachmentRouteAction(
                  owner, semantic, source, kind)
         PROVE rrSourceActive' = rrSourceActive
    <2>1. CASE kind = "New"
      BY <1>1, <2>1, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             ObserveNewReplySource
    <2>2. CASE kind = "Exact"
      BY <1>1, <2>2, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             RetryExactReplySource, ReplyRouteVars
    <2>3. CASE kind = "Later"
      BY <1>1, <2>3, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             ObserveLaterReplyDelivery
    <2> QED BY <1>1, <2>1, <2>2, <2>3
         DEF ReplyAttachmentKinds
  <1> QED BY <1>1

THEOREM ReplyReconnectPreservesOtherSourceActivity ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     checkedOwner \in ReplyOwners,
     checkedSource \in ReplySources:
    /\ ReplyRouteTypeInvariant
    /\ ReconnectReplySource(owner, semantic, source)
    /\ (checkedOwner # owner \/ checkedSource # source)
    => rrSourceActive'[checkedOwner][checkedSource] =
         rrSourceActive[checkedOwner][checkedSource]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                NEW checkedSource \in ReplySources,
                ReplyRouteTypeInvariant,
                ReconnectReplySource(owner, semantic, source),
                checkedOwner # owner \/ checkedSource # source
         PROVE rrSourceActive'[checkedOwner][checkedSource] =
                 rrSourceActive[checkedOwner][checkedSource]
    <2>1. CASE checkedOwner # owner
      BY <1>1, <2>1,
         ReplyNestedActiveUpdatePreservesOtherOwner,
         SMTT(10)
         DEF ReplyRouteTypeInvariant,
             ReconnectReplySource
    <2>2. CASE /\ checkedOwner = owner
                /\ checkedSource # source
      BY <1>1, <2>2,
         ReplyNestedActiveUpdatePreservesOtherSource,
         SMTT(10)
         DEF ReplyRouteTypeInvariant,
             ReconnectReplySource
    <2> QED BY <1>1, <2>1, <2>2, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesReconnectNoTicketedInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyPipelineReconnectNoTicketedInvariant
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    => ReplyPipelineReconnectNoTicketedInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyPipelineReconnectNoTicketedInvariant,
                rpPendingAttachments' =
                  IF selected.kind = "Reconnect"
                  THEN ReplyPendingAfterReconnectAttach(selected)
                  ELSE rpPendingAttachments \ {selected},
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems
         PROVE ReplyPipelineReconnectNoTicketedInvariant'
    <2>1. CASE selected.kind = "Reconnect"
      <3>1. ASSUME NEW checkedOwner \in ReplyOwners,
                    NEW checkedSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      checkedOwner, checkedSource)'
             PROVE \A item \in rpItems':
                     (item.owner = checkedOwner
                       /\ item.source = checkedSource) =>
                       item.phase # "Ticketed"
        <4>1. ReplyReconnectPendingForSource(
                 checkedOwner, checkedSource)
          BY <1>1, <2>1, <3>1,
             ReplyReconnectAttachProjectsReconnectPending,
             SMTT(10)
        <4>2. \A item \in rpItems:
                 (item.owner = checkedOwner
                   /\ item.source = checkedSource) =>
                   item.phase # "Ticketed"
          BY <1>1, <4>1
             DEF ReplyPipelineReconnectNoTicketedInvariant
        <4> QED BY <1>1, <4>2,
                    ReplyAttachmentItemsPreserveSourcePhaseExclusion,
                    SMTT(15)
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectNoTicketedInvariant
    <2>2. CASE selected.kind # "Reconnect"
      <3>1. ASSUME NEW checkedOwner \in ReplyOwners,
                    NEW checkedSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      checkedOwner, checkedSource)'
             PROVE \A item \in rpItems':
                     (item.owner = checkedOwner
                       /\ item.source = checkedSource) =>
                       item.phase # "Ticketed"
        <4>1. ReplyReconnectPendingForSource(
                 checkedOwner, checkedSource)
          BY <1>1, <2>2, <3>1,
             ReplyPendingRemovalDoesNotCreateReconnect,
             SMTT(10)
        <4>2. \A item \in rpItems:
                 (item.owner = checkedOwner
                   /\ item.source = checkedSource) =>
                   item.phase # "Ticketed"
          BY <1>1, <4>1
             DEF ReplyPipelineReconnectNoTicketedInvariant
        <4> QED BY <1>1, <4>2,
                    ReplyAttachmentItemsPreserveSourcePhaseExclusion,
                    SMTT(15)
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectNoTicketedInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesReconnectWriterActiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyRouteTypeInvariant
    /\ ReplyPipelineReconnectWriterActiveInvariant
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    => ReplyPipelineReconnectWriterActiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW selected \in ReplyAttachmentSet,
                ReplyRouteTypeInvariant,
                ReplyPipelineReconnectWriterActiveInvariant,
                selected.owner = owner,
                selected.semantic = semantic,
                selected.source = source,
                ReplyAttachmentRouteAction(
                  owner, semantic, source, selected.kind),
                rpPendingAttachments' =
                  IF selected.kind = "Reconnect"
                  THEN ReplyPendingAfterReconnectAttach(selected)
                  ELSE rpPendingAttachments \ {selected},
                rpItems' =
                  IF selected.kind \in {"Later", "Reconnect"}
                  THEN ReplyPipelineItemsAfterRouteRebind(
                         owner, semantic, source,
                         rrConnectionTenure'[owner][source])
                  ELSE rpItems
         PROVE ReplyPipelineReconnectWriterActiveInvariant'
    <2>1. CASE selected.kind = "Reconnect"
      <3>1. ASSUME NEW checkedOwner \in ReplyOwners,
                    NEW checkedSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      checkedOwner, checkedSource)',
                    ReplyPipelineHasUnresolvedWriter(
                      checkedOwner, checkedSource)'
             PROVE rrSourceActive'[checkedOwner][checkedSource]
        <4>1. /\ (checkedOwner # selected.owner
                    \/ checkedSource # selected.source)
               /\ ReplyReconnectPendingForSource(
                    checkedOwner, checkedSource)
          BY <1>1, <2>1, <3>1,
             ReplyReconnectAttachProjectsReconnectPending,
             SMTT(10)
        <4>2. ReplyPipelineHasUnresolvedWriter(
                 checkedOwner, checkedSource)
          BY <1>1, <3>1,
             ReplyAttachmentItemsPreserveUnresolvedWriter,
             SMTT(10)
        <4>3. rrSourceActive[checkedOwner][checkedSource]
          BY <1>1, <4>1, <4>2
             DEF ReplyPipelineReconnectWriterActiveInvariant
        <4>4. ReconnectReplySource(owner, semantic, source)
          BY <1>1, <2>1, SMTT(5)
             DEF ReplyAttachmentRouteAction
        <4>5. checkedOwner # owner \/ checkedSource # source
          BY <1>1, <4>1, SMTT(5)
        <4>6. rrSourceActive'[checkedOwner][checkedSource] =
                 rrSourceActive[checkedOwner][checkedSource]
          BY <1>1, <4>4, <4>5,
             ReplyReconnectPreservesOtherSourceActivity
        <4> QED BY <4>3, <4>6
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectWriterActiveInvariant
    <2>2. CASE selected.kind # "Reconnect"
      <3>1. ASSUME NEW checkedOwner \in ReplyOwners,
                    NEW checkedSource \in ReplySources,
                    ReplyReconnectPendingForSource(
                      checkedOwner, checkedSource)',
                    ReplyPipelineHasUnresolvedWriter(
                      checkedOwner, checkedSource)'
             PROVE rrSourceActive'[checkedOwner][checkedSource]
        <4>1. ReplyReconnectPendingForSource(
                 checkedOwner, checkedSource)
          BY <1>1, <2>2, <3>1,
             ReplyPendingRemovalDoesNotCreateReconnect,
             SMTT(10)
        <4>2. ReplyPipelineHasUnresolvedWriter(
                 checkedOwner, checkedSource)
          BY <1>1, <3>1,
             ReplyAttachmentItemsPreserveUnresolvedWriter,
             SMTT(10)
        <4>3. rrSourceActive[checkedOwner][checkedSource]
          BY <1>1, <4>1, <4>2
             DEF ReplyPipelineReconnectWriterActiveInvariant
        <4>4. rrSourceActive' = rrSourceActive
          BY <1>1, <2>2,
             ReplyAttachmentNonReconnectPreservesSourceActivity,
             SMTT(10)
             DEF ReplyAttachmentSet, ReplyAttachmentKinds
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
           DEF ReplyPipelineReconnectWriterActiveInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyAttachmentLocalEffectPreservesReconnectBarrierInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, selected \in ReplyAttachmentSet:
    /\ ReplyRouteTypeInvariant
    /\ ReplyPipelineReconnectBarrierInvariant
    /\ selected.owner = owner
    /\ selected.semantic = semantic
    /\ selected.source = source
    /\ ReplyAttachmentRouteAction(
         owner, semantic, source, selected.kind)
    /\ rpPendingAttachments' =
         IF selected.kind = "Reconnect"
         THEN ReplyPendingAfterReconnectAttach(selected)
         ELSE rpPendingAttachments \ {selected}
    /\ rpItems' =
         IF selected.kind \in {"Later", "Reconnect"}
         THEN ReplyPipelineItemsAfterRouteRebind(
                owner, semantic, source,
                rrConnectionTenure'[owner][source])
         ELSE rpItems
    => ReplyPipelineReconnectBarrierInvariant'
BY ReplyAttachmentLocalEffectPreservesReconnectNoTicketedInvariant,
   ReplyAttachmentLocalEffectPreservesReconnectWriterActiveInvariant,
   SMTT(20)
   DEF ReplyPipelineReconnectBarrierInvariant

THEOREM ReplyAttachmentRouteActionPreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    /\ ReplyRouteInductiveInvariant
    /\ ReplyAttachmentRouteAction(owner, semantic, source, kind)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyRouteInductiveInvariant,
                ReplyAttachmentRouteAction(
                  owner, semantic, source, kind)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. CASE kind = "New"
      BY <1>1, <2>1,
         ObserveNewReplySourcePreservesInductiveInvariant,
         SMTT(5)
         DEF ReplyAttachmentRouteAction
    <2>2. CASE kind = "Exact"
      BY <1>1, <2>2,
         RetryExactReplySourcePreservesInductiveInvariant,
         SMTT(5)
         DEF ReplyAttachmentRouteAction
    <2>3. CASE kind = "Later"
      BY <1>1, <2>3,
         ObserveLaterReplyDeliveryPreservesInductiveInvariant,
         SMTT(5)
         DEF ReplyAttachmentRouteAction
    <2>4. CASE kind = "Reconnect"
      BY <1>1, <2>4,
         ReconnectReplySourcePreservesInductiveInvariant,
         SMTT(5)
         DEF ReplyAttachmentRouteAction
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ReplyAttachmentKinds
  <1> QED BY <1>1

THEOREM ReplyPendingAttachmentChoiceProjection ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyPendingAttachmentOwned(owner, semantic, source) =>
      LET selected ==
            ReplyPendingAttachmentFor(owner, semantic, source)
      IN /\ selected \in rpPendingAttachments
         /\ selected.owner = owner
         /\ selected.semantic = semantic
         /\ selected.source = source
BY SMTT(20)
   DEF ReplyPendingAttachmentOwned,
       ReplyPendingAttachmentFor,
       ReplyPendingAttachmentsFor

THEOREM ReplyAttachPendingDeliveryPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AttachPendingReplyDelivery(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AttachPendingReplyDelivery(owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1,
         ReplyAttachmentRouteActionPreservesInductiveInvariant,
         SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor,
             ReplyPipelineTypeInvariant,
             ReplyAttachmentSet
    <2>2. ReplyPipelineConfiguration'
      BY <1>1 DEF ReplyPipelineInductiveInvariant
    <2>3. ReplyPipelineTypeInvariant'
      BY <1>1, <2>1,
         ReplyAttachmentLocalEffectPreservesTypeInvariant,
         SMTT(15)
         DEF ReplyPipelineInductiveInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>4. ReplyPipelinePerIdentityInvariant'
      BY <1>1,
         ReplyAttachmentLocalEffectPreservesPerIdentityInvariant,
         SMTT(15)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor
    <2>5. ReplyPipelineFifoOrdinalInvariant'
      BY <1>1,
         ReplyAttachmentLocalEffectPreservesFifoOrdinalInvariant,
         SMTT(15)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor
    <2>6. ReplyPipelineTicketIdentityInvariant'
      BY <1>1,
         ReplyAttachmentLocalEffectPreservesTicketIdentityInvariant,
         SMTT(15)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor
    <2>7. ReplyPipelineItemBindingInvariant'
      <3>1. LET selected ==
                   ReplyPendingAttachmentFor(
                     owner, semantic, source)
             IN /\ selected \in rpPendingAttachments
                /\ selected.owner = owner
                /\ selected.semantic = semantic
                /\ selected.source = source
        BY <1>1, ReplyPendingAttachmentChoiceProjection
           DEF AttachPendingReplyDelivery
      <3> QED BY <1>1, <2>1, <3>1,
                    ReplyAttachmentLocalEffectPreservesItemBindingInvariant,
                    SMTT(25)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineTypeInvariant,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               AttachPendingReplyDelivery,
               ReplyPendingAttachmentFor
    <2>8. ReplyPipelineReconnectBarrierInvariant'
      <3>1. LET selected ==
                   ReplyPendingAttachmentFor(
                     owner, semantic, source)
             IN /\ selected \in rpPendingAttachments
                /\ selected.owner = owner
                /\ selected.semantic = semantic
                /\ selected.source = source
        BY <1>1, ReplyPendingAttachmentChoiceProjection
           DEF AttachPendingReplyDelivery
      <3> QED BY <1>1, <3>1,
                    ReplyAttachmentLocalEffectPreservesReconnectBarrierInvariant,
                    SMTT(25)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineTypeInvariant,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               AttachPendingReplyDelivery,
               ReplyPendingAttachmentFor
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                  <2>5, <2>6, <2>7, <2>8
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
  <1> QED BY <1>1

THEOREM ReplyOwnedAttemptHasType ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteTypeInvariant
    /\ ReplyAttemptOwned(owner, semantic, source)
    => ReplyAttemptFor(owner, semantic, source) \in ReplyAttemptSet
BY SMTT(30)
   DEF ReplyRouteTypeInvariant,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource

THEOREM ReplyOwnedAttemptIdentityProjection ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteTypeInvariant
    /\ ReplyAttemptOwned(owner, semantic, source)
    => LET attempt == ReplyAttemptFor(owner, semantic, source)
       IN /\ attempt \in ReplyAttemptSet
          /\ attempt.owner = owner
          /\ attempt.semantic = semantic
          /\ attempt.source = source
BY ReplyOwnedAttemptHasType, SMTT(20)
   DEF ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource

THEOREM ReplyEnqueueItemProjection ==
  \A owner, semantic, source:
    LET attempt == ReplyAttemptFor(owner, semantic, source)
        item ==
          ReplyPipelineItem(
            owner, semantic, source,
            attempt.messageCursor, attempt.chunkCursor,
            rpNextFifoOrdinal[owner], attempt.connectionTenure,
            "Queued", NoReplyPipelineTicket,
            NoReplyTicketTenure, {})
    IN /\ item.owner = owner
       /\ item.semantic = semantic
       /\ item.source = source
       /\ item.messageCursor = attempt.messageCursor
       /\ item.chunkCursor = attempt.chunkCursor
       /\ item.outputClass = ReplyItemClass(
              semantic, attempt.messageCursor, attempt.chunkCursor)
       /\ item.flushRequired = ReplyItemRequiresFlush(
              semantic, attempt.messageCursor, attempt.chunkCursor)
       /\ item.fifoOrdinal = rpNextFifoOrdinal[owner]
       /\ item.routeTenure = attempt.connectionTenure
       /\ item.phase = "Queued"
       /\ item.ticketId = NoReplyPipelineTicket
       /\ item.ticketTenure = NoReplyTicketTenure
       /\ item.ticketPayload = {}
BY SMTT(10)
   DEF ReplyPipelineItem, ReplyPipelineRawItem

ReplyEnqueuedPipelineItem(owner, semantic, source) ==
  LET attempt == ReplyAttemptFor(owner, semantic, source)
  IN ReplyPipelineItem(
       owner, semantic, source,
       attempt.messageCursor, attempt.chunkCursor,
       rpNextFifoOrdinal[owner], attempt.connectionTenure,
       "Queued", NoReplyPipelineTicket,
       NoReplyTicketTenure, {})

THEOREM ReplyEnqueuedPipelineItemProjection ==
  \A owner, semantic, source:
    LET attempt == ReplyAttemptFor(owner, semantic, source)
        item == ReplyEnqueuedPipelineItem(owner, semantic, source)
    IN /\ item.owner = owner
       /\ item.semantic = semantic
       /\ item.source = source
       /\ item.messageCursor = attempt.messageCursor
       /\ item.chunkCursor = attempt.chunkCursor
       /\ item.outputClass = ReplyItemClass(
              semantic, attempt.messageCursor, attempt.chunkCursor)
       /\ item.flushRequired = ReplyItemRequiresFlush(
              semantic, attempt.messageCursor, attempt.chunkCursor)
       /\ item.fifoOrdinal = rpNextFifoOrdinal[owner]
       /\ item.routeTenure = attempt.connectionTenure
       /\ item.phase = "Queued"
       /\ item.ticketId = NoReplyPipelineTicket
       /\ item.ticketTenure = NoReplyTicketTenure
       /\ item.ticketPayload = {}
BY ReplyEnqueueItemProjection
   DEF ReplyEnqueuedPipelineItem

THEOREM ReplyEnqueueItemHasType ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineConfiguration
    /\ ReplyRouteTypeInvariant
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    => LET attempt == ReplyAttemptFor(owner, semantic, source)
           item ==
             ReplyPipelineItem(
               owner, semantic, source,
               attempt.messageCursor, attempt.chunkCursor,
               rpNextFifoOrdinal[owner], attempt.connectionTenure,
               "Queued", NoReplyPipelineTicket,
               NoReplyTicketTenure, {})
       IN ReplyPipelineItemHasType(item)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineConfiguration,
                ReplyRouteTypeInvariant,
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE LET attempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   item ==
                     ReplyPipelineItem(
                       owner, semantic, source,
                       attempt.messageCursor, attempt.chunkCursor,
                       rpNextFifoOrdinal[owner],
                       attempt.connectionTenure,
                       "Queued", NoReplyPipelineTicket,
                       NoReplyTicketTenure, {})
               IN ReplyPipelineItemHasType(item)
    <2>1. LET attempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ attempt \in ReplyAttemptSet
              /\ attempt.owner = owner
              /\ attempt.semantic = semantic
              /\ attempt.source = source
      BY <1>1, ReplyOwnedAttemptIdentityProjection
         DEF EnqueueCurrentReplyItem
    <2>2. LET attempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ attempt.messageCursor \in 0..ReplyMessageCount
              /\ attempt.chunkCursor \in 0..ReplyChunkCount
              /\ attempt.connectionTenure
                   \in ReplyConnectionTenures
      BY <2>1, SMTT(10) DEF ReplyAttemptSet
    <2>3. LET attempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ ReplyItemClass(
                    semantic,
                    attempt.messageCursor,
                    attempt.chunkCursor) \in ReplyOutputClasses
              /\ ReplyItemRequiresFlush(
                    semantic,
                    attempt.messageCursor,
                    attempt.chunkCursor) \in BOOLEAN
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyPipelineConfiguration
    <2>4. rpNextFifoOrdinal[owner]
             \in 1..ReplyPipelineOrdinalLimit
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2>5. /\ "Queued" \in ReplyPipelinePhases
           /\ NoReplyPipelineTicket \in Nat
           /\ NoReplyTicketTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ {} \in SUBSET ReplyPipelinePayloads
      BY <1>1, SMTT(10)
         DEF ReplyPipelinePhases,
             NoReplyPipelineTicket, NoReplyTicketTenure,
             ReplyPipelineConfiguration, ReplyRouteConfiguration
    <2>6. LET attempt ==
                 ReplyAttemptFor(owner, semantic, source)
             item ==
               ReplyPipelineItem(
                 owner, semantic, source,
                 attempt.messageCursor, attempt.chunkCursor,
                 rpNextFifoOrdinal[owner],
                 attempt.connectionTenure,
                 "Queued", NoReplyPipelineTicket,
                 NoReplyTicketTenure, {})
           IN /\ item.owner = owner
              /\ item.semantic = semantic
              /\ item.source = source
              /\ item.messageCursor = attempt.messageCursor
              /\ item.chunkCursor = attempt.chunkCursor
              /\ item.outputClass = ReplyItemClass(
                     semantic,
                     attempt.messageCursor,
                     attempt.chunkCursor)
              /\ item.flushRequired = ReplyItemRequiresFlush(
                     semantic,
                     attempt.messageCursor,
                     attempt.chunkCursor)
              /\ item.fifoOrdinal = rpNextFifoOrdinal[owner]
              /\ item.routeTenure = attempt.connectionTenure
              /\ item.phase = "Queued"
              /\ item.ticketId = NoReplyPipelineTicket
              /\ item.ticketTenure = NoReplyTicketTenure
              /\ item.ticketPayload = {}
      BY ReplyEnqueueItemProjection
    <2> QED BY ONLY <1>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                       SMTT(10)
         DEF ReplyPipelineItemHasType
  <1> QED BY <1>1

THEOREM ReplyEnqueuePreservesExistingItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item:
      /\ ReplyPipelineItemCoreBinding(item)
      /\ EnqueueCurrentReplyItem(owner, semantic, source)
      => ReplyPipelineItemCoreBinding(item)'
BY ReplyRouteStutterPreservesItemCoreBinding, SMTT(10)
   DEF EnqueueCurrentReplyItem, ReplyRouteVars

THEOREM ReplyEnqueueNewItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item:
      /\ ReplyRouteTypeInvariant
      /\ EnqueueCurrentReplyItem(owner, semantic, source)
      /\ item = ReplyEnqueuedPipelineItem(
                   owner, semantic, source)
      => ReplyPipelineItemCoreBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item,
                ReplyRouteTypeInvariant,
                EnqueueCurrentReplyItem(owner, semantic, source),
                item = ReplyEnqueuedPipelineItem(
                         owner, semantic, source)
         PROVE ReplyPipelineItemCoreBinding(item)'
    <2>1. LET attempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ attempt.owner = owner
              /\ attempt.semantic = semantic
              /\ attempt.source = source
      BY <1>1, ReplyOwnedAttemptIdentityProjection
         DEF EnqueueCurrentReplyItem
    <2>2. ReplyPipelineItemCoreBinding(item)
      BY <1>1, <2>1,
         ReplyEnqueuedPipelineItemProjection, SMTT(20)
         DEF EnqueueCurrentReplyItem,
             ReplyPipelineItemCoreBinding,
             ReplyPipelineItemMatchesAttempt
    <2>3. UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(5)
         DEF EnqueueCurrentReplyItem
    <2> QED BY <2>2, <2>3,
         ReplyRouteStutterPreservesItemCoreBinding
  <1> QED BY <1>1

THEOREM ReplyEnqueuePreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item:
      /\ ReplyPipelineItemRouteBinding(item)
      /\ EnqueueCurrentReplyItem(owner, semantic, source)
      => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item,
                ReplyPipelineItemRouteBinding(item),
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. /\ UNCHANGED ReplyRouteVars
           /\ rpPendingAttachments' = rpPendingAttachments
      BY <1>1, SMTT(5)
         DEF EnqueueCurrentReplyItem
    <2>2. ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))' <=>
           ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))
      BY <2>1, ReplyRouteStutterPreservesAttemptCurrentView
    <2>3. ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)' <=>
           ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)
      BY <2>1, SMTT(10)
         DEF ReplyRouteRebindPending,
             ReplyPendingAttachmentsFor
    <2> QED BY <1>1, <2>2, <2>3, SMTT(5)
         DEF ReplyPipelineItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyEnqueueNewItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item:
      /\ EnqueueCurrentReplyItem(owner, semantic, source)
      /\ item = ReplyEnqueuedPipelineItem(
                   owner, semantic, source)
      => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item,
                EnqueueCurrentReplyItem(owner, semantic, source),
                item = ReplyEnqueuedPipelineItem(
                         owner, semantic, source)
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. ReplyAttemptCurrent(
             ReplyAttemptFor(owner, semantic, source))
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2>2. UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(5)
         DEF EnqueueCurrentReplyItem
    <2>3. ReplyAttemptCurrent(
             ReplyAttemptFor(owner, semantic, source))'
      BY <2>1, <2>2,
         ReplyRouteStutterPreservesAttemptCurrentView
    <2>4. /\ item.owner = owner
           /\ item.semantic = semantic
           /\ item.source = source
      BY <1>1, ReplyEnqueuedPipelineItemProjection, SMTT(5)
    <2>5. ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))'
      BY <2>3, <2>4, SMTT(10)
    <2> QED BY <2>5,
         ReplyCurrentAttemptEstablishesRouteBindingPrime
  <1> QED BY <1>1

THEOREM ReplyEnqueueNewItemPhaseBinding ==
  \A owner, semantic, source:
    \A item:
      item = ReplyEnqueuedPipelineItem(owner, semantic, source)
      => ReplyPipelineItemPhaseBinding(item)'
PROOF
  <1>1. ASSUME NEW owner, NEW semantic, NEW source,
                NEW item,
                item = ReplyEnqueuedPipelineItem(
                         owner, semantic, source)
         PROVE ReplyPipelineItemPhaseBinding(item)'
    <2>1. /\ item.phase = "Queued"
           /\ item.ticketId = NoReplyPipelineTicket
           /\ item.ticketTenure = NoReplyTicketTenure
           /\ item.ticketPayload = {}
      BY <1>1, ReplyEnqueuedPipelineItemProjection, SMTT(5)
    <2>2. ReplyPipelineQueuedItem(item)'
      BY <2>1, SMTT(5)
         DEF ReplyPipelineQueuedItem
    <2> QED BY <2>2,
         ReplyQueuedItemEstablishesPhaseBindingPrime
  <1> QED BY <1>1

THEOREM ReplyEnqueuePreservesExistingItemFifoHead ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item \in rpItems:
      /\ ReplyPipelineFifoOrdinalInvariant
      /\ ReplyPipelineItemIsFifoHead(item)
      /\ EnqueueCurrentReplyItem(owner, semantic, source)
      => ReplyPipelineItemIsFifoHead(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item \in rpItems,
                ReplyPipelineFifoOrdinalInvariant,
                ReplyPipelineItemIsFifoHead(item),
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE ReplyPipelineItemIsFifoHead(item)'
    <2>1. rpItems' =
             rpItems \cup
               {ReplyEnqueuedPipelineItem(
                  owner, semantic, source)}
      BY <1>1 DEF EnqueueCurrentReplyItem,
                   ReplyEnqueuedPipelineItem
    <2>2. /\ ReplyEnqueuedPipelineItem(
                  owner, semantic, source).owner = owner
           /\ ReplyEnqueuedPipelineItem(
                  owner, semantic, source).fifoOrdinal =
                rpNextFifoOrdinal[owner]
      BY ReplyEnqueuedPipelineItemProjection
    <2>3. item.fifoOrdinal <
             rpNextFifoOrdinal[item.owner]
      BY <1>1 DEF ReplyPipelineFifoOrdinalInvariant
    <2>4. ASSUME NEW other \in
                    ReplyPipelineItemsInLane(
                      item.owner,
                      item.source,
                      item.outputClass)'
           PROVE item.fifoOrdinal <= other.fifoOrdinal
      <3>1. /\ other \in rpItems'
             /\ other.owner = item.owner
             /\ other.source = item.source
             /\ other.outputClass = item.outputClass
        BY <2>4 DEF ReplyPipelineItemsInLane
      <3>2. other \in rpItems
                \/ other = ReplyEnqueuedPipelineItem(
                             owner, semantic, source)
        BY <2>1, <3>1, SMTT(10)
      <3>3. CASE other \in rpItems
        <4>1. other \in
                 ReplyPipelineItemsInLane(
                   item.owner,
                   item.source,
                   item.outputClass)
          BY <3>1, <3>3
             DEF ReplyPipelineItemsInLane
        <4> QED BY <1>1, <4>1
             DEF ReplyPipelineItemIsFifoHead
      <3>4. CASE other =
                    ReplyEnqueuedPipelineItem(
                      owner, semantic, source)
        BY <2>2, <2>3, <3>1, <3>4, SMTT(10)
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>4
         DEF ReplyPipelineItemIsFifoHead
  <1> QED BY <1>1

THEOREM ReplyEnqueuePreservesExistingItemPhaseBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item \in rpItems:
      /\ ReplyPipelineFifoOrdinalInvariant
      /\ ReplyPipelineItemPhaseBinding(item)
      /\ EnqueueCurrentReplyItem(owner, semantic, source)
      => ReplyPipelineItemPhaseBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item \in rpItems,
                ReplyPipelineFifoOrdinalInvariant,
                ReplyPipelineItemPhaseBinding(item),
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE ReplyPipelineItemPhaseBinding(item)'
    <2>1. rpNextTicketId' = rpNextTicketId
      BY <1>1, SMTT(5)
         DEF EnqueueCurrentReplyItem
    <2>2. CASE item.phase = "Queued"
      <3>1. ReplyPipelineQueuedItem(item)'
        BY <1>1, <2>2, SMTT(10)
           DEF ReplyPipelineItemPhaseBinding,
               ReplyPipelineQueuedItem
      <3> QED BY <3>1,
           ReplyQueuedItemEstablishesPhaseBindingPrime
    <2>3. CASE item.phase # "Queued"
      <3>1. /\ ReplyPipelineTicketValid(item)
             /\ ReplyPipelineItemIsFifoHead(item)
             /\ (item.phase \in {"Admitted", "Flushed"}
                   => item.flushRequired)
        BY <1>1, <2>3
           DEF ReplyPipelineItemPhaseBinding
      <3>2. ReplyPipelineTicketValid(item)'
        BY <2>1, <3>1, SMTT(20)
           DEF ReplyPipelineTicketValid
      <3>3. ReplyPipelineItemIsFifoHead(item)'
        BY <1>1, <3>1,
           ReplyEnqueuePreservesExistingItemFifoHead
      <3> QED BY <2>3, <3>1, <3>2, <3>3, SMTT(10)
           DEF ReplyPipelineItemPhaseBinding
    <2> QED BY <2>2, <2>3, SMTT(5)
  <1> QED BY <1>1

THEOREM ReplyEnqueuePreservesReconnectNoTicketedInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineReconnectNoTicketedInvariant
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    => ReplyPipelineReconnectNoTicketedInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineReconnectNoTicketedInvariant,
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE ReplyPipelineReconnectNoTicketedInvariant'
    <2>1. /\ rpPendingAttachments' = rpPendingAttachments
           /\ rpItems' =
                rpItems \cup
                  {ReplyEnqueuedPipelineItem(
                     owner, semantic, source)}
      BY <1>1, SMTT(5)
         DEF EnqueueCurrentReplyItem,
             ReplyEnqueuedPipelineItem
    <2>2. ReplyEnqueuedPipelineItem(
             owner, semantic, source).phase = "Queued"
      BY ReplyEnqueuedPipelineItemProjection
    <2>3. ASSUME NEW checkedOwner \in ReplyOwners,
                  NEW checkedSource \in ReplySources,
                  ReplyReconnectPendingForSource(
                    checkedOwner, checkedSource)'
           PROVE \A checkedItem \in rpItems':
                   /\ checkedItem.owner = checkedOwner
                   /\ checkedItem.source = checkedSource
                   => checkedItem.phase # "Ticketed"
      <3>1. ReplyReconnectPendingForSource(
               checkedOwner, checkedSource)
        BY <2>1, <2>3, SMTT(10)
           DEF ReplyReconnectPendingForSource,
               ReplyReconnectPending,
               ReplyPendingAttachmentsFor
      <3>2. ASSUME NEW checkedItem \in rpItems',
                    checkedItem.owner = checkedOwner,
                    checkedItem.source = checkedSource
             PROVE checkedItem.phase # "Ticketed"
        <4>1. checkedItem \in rpItems
                  \/ checkedItem = ReplyEnqueuedPipelineItem(
                                       owner, semantic, source)
          BY <2>1, <3>2, SMTT(10)
        <4>2. CASE checkedItem \in rpItems
          BY <1>1, <3>1, <3>2, <4>2
             DEF ReplyPipelineReconnectNoTicketedInvariant
        <4>3. CASE checkedItem =
                      ReplyEnqueuedPipelineItem(
                        owner, semantic, source)
          BY <2>2, <4>3, SMTT(5)
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2
    <2> QED BY <2>3
         DEF ReplyPipelineReconnectNoTicketedInvariant
  <1> QED BY <1>1

THEOREM ReplyEnqueuePreservesReconnectWriterActiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineReconnectWriterActiveInvariant
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    => ReplyPipelineReconnectWriterActiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineReconnectWriterActiveInvariant,
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE ReplyPipelineReconnectWriterActiveInvariant'
    <2>1. /\ rpPendingAttachments' = rpPendingAttachments
           /\ rpItems' =
                rpItems \cup
                  {ReplyEnqueuedPipelineItem(
                     owner, semantic, source)}
           /\ rrSourceActive' = rrSourceActive
      BY <1>1, SMTT(5)
         DEF EnqueueCurrentReplyItem,
             ReplyEnqueuedPipelineItem,
             ReplyRouteVars
    <2>2. ReplyEnqueuedPipelineItem(
             owner, semantic, source).phase = "Queued"
      BY ReplyEnqueuedPipelineItemProjection
    <2>3. \A checkedOwner \in ReplyOwners,
              checkedSource \in ReplySources:
             ReplyReconnectPendingForSource(
               checkedOwner, checkedSource)' <=>
             ReplyReconnectPendingForSource(
               checkedOwner, checkedSource)
      BY <2>1, SMTT(20)
         DEF ReplyReconnectPendingForSource,
             ReplyReconnectPending,
             ReplyPendingAttachmentsFor
    <2>4. \A checkedOwner \in ReplyOwners,
              checkedSource \in ReplySources:
             ReplyPipelineHasUnresolvedWriter(
               checkedOwner, checkedSource)' <=>
             ReplyPipelineHasUnresolvedWriter(
               checkedOwner, checkedSource)
      BY <2>1, <2>2, SMTT(30)
         DEF ReplyPipelineHasUnresolvedWriter
    <2>5. ASSUME NEW checkedOwner \in ReplyOwners,
                  NEW checkedSource \in ReplySources,
                  ReplyReconnectPendingForSource(
                    checkedOwner, checkedSource)',
                  ReplyPipelineHasUnresolvedWriter(
                    checkedOwner, checkedSource)'
           PROVE rrSourceActive'[checkedOwner][checkedSource]
      BY <1>1, <2>1, <2>3, <2>4, <2>5, SMTT(10)
         DEF ReplyPipelineReconnectWriterActiveInvariant
    <2> QED BY <2>5
         DEF ReplyPipelineReconnectWriterActiveInvariant
  <1> QED BY <1>1

THEOREM ReplyEnqueueFifoCounterSelectedUpdate ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ rpNextFifoOrdinal
         \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    =>
      rpNextFifoOrdinal'[owner] = rpNextFifoOrdinal[owner] + 1
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                rpNextFifoOrdinal
                  \in [ReplyOwners ->
                        1..(ReplyPipelineOrdinalLimit + 1)],
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE rpNextFifoOrdinal'[owner] =
                 rpNextFifoOrdinal[owner] + 1
    <2>1. rpNextFifoOrdinal' =
             [rpNextFifoOrdinal EXCEPT
                ![owner] = rpNextFifoOrdinal[owner] + 1]
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2> QED BY <1>1, <2>1, SMTT(10)
  <1> QED BY <1>1

THEOREM ReplyEnqueueFifoCounterOtherUpdate ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, checkedOwner \in ReplyOwners:
    /\ rpNextFifoOrdinal
         \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    /\ checkedOwner # owner
    => rpNextFifoOrdinal'[checkedOwner] =
         rpNextFifoOrdinal[checkedOwner]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW checkedOwner \in ReplyOwners,
                rpNextFifoOrdinal
                  \in [ReplyOwners ->
                        1..(ReplyPipelineOrdinalLimit + 1)],
                EnqueueCurrentReplyItem(owner, semantic, source),
                checkedOwner # owner
         PROVE rpNextFifoOrdinal'[checkedOwner] =
                 rpNextFifoOrdinal[checkedOwner]
    <2>1. rpNextFifoOrdinal' =
             [rpNextFifoOrdinal EXCEPT
                ![owner] = rpNextFifoOrdinal[owner] + 1]
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2> QED BY <1>1, <2>1, SMTT(10)
  <1> QED BY <1>1

THEOREM ReplyEnqueueFifoCounterPreservesDomain ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ rpNextFifoOrdinal
         \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    =>
      DOMAIN rpNextFifoOrdinal' = DOMAIN rpNextFifoOrdinal
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                rpNextFifoOrdinal
                  \in [ReplyOwners ->
                        1..(ReplyPipelineOrdinalLimit + 1)],
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE DOMAIN rpNextFifoOrdinal' =
                 DOMAIN rpNextFifoOrdinal
    <2>1. rpNextFifoOrdinal' =
             [rpNextFifoOrdinal EXCEPT
                ![owner] = rpNextFifoOrdinal[owner] + 1]
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2> QED BY <1>1, <2>1, SMTT(10)
  <1> QED BY <1>1

THEOREM ReplyNatIntervalSuccessor ==
  \A upper \in Nat:
    \A value \in 1..upper:
      value + 1 \in 1..(upper + 1)
BY SMT

THEOREM ReplyNatLessThanSuccessor ==
  \A lower, upper \in Nat:
    lower < upper => lower < upper + 1
BY SMT

THEOREM ReplyEnqueueFifoCounterPreservesType ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineConfiguration
    /\ rpNextFifoOrdinal
         \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    => rpNextFifoOrdinal'
         \in [ReplyOwners -> 1..(ReplyPipelineOrdinalLimit + 1)]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineConfiguration,
                rpNextFifoOrdinal
                  \in [ReplyOwners ->
                        1..(ReplyPipelineOrdinalLimit + 1)],
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE rpNextFifoOrdinal'
                 \in [ReplyOwners ->
                       1..(ReplyPipelineOrdinalLimit + 1)]
    <2>1. rpNextFifoOrdinal[owner]
             \in 1..ReplyPipelineOrdinalLimit
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2>2. ReplyPipelineOrdinalLimit \in Nat
      BY <1>1 DEF ReplyPipelineConfiguration
    <2>3. rpNextFifoOrdinal[owner] + 1
             \in 1..(ReplyPipelineOrdinalLimit + 1)
      BY <2>1, <2>2, ReplyNatIntervalSuccessor
    <2>4. rpNextFifoOrdinal' =
             [rpNextFifoOrdinal EXCEPT
                ![owner] = rpNextFifoOrdinal[owner] + 1]
      BY <1>1 DEF EnqueueCurrentReplyItem
    <2> QED BY <1>1, <2>3, <2>4,
         ReplyFunctionalUpdatePreservesType
  <1> QED BY <1>1

THEOREM ReplyEnqueueCurrentItemPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ EnqueueCurrentReplyItem(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                EnqueueCurrentReplyItem(owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStutterPreservesInductiveInvariant
         DEF ReplyPipelineInductiveInvariant,
             EnqueueCurrentReplyItem, ReplyRouteVars
    <2>2. ReplyPipelineConfiguration'
      BY <1>1 DEF ReplyPipelineInductiveInvariant
    <2>3. ReplyPipelineTypeInvariant'
      <3>1. LET attempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 item ==
                   ReplyPipelineItem(
                     owner, semantic, source,
                     attempt.messageCursor, attempt.chunkCursor,
                     rpNextFifoOrdinal[owner],
                     attempt.connectionTenure,
                     "Queued", NoReplyPipelineTicket,
                     NoReplyTicketTenure, {})
             IN ReplyPipelineItemHasType(item)
        BY <1>1, ReplyEnqueueItemHasType
           DEF ReplyPipelineInductiveInvariant,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant
      <3>2. rpPendingAttachments' \subseteq ReplyAttachmentSet
        BY <1>1, SMTT(5)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineTypeInvariant,
               EnqueueCurrentReplyItem, ReplyRouteVars
      <3>3. \A item \in rpItems': ReplyPipelineItemHasType(item)
        BY <1>1, <3>1, FS_AddElement, SMTT(20)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineTypeInvariant,
               EnqueueCurrentReplyItem
      <3>4. rpNextFifoOrdinal'
                 \in [ReplyOwners ->
                       1..(ReplyPipelineOrdinalLimit + 1)]
        BY <1>1, ReplyEnqueueFifoCounterPreservesType
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineTypeInvariant
      <3>5. rpNextTicketId'
                 \in [ReplyOwners -> Nat \ {0}]
        BY <1>1, SMTT(5)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineTypeInvariant,
               EnqueueCurrentReplyItem, ReplyRouteVars
      <3> QED BY <3>2, <3>3, <3>4, <3>5
           DEF ReplyPipelineTypeInvariant
    <2>4. ReplyPipelinePerIdentityInvariant'
      <3>1. ReplyPipelinePendingPerIdentityInvariant'
        <4>1. ReplyPipelinePendingPerIdentityInvariant
          BY <1>1
             DEF ReplyPipelineInductiveInvariant,
                 ReplyPipelineOwnershipInvariant,
                 ReplyPipelinePerIdentityInvariant
        <4>2. rpPendingAttachments' = rpPendingAttachments
          BY <1>1 DEF EnqueueCurrentReplyItem, ReplyRouteVars
        <4> QED BY <4>1, <4>2
             DEF ReplyPipelinePendingPerIdentityInvariant
      <3>2. LET attempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 item ==
                   ReplyPipelineItem(
                     owner, semantic, source,
                     attempt.messageCursor, attempt.chunkCursor,
                     rpNextFifoOrdinal[owner],
                     attempt.connectionTenure,
                     "Queued", NoReplyPipelineTicket,
                     NoReplyTicketTenure, {})
             IN /\ item.owner = owner
                /\ item.semantic = semantic
                /\ item.source = source
                /\ ~ReplyPipelineItemOwned(owner, semantic, source)
        BY <1>1, ReplyEnqueueItemProjection
           DEF EnqueueCurrentReplyItem
      <3>3. ReplyPipelineItemPerIdentityInvariant'
        BY <1>1, <3>2, FS_AddElement, SMTT(40)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelinePerIdentityInvariant,
               ReplyPipelineItemPerIdentityInvariant,
               ReplyPipelineItemOwned,
               ReplyPipelineItemsFor,
               EnqueueCurrentReplyItem
      <3> QED BY <3>1, <3>3
           DEF ReplyPipelinePerIdentityInvariant
    <2>5. ReplyPipelineFifoOrdinalInvariant'
      <3>1. LET attempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 item ==
                   ReplyPipelineItem(
                     owner, semantic, source,
                     attempt.messageCursor, attempt.chunkCursor,
                     rpNextFifoOrdinal[owner],
                     attempt.connectionTenure,
                     "Queued", NoReplyPipelineTicket,
                     NoReplyTicketTenure, {})
             IN /\ item.owner = owner
                /\ item.fifoOrdinal = rpNextFifoOrdinal[owner]
        BY ReplyEnqueueItemProjection
      <3>2. \A left, right \in rpItems':
               /\ left.owner = right.owner
               /\ left.fifoOrdinal = right.fifoOrdinal
               => left = right
        BY <1>1, <3>1, FS_AddElement, SMTT(40)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineFifoOrdinalInvariant,
               EnqueueCurrentReplyItem
      <3>3. \A item \in rpItems':
               item.fifoOrdinal <
                 rpNextFifoOrdinal'[item.owner]
        <4>1. ASSUME NEW checkedItem \in rpItems'
               PROVE checkedItem.fifoOrdinal <
                       rpNextFifoOrdinal'[checkedItem.owner]
          <5>1. LET attempt ==
                       ReplyAttemptFor(owner, semantic, source)
                     enqueued ==
                       ReplyPipelineItem(
                         owner, semantic, source,
                         attempt.messageCursor,
                         attempt.chunkCursor,
                         rpNextFifoOrdinal[owner],
                         attempt.connectionTenure,
                         "Queued", NoReplyPipelineTicket,
                         NoReplyTicketTenure, {})
                 IN checkedItem \in rpItems \/ checkedItem = enqueued
            BY <1>1, <4>1, SMTT(10)
               DEF EnqueueCurrentReplyItem
          <5>2. CASE LET attempt ==
                           ReplyAttemptFor(owner, semantic, source)
                         enqueued ==
                           ReplyPipelineItem(
                             owner, semantic, source,
                             attempt.messageCursor,
                             attempt.chunkCursor,
                             rpNextFifoOrdinal[owner],
                             attempt.connectionTenure,
                             "Queued", NoReplyPipelineTicket,
                             NoReplyTicketTenure, {})
                     IN checkedItem = enqueued
            <6>1. /\ checkedItem.owner = owner
                   /\ checkedItem.fifoOrdinal =
                        rpNextFifoOrdinal[owner]
              BY <3>1, <5>2, SMTT(5)
            <6>2. rpNextFifoOrdinal'[owner] =
                     rpNextFifoOrdinal[owner] + 1
              BY <1>1, ReplyEnqueueFifoCounterSelectedUpdate
                 DEF ReplyPipelineInductiveInvariant,
                     ReplyPipelineTypeInvariant
            <6>3. rpNextFifoOrdinal[owner]
                     \in 1..ReplyPipelineOrdinalLimit
              BY <1>1 DEF EnqueueCurrentReplyItem
            <6> QED BY <6>1, <6>2, <6>3, SMTT(5)
          <5>3. CASE checkedItem \in rpItems
            <6>1. checkedItem.fifoOrdinal <
                     rpNextFifoOrdinal[checkedItem.owner]
              BY <1>1, <5>3
                 DEF ReplyPipelineInductiveInvariant,
                     ReplyPipelineOwnershipInvariant,
                     ReplyPipelineFifoOrdinalInvariant
            <6>2. CASE checkedItem.owner = owner
              <7>1. rpNextFifoOrdinal'[owner] =
                       rpNextFifoOrdinal[owner] + 1
                BY <1>1, ReplyEnqueueFifoCounterSelectedUpdate
                   DEF ReplyPipelineInductiveInvariant,
                       ReplyPipelineTypeInvariant
              <7>2. rpNextFifoOrdinal[owner]
                       \in 1..ReplyPipelineOrdinalLimit
                BY <1>1 DEF EnqueueCurrentReplyItem
              <7>3. checkedItem.fifoOrdinal \in Nat
                BY <1>1, <5>3, SMTT(10)
                   DEF ReplyPipelineInductiveInvariant,
                       ReplyPipelineTypeInvariant,
                       ReplyPipelineItemHasType
              <7>4. rpNextFifoOrdinal[owner] \in Nat
                BY <7>2, SMT
              <7>5. checkedItem.fifoOrdinal <
                       rpNextFifoOrdinal[owner] + 1
                BY <6>1, <6>2, <7>3, <7>4,
                   ReplyNatLessThanSuccessor
              <7> QED BY <6>2, <7>1, <7>5
            <6>3. CASE checkedItem.owner # owner
              <7>1. checkedItem.owner \in ReplyOwners
                BY <1>1, <5>3
                   DEF ReplyPipelineInductiveInvariant,
                       ReplyPipelineTypeInvariant,
                       ReplyPipelineItemHasType
              <7>2. rpNextFifoOrdinal'[checkedItem.owner] =
                       rpNextFifoOrdinal[checkedItem.owner]
                BY <1>1, <6>3, <7>1,
                   ReplyEnqueueFifoCounterOtherUpdate
                   DEF ReplyPipelineInductiveInvariant,
                       ReplyPipelineTypeInvariant
              <7> QED BY <6>1, <7>2
            <6> QED BY <6>2, <6>3
          <5> QED BY <5>1, <5>2, <5>3
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3
           DEF ReplyPipelineFifoOrdinalInvariant
    <2>6. ReplyPipelineTicketIdentityInvariant'
      <3>1. LET attempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 item ==
                   ReplyPipelineItem(
                     owner, semantic, source,
                     attempt.messageCursor, attempt.chunkCursor,
                     rpNextFifoOrdinal[owner],
                     attempt.connectionTenure,
                     "Queued", NoReplyPipelineTicket,
                     NoReplyTicketTenure, {})
             IN item.ticketId = NoReplyPipelineTicket
        BY ReplyEnqueueItemProjection
      <3> QED BY <1>1, <3>1, FS_AddElement, SMTT(40)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineTicketIdentityInvariant,
               EnqueueCurrentReplyItem
    <2>7. ReplyPipelineItemBindingInvariant'
      <3>1. rpItems' =
               rpItems \cup
                 {ReplyEnqueuedPipelineItem(
                    owner, semantic, source)}
        BY <1>1 DEF EnqueueCurrentReplyItem,
                     ReplyEnqueuedPipelineItem
      <3>2. ASSUME NEW checkedItem \in rpItems'
             PROVE /\ ReplyPipelineItemCoreBinding(checkedItem)'
                   /\ ReplyPipelineItemRouteBinding(checkedItem)'
                   /\ ReplyPipelineItemPhaseBinding(checkedItem)'
        <4>1. checkedItem \in rpItems
                  \/ checkedItem = ReplyEnqueuedPipelineItem(
                                       owner, semantic, source)
          BY <3>1, <3>2, SMTT(10)
        <4>2. CASE checkedItem \in rpItems
          <5>1. ReplyPipelineItemCoreBinding(checkedItem)'
            BY <1>1, <4>2,
               ReplyEnqueuePreservesExistingItemCoreBinding
               DEF ReplyPipelineInductiveInvariant,
                   ReplyPipelineOwnershipInvariant,
                   ReplyPipelineItemBindingInvariant
          <5>2. ReplyPipelineItemRouteBinding(checkedItem)'
            BY <1>1, <4>2,
               ReplyEnqueuePreservesExistingItemRouteBinding
               DEF ReplyPipelineInductiveInvariant,
                   ReplyPipelineOwnershipInvariant,
                   ReplyPipelineItemBindingInvariant
          <5>3. ReplyPipelineItemPhaseBinding(checkedItem)'
            BY <1>1, <4>2,
               ReplyEnqueuePreservesExistingItemPhaseBinding
               DEF ReplyPipelineInductiveInvariant,
                   ReplyPipelineOwnershipInvariant,
                   ReplyPipelineFifoOrdinalInvariant,
                   ReplyPipelineItemBindingInvariant
          <5> QED BY <5>1, <5>2, <5>3
        <4>3. CASE checkedItem =
                      ReplyEnqueuedPipelineItem(
                        owner, semantic, source)
          <5>1. ReplyRouteTypeInvariant
            BY <1>1
               DEF ReplyPipelineInductiveInvariant,
                   ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant
          <5>2. ReplyPipelineItemCoreBinding(checkedItem)'
            BY <1>1, <4>3, <5>1,
               ReplyEnqueueNewItemCoreBinding
          <5>3. ReplyPipelineItemRouteBinding(checkedItem)'
            BY <1>1, <4>3,
               ReplyEnqueueNewItemRouteBinding
          <5>4. ReplyPipelineItemPhaseBinding(checkedItem)'
            BY <4>3,
               ReplyEnqueueNewItemPhaseBinding
          <5> QED BY <5>2, <5>3, <5>4
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2
           DEF ReplyPipelineItemBindingInvariant
    <2>8. ReplyPipelineReconnectBarrierInvariant'
      <3>1. ReplyPipelineReconnectNoTicketedInvariant'
        BY <1>1,
           ReplyEnqueuePreservesReconnectNoTicketedInvariant
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineReconnectBarrierInvariant
      <3>2. ReplyPipelineReconnectWriterActiveInvariant'
        BY <1>1,
           ReplyEnqueuePreservesReconnectWriterActiveInvariant
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineReconnectBarrierInvariant
      <3> QED BY <3>1, <3>2
           DEF ReplyPipelineReconnectBarrierInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                  <2>5, <2>6, <2>7, <2>8
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
  <1> QED BY <1>1

THEOREM ReplyOwnedPipelineItemProjection ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyPipelineItemOwned(owner, semantic, source)
    => LET item == ReplyPipelineItemFor(owner, semantic, source)
       IN /\ item \in rpItems
          /\ item.owner = owner
          /\ item.semantic = semantic
          /\ item.source = source
BY SMTT(30)
   DEF ReplyPipelineItemOwned,
       ReplyPipelineItemFor,
       ReplyPipelineItemsFor

THEOREM ReplyPipelineItemWithTicketProjection ==
  \A item, ticketId:
    LET ticketed == ReplyPipelineItemWithTicket(item, ticketId)
    IN /\ ticketed.owner = item.owner
       /\ ticketed.semantic = item.semantic
       /\ ticketed.source = item.source
       /\ ticketed.messageCursor = item.messageCursor
       /\ ticketed.chunkCursor = item.chunkCursor
       /\ ticketed.outputClass = item.outputClass
       /\ ticketed.flushRequired = item.flushRequired
       /\ ticketed.fifoOrdinal = item.fifoOrdinal
       /\ ticketed.routeTenure = item.routeTenure
       /\ ticketed.phase = "Ticketed"
       /\ ticketed.ticketId = ticketId
       /\ ticketed.ticketTenure = item.routeTenure
       /\ ticketed.ticketPayload =
            {ReplyPipelinePayloadForItem(item)}
BY SMTT(10)
   DEF ReplyPipelineItemWithTicket,
       ReplyPipelineRawItem

THEOREM ReplyPipelineReplacementMembership ==
  \A items, oldItem, newItem, checkedItem:
    checkedItem \in ((items \ {oldItem}) \cup {newItem})
    => \/ checkedItem \in items
       \/ checkedItem = newItem
BY SMT

THEOREM ReplyPipelineReplacementPreservesPerIdentity ==
  \A items, oldItem, newItem:
    /\ oldItem \in items
    /\ newItem.owner = oldItem.owner
    /\ newItem.semantic = oldItem.semantic
    /\ newItem.source = oldItem.source
    /\ \A left, right \in items:
         /\ left.owner = right.owner
         /\ left.semantic = right.semantic
         /\ left.source = right.source
         => left = right
    => \A left, right \in
            ((items \ {oldItem}) \cup {newItem}):
         /\ left.owner = right.owner
         /\ left.semantic = right.semantic
         /\ left.source = right.source
         => left = right
BY SMTT(30)

THEOREM ReplyPipelineReplacementPreservesFifoIdentity ==
  \A items, oldItem, newItem:
    /\ oldItem \in items
    /\ newItem.owner = oldItem.owner
    /\ newItem.fifoOrdinal = oldItem.fifoOrdinal
    /\ \A left, right \in items:
         /\ left.owner = right.owner
         /\ left.fifoOrdinal = right.fifoOrdinal
         => left = right
    => \A left, right \in
            ((items \ {oldItem}) \cup {newItem}):
         /\ left.owner = right.owner
         /\ left.fifoOrdinal = right.fifoOrdinal
         => left = right
BY SMTT(30)

THEOREM ReplyPipelineReplacementPreservesFifoBound ==
  \A items, oldItem, newItem, counters:
    /\ oldItem \in items
    /\ newItem.owner = oldItem.owner
    /\ newItem.fifoOrdinal = oldItem.fifoOrdinal
    /\ \A item \in items:
         item.fifoOrdinal < counters[item.owner]
    => \A item \in ((items \ {oldItem}) \cup {newItem}):
         item.fifoOrdinal < counters[item.owner]
BY SMTT(30)

THEOREM ReplyPipelineReplacementPreservesTicketIdentity ==
  \A items, oldItem, newItem:
    /\ oldItem \in items
    /\ newItem.ticketId # NoReplyPipelineTicket
    /\ \A item \in items:
         item.owner = newItem.owner =>
           item.ticketId # newItem.ticketId
    /\ \A left, right \in items:
         /\ left.owner = right.owner
         /\ left.ticketId # NoReplyPipelineTicket
         /\ left.ticketId = right.ticketId
         => left = right
    => \A left, right \in
            ((items \ {oldItem}) \cup {newItem}):
         /\ left.owner = right.owner
         /\ left.ticketId # NoReplyPipelineTicket
         /\ left.ticketId = right.ticketId
         => left = right
BY SMTT(30)

THEOREM ReplyPipelineItemWithTicketPreservesCoreBinding ==
  \A item, ticketId:
    ReplyPipelineItemCoreBinding(item) =>
      ReplyPipelineItemCoreBinding(
        ReplyPipelineItemWithTicket(item, ticketId))
BY SMTT(15)
   DEF ReplyPipelineItemCoreBinding,
       ReplyPipelineItemMatchesAttempt,
       ReplyPipelineItemWithTicket,
       ReplyPipelineRawItem

THEOREM ReplyPipelineItemWithTicketPreservesRouteBinding ==
  \A item, ticketId:
    ReplyPipelineItemRouteBinding(
      ReplyPipelineItemWithTicket(item, ticketId)) <=>
      ReplyPipelineItemRouteBinding(item)
BY SMTT(15)
   DEF ReplyPipelineItemRouteBinding,
       ReplyPipelineItemWithTicket,
       ReplyPipelineRawItem

THEOREM ReplyRouteAndPendingStutterPreservesItemRouteBinding ==
  \A item:
    /\ ReplyPipelineItemRouteBinding(item)
    /\ UNCHANGED ReplyRouteVars
    /\ rpPendingAttachments' = rpPendingAttachments
    => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW item,
                ReplyPipelineItemRouteBinding(item),
                UNCHANGED ReplyRouteVars,
                rpPendingAttachments' = rpPendingAttachments
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))' <=>
           ReplyAttemptCurrent(
             ReplyAttemptFor(
               item.owner, item.semantic, item.source))
      BY <1>1,
         ReplyRouteStutterPreservesAttemptCurrentView
    <2>2. ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)' <=>
           ReplyRouteRebindPending(
             item.owner, item.semantic, item.source)
      BY <1>1, SMTT(15)
         DEF ReplyRouteRebindPending,
             ReplyPendingAttachmentsFor
    <2> QED BY <1>1, <2>1, <2>2, SMTT(10)
         DEF ReplyPipelineItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyPipelineReplacementPreservesFifoHead ==
  \A oldItem, newItem, checkedItem:
    /\ oldItem \in rpItems
    /\ newItem.owner = oldItem.owner
    /\ newItem.source = oldItem.source
    /\ newItem.outputClass = oldItem.outputClass
    /\ newItem.fifoOrdinal = oldItem.fifoOrdinal
    /\ newItem.fifoOrdinal \in Nat
    /\ ReplyPipelineItemIsFifoHead(checkedItem)
    /\ rpItems' = ReplyPipelineReplaceItem(oldItem, newItem)
    => ReplyPipelineItemIsFifoHead(checkedItem)'
PROOF
  <1>1. ASSUME NEW oldItem, NEW newItem, NEW checkedItem,
                oldItem \in rpItems,
                newItem.owner = oldItem.owner,
                newItem.source = oldItem.source,
                newItem.outputClass = oldItem.outputClass,
                newItem.fifoOrdinal = oldItem.fifoOrdinal,
                newItem.fifoOrdinal \in Nat,
                ReplyPipelineItemIsFifoHead(checkedItem),
                rpItems' =
                  ReplyPipelineReplaceItem(oldItem, newItem)
         PROVE ReplyPipelineItemIsFifoHead(checkedItem)'
    <2>1. ASSUME NEW other \in
                    ReplyPipelineItemsInLane(
                      checkedItem.owner,
                      checkedItem.source,
                      checkedItem.outputClass)'
           PROVE checkedItem.fifoOrdinal <= other.fifoOrdinal
      <3>1. /\ other \in rpItems'
             /\ other.owner = checkedItem.owner
             /\ other.source = checkedItem.source
             /\ other.outputClass = checkedItem.outputClass
        BY <2>1 DEF ReplyPipelineItemsInLane
      <3>2. \/ other \in rpItems
             \/ other = newItem
        BY <1>1, <3>1,
           ReplyPipelineReplacementMembership
           DEF ReplyPipelineReplaceItem
      <3>3. CASE other \in rpItems
        <4>1. other \in
                 ReplyPipelineItemsInLane(
                   checkedItem.owner,
                   checkedItem.source,
                   checkedItem.outputClass)
          BY <3>1, <3>3
             DEF ReplyPipelineItemsInLane
        <4> QED BY <1>1, <4>1
             DEF ReplyPipelineItemIsFifoHead
      <3>4. CASE other = newItem
        <4>1. oldItem \in
                 ReplyPipelineItemsInLane(
                   checkedItem.owner,
                   checkedItem.source,
                   checkedItem.outputClass)
          BY <1>1, <3>1, <3>4, SMTT(10)
             DEF ReplyPipelineItemsInLane
        <4> QED BY <1>1, <3>4, <4>1, SMTT(10)
             DEF ReplyPipelineItemIsFifoHead
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1
         DEF ReplyPipelineItemIsFifoHead
  <1> QED BY <1>1

THEOREM ReplyPipelineReplacementEstablishesNewFifoHead ==
  \A oldItem, newItem:
    /\ oldItem \in rpItems
    /\ newItem.owner = oldItem.owner
    /\ newItem.source = oldItem.source
    /\ newItem.outputClass = oldItem.outputClass
    /\ newItem.fifoOrdinal = oldItem.fifoOrdinal
    /\ newItem.fifoOrdinal \in Nat
    /\ ReplyPipelineItemIsFifoHead(oldItem)
    /\ rpItems' = ReplyPipelineReplaceItem(oldItem, newItem)
    => ReplyPipelineItemIsFifoHead(newItem)'
PROOF
  <1>1. ASSUME NEW oldItem, NEW newItem,
                oldItem \in rpItems,
                newItem.owner = oldItem.owner,
                newItem.source = oldItem.source,
                newItem.outputClass = oldItem.outputClass,
                newItem.fifoOrdinal = oldItem.fifoOrdinal,
                newItem.fifoOrdinal \in Nat,
                ReplyPipelineItemIsFifoHead(oldItem),
                rpItems' =
                  ReplyPipelineReplaceItem(oldItem, newItem)
         PROVE ReplyPipelineItemIsFifoHead(newItem)'
    <2>1. ASSUME NEW other \in
                    ReplyPipelineItemsInLane(
                      newItem.owner,
                      newItem.source,
                      newItem.outputClass)'
           PROVE newItem.fifoOrdinal <= other.fifoOrdinal
      <3>1. /\ other \in rpItems'
             /\ other.owner = newItem.owner
             /\ other.source = newItem.source
             /\ other.outputClass = newItem.outputClass
        BY <2>1 DEF ReplyPipelineItemsInLane
      <3>2. \/ other \in rpItems
             \/ other = newItem
        BY <1>1, <3>1,
           ReplyPipelineReplacementMembership
           DEF ReplyPipelineReplaceItem
      <3>3. CASE other \in rpItems
        <4>1. other \in
                 ReplyPipelineItemsInLane(
                   oldItem.owner,
                   oldItem.source,
                   oldItem.outputClass)
          BY <1>1, <3>1, <3>3, SMTT(10)
             DEF ReplyPipelineItemsInLane
        <4> QED BY <1>1, <4>1, SMTT(10)
             DEF ReplyPipelineItemIsFifoHead
      <3>4. CASE other = newItem
        BY <1>1, <3>4, SMT
      <3> QED BY <3>2, <3>3, <3>4
    <2> QED BY <2>1
         DEF ReplyPipelineItemIsFifoHead
  <1> QED BY <1>1

THEOREM ReplyPipelineReplacementPreservesWriterView ==
  \A oldItem, newItem:
    /\ oldItem \in rpItems
    /\ oldItem.phase = "Queued"
    /\ newItem.phase = "Ticketed"
    /\ newItem.owner = oldItem.owner
    /\ newItem.source = oldItem.source
    /\ rpItems' = ReplyPipelineReplaceItem(oldItem, newItem)
    => \A owner, source:
         ReplyPipelineHasUnresolvedWriter(owner, source)' <=>
           ReplyPipelineHasUnresolvedWriter(owner, source)
PROOF
  <1>1. ASSUME NEW oldItem, NEW newItem,
                oldItem \in rpItems,
                oldItem.phase = "Queued",
                newItem.phase = "Ticketed",
                newItem.owner = oldItem.owner,
                newItem.source = oldItem.source,
                rpItems' =
                  ReplyPipelineReplaceItem(oldItem, newItem)
         PROVE \A owner, source:
                 ReplyPipelineHasUnresolvedWriter(owner, source)' <=>
                   ReplyPipelineHasUnresolvedWriter(owner, source)
    <2>1. ASSUME NEW owner, NEW source,
                  ReplyPipelineHasUnresolvedWriter(owner, source)'
           PROVE ReplyPipelineHasUnresolvedWriter(owner, source)
      <3>1. PICK writer \in rpItems':
               /\ writer.owner = owner
               /\ writer.source = source
               /\ writer.phase \in {"Admitted", "Flushed"}
        BY <2>1
           DEF ReplyPipelineHasUnresolvedWriter
      <3>2. \/ writer \in rpItems
             \/ writer = newItem
        BY <1>1, <3>1,
           ReplyPipelineReplacementMembership
           DEF ReplyPipelineReplaceItem
      <3>3. writer # newItem
        BY <1>1, <3>1, SMTT(5)
      <3>4. writer \in rpItems
        BY <3>2, <3>3, SMTT(5)
      <3> QED BY <3>1, <3>4
           DEF ReplyPipelineHasUnresolvedWriter
    <2>2. ASSUME NEW owner, NEW source,
                  ReplyPipelineHasUnresolvedWriter(owner, source)
           PROVE ReplyPipelineHasUnresolvedWriter(owner, source)'
      <3>1. PICK writer \in rpItems:
               /\ writer.owner = owner
               /\ writer.source = source
               /\ writer.phase \in {"Admitted", "Flushed"}
        BY <2>2
           DEF ReplyPipelineHasUnresolvedWriter
      <3>2. writer # oldItem
        BY <1>1, <3>1, SMTT(5)
      <3>3. writer \in rpItems'
        BY <1>1, <3>1, <3>2, SMTT(10)
           DEF ReplyPipelineReplaceItem
      <3> QED BY <3>1, <3>3
           DEF ReplyPipelineHasUnresolvedWriter
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyPipelinePayloadForTypedItemHasType ==
  \A item:
    /\ ReplyPipelineConfiguration
    /\ ReplyPipelineItemHasType(item)
    => ReplyPipelinePayloadForItem(item)
         \in ReplyPipelinePayloads
PROOF
  <1>1. ASSUME NEW item,
                ReplyPipelineConfiguration,
                ReplyPipelineItemHasType(item)
         PROVE ReplyPipelinePayloadForItem(item)
                 \in ReplyPipelinePayloads
    <2>1. /\ item.semantic \in ReplySemantics
           /\ item.messageCursor \in 0..ReplyMessageCount
           /\ item.chunkCursor \in 0..ReplyChunkCount
      BY <1>1 DEF ReplyPipelineItemHasType
    <2>2. /\ ReplySemanticTarget(item.semantic)
                  \in ReplyTargets
           /\ ReplyItemClass(item.semantic,
                              item.messageCursor,
                              item.chunkCursor)
                  \in ReplyOutputClasses
      BY <1>1, <2>1, SMTT(10)
         DEF ReplyPipelineConfiguration,
             ReplyRouteConfiguration
    <2> QED BY <2>1, <2>2, SMTT(15)
         DEF ReplyPipelinePayloadForItem,
             ReplyPipelinePayload,
             ReplyPipelinePayloads
  <1> QED BY <1>1

THEOREM ReplyPipelineItemWithTicketHasType ==
  \A item, ticketId:
    /\ ReplyPipelineConfiguration
    /\ ReplyPipelineItemHasType(item)
    /\ ticketId \in Nat
    => ReplyPipelineItemHasType(
         ReplyPipelineItemWithTicket(item, ticketId))
PROOF
  <1>1. ASSUME NEW item, NEW ticketId,
                ReplyPipelineConfiguration,
                ReplyPipelineItemHasType(item),
                ticketId \in Nat
         PROVE ReplyPipelineItemHasType(
                 ReplyPipelineItemWithTicket(item, ticketId))
    <2>1. LET ticketed ==
                 ReplyPipelineItemWithTicket(item, ticketId)
           IN /\ ticketed.owner = item.owner
              /\ ticketed.semantic = item.semantic
              /\ ticketed.source = item.source
              /\ ticketed.messageCursor = item.messageCursor
              /\ ticketed.chunkCursor = item.chunkCursor
              /\ ticketed.outputClass = item.outputClass
              /\ ticketed.flushRequired = item.flushRequired
              /\ ticketed.fifoOrdinal = item.fifoOrdinal
              /\ ticketed.routeTenure = item.routeTenure
              /\ ticketed.phase = "Ticketed"
              /\ ticketed.ticketId = ticketId
              /\ ticketed.ticketTenure = item.routeTenure
              /\ ticketed.ticketPayload =
                   {ReplyPipelinePayloadForItem(item)}
      BY ReplyPipelineItemWithTicketProjection
    <2>2. ReplyPipelinePayloadForItem(item)
             \in ReplyPipelinePayloads
      BY <1>1, ReplyPipelinePayloadForTypedItemHasType
    <2>3. /\ "Ticketed" \in ReplyPipelinePhases
           /\ item.routeTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ {ReplyPipelinePayloadForItem(item)}
                \in SUBSET ReplyPipelinePayloads
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyPipelinePhases,
             ReplyPipelineItemHasType,
             ReplyPipelineConfiguration,
             ReplyRouteConfiguration,
             ReplyConnectionTenures
    <2> QED BY ONLY <1>1, <2>1, <2>3, SMTT(15)
         DEF ReplyPipelineItemHasType
  <1> QED BY <1>1

THEOREM ReplyPositiveNatSuccessor ==
  \A value \in Nat \ {0}:
    value + 1 \in Nat \ {0}
BY SMT

THEOREM ReplyTicketCounterUpdatePreservesType ==
  \A counters \in [ReplyOwners -> Nat \ {0}],
     owner \in ReplyOwners:
    [counters EXCEPT ![owner] = @ + 1]
      \in [ReplyOwners -> Nat \ {0}]
PROOF
  <1>1. ASSUME NEW counters \in
                  [ReplyOwners -> Nat \ {0}],
                NEW owner \in ReplyOwners
         PROVE [counters EXCEPT ![owner] = @ + 1]
                 \in [ReplyOwners -> Nat \ {0}]
    <2>1. counters[owner] \in Nat \ {0}
      BY <1>1
    <2>2. counters[owner] + 1 \in Nat \ {0}
      BY <2>1, ReplyPositiveNatSuccessor
    <2> QED BY <1>1, <2>2,
         ReplyFunctionalUpdatePreservesType
  <1> QED BY <1>1

THEOREM ReplyPipelineFunctionalUpdateAtKey ==
  \A domain, codomain, mapping, key, value:
    /\ mapping \in [domain -> codomain]
    /\ key \in domain
    => [mapping EXCEPT ![key] = value][key] = value
BY Isa

THEOREM ReplyAcquirePipelineTicketPreservesTypeInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineTypeInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineTypeInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ item.owner = owner
              /\ item.semantic = semantic
              /\ item.source = source
      BY <1>1, ReplyOwnedPipelineItemProjection
         DEF AcquireReplyPipelineTicket
    <2>2. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN ReplyPipelineItemHasType(item)
      BY <1>1, <2>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant
    <2>3. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
               ticketId == rpNextTicketId[owner]
               ticketed ==
                 ReplyPipelineItemWithTicket(item, ticketId)
           IN ReplyPipelineItemHasType(ticketed)
      BY <1>1, <2>2,
         ReplyPipelineItemWithTicketHasType
         DEF ReplyPipelineInductiveInvariant,
             AcquireReplyPipelineTicket
    <2>4. rpPendingAttachments' \subseteq ReplyAttachmentSet
      BY <1>1, SMTT(5)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             AcquireReplyPipelineTicket, ReplyRouteVars
    <2>5. \A checkedItem \in rpItems':
             ReplyPipelineItemHasType(checkedItem)
      <3>1. ASSUME NEW checkedItem \in rpItems'
             PROVE ReplyPipelineItemHasType(checkedItem)
        <4>1. LET item ==
                     ReplyPipelineItemFor(owner, semantic, source)
                   ticketId == rpNextTicketId[owner]
                   ticketed ==
                     ReplyPipelineItemWithTicket(item, ticketId)
               IN \/ checkedItem \in rpItems
                  \/ checkedItem = ticketed
          BY <1>1, <3>1,
             ReplyPipelineReplacementMembership
             DEF AcquireReplyPipelineTicket,
                 ReplyPipelineReplaceItem
        <4> QED BY <1>1, <2>3, <4>1, SMTT(15)
             DEF ReplyPipelineInductiveInvariant,
                 ReplyPipelineTypeInvariant
      <3> QED BY <3>1
    <2>6. rpNextFifoOrdinal'
               \in [ReplyOwners ->
                     1..(ReplyPipelineOrdinalLimit + 1)]
      BY <1>1, SMTT(5)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             AcquireReplyPipelineTicket, ReplyRouteVars
    <2>7. rpNextTicketId'
               \in [ReplyOwners -> Nat \ {0}]
      BY <1>1, ReplyTicketCounterUpdatePreservesType
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             AcquireReplyPipelineTicket
    <2> QED BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyPipelineTypeInvariant
  <1> QED BY <1>1

THEOREM ReplyAcquirePipelineTicketPreservesPerIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelinePerIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelinePerIdentityInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
               ticketed ==
                 ReplyPipelineItemWithTicket(
                   item, rpNextTicketId[owner])
           IN /\ item \in rpItems
              /\ ticketed.owner = item.owner
              /\ ticketed.semantic = item.semantic
              /\ ticketed.source = item.source
      BY <1>1, ReplyOwnedPipelineItemProjection,
         ReplyPipelineItemWithTicketProjection
         DEF AcquireReplyPipelineTicket
    <2>2. ReplyPipelinePendingPerIdentityInvariant'
      <3>1. ReplyPipelinePendingPerIdentityInvariant
        BY <1>1
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelinePerIdentityInvariant
      <3>2. rpPendingAttachments' = rpPendingAttachments
        BY <1>1
           DEF AcquireReplyPipelineTicket, ReplyRouteVars
      <3> QED BY <3>1, <3>2
           DEF ReplyPipelinePendingPerIdentityInvariant
    <2>3. ReplyPipelineItemPerIdentityInvariant'
      BY <1>1, <2>1,
         ReplyPipelineReplacementPreservesPerIdentity
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelinePerIdentityInvariant,
             ReplyPipelineItemPerIdentityInvariant,
             AcquireReplyPipelineTicket,
             ReplyPipelineReplaceItem
    <2> QED BY <2>2, <2>3
         DEF ReplyPipelinePerIdentityInvariant
  <1> QED BY <1>1

THEOREM ReplyAcquirePipelineTicketPreservesFifoOrdinalInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineFifoOrdinalInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineFifoOrdinalInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
               ticketed ==
                 ReplyPipelineItemWithTicket(
                   item, rpNextTicketId[owner])
           IN /\ item \in rpItems
              /\ ticketed.owner = item.owner
              /\ ticketed.fifoOrdinal = item.fifoOrdinal
      BY <1>1, ReplyOwnedPipelineItemProjection,
         ReplyPipelineItemWithTicketProjection
         DEF AcquireReplyPipelineTicket
    <2>2. \A left, right \in rpItems':
             /\ left.owner = right.owner
             /\ left.fifoOrdinal = right.fifoOrdinal
             => left = right
      BY <1>1, <2>1,
         ReplyPipelineReplacementPreservesFifoIdentity
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineFifoOrdinalInvariant,
             AcquireReplyPipelineTicket,
             ReplyPipelineReplaceItem
    <2>3. \A item \in rpItems':
             item.fifoOrdinal < rpNextFifoOrdinal'[item.owner]
      <3>1. rpNextFifoOrdinal' = rpNextFifoOrdinal
        BY <1>1
           DEF AcquireReplyPipelineTicket, ReplyRouteVars
      <3> QED BY <1>1, <2>1, <3>1,
           ReplyPipelineReplacementPreservesFifoBound
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineFifoOrdinalInvariant,
               AcquireReplyPipelineTicket,
               ReplyPipelineReplaceItem
    <2> QED BY <2>2, <2>3
         DEF ReplyPipelineFifoOrdinalInvariant
  <1> QED BY <1>1

THEOREM ReplyAcquirePipelineTicketIsFresh ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => LET item ==
             ReplyPipelineItemFor(owner, semantic, source)
           ticketed ==
             ReplyPipelineItemWithTicket(
               item, rpNextTicketId[owner])
       IN /\ ticketed.ticketId # NoReplyPipelineTicket
          /\ \A checkedItem \in rpItems:
               checkedItem.owner = ticketed.owner =>
                 checkedItem.ticketId # ticketed.ticketId
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE LET item ==
                     ReplyPipelineItemFor(owner, semantic, source)
                   ticketed ==
                     ReplyPipelineItemWithTicket(
                       item, rpNextTicketId[owner])
               IN /\ ticketed.ticketId #
                        NoReplyPipelineTicket
                  /\ \A checkedItem \in rpItems:
                       checkedItem.owner = ticketed.owner =>
                         checkedItem.ticketId # ticketed.ticketId
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
               ticketed ==
                 ReplyPipelineItemWithTicket(
                   item, rpNextTicketId[owner])
           IN /\ ticketed.owner = owner
              /\ ticketed.ticketId = rpNextTicketId[owner]
      BY <1>1, ReplyOwnedPipelineItemProjection,
         ReplyPipelineItemWithTicketProjection
         DEF AcquireReplyPipelineTicket
    <2>2. rpNextTicketId[owner] \in Nat \ {0}
      BY <1>1
         DEF AcquireReplyPipelineTicket
    <2>3. \A checkedItem \in rpItems:
             checkedItem.owner = owner =>
               checkedItem.ticketId # rpNextTicketId[owner]
      <3>1. ASSUME NEW checkedItem \in rpItems,
                    checkedItem.owner = owner
             PROVE checkedItem.ticketId #
                     rpNextTicketId[owner]
        <4>1. ReplyPipelineItemPhaseBinding(checkedItem)
          BY <1>1, <3>1
             DEF ReplyPipelineInductiveInvariant,
                 ReplyPipelineOwnershipInvariant,
                 ReplyPipelineItemBindingInvariant
        <4>2. CASE checkedItem.phase = "Queued"
          BY <2>2, <4>1, <4>2, SMT
             DEF ReplyPipelineItemPhaseBinding,
                 ReplyPipelineQueuedItem,
                 NoReplyPipelineTicket
        <4>3. CASE checkedItem.phase # "Queued"
          BY <3>1, <4>1, <4>3, SMT
             DEF ReplyPipelineItemPhaseBinding,
                 ReplyPipelineTicketValid
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2, <2>3, SMTT(10)
         DEF NoReplyPipelineTicket
  <1> QED BY <1>1

THEOREM ReplyAcquirePipelineTicketPreservesTicketIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineTicketIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineTicketIdentityInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
               ticketed ==
                 ReplyPipelineItemWithTicket(
                   item, rpNextTicketId[owner])
           IN /\ item \in rpItems
              /\ ticketed.ticketId # NoReplyPipelineTicket
              /\ \A checkedItem \in rpItems:
                   checkedItem.owner = ticketed.owner =>
                     checkedItem.ticketId # ticketed.ticketId
      BY <1>1, ReplyOwnedPipelineItemProjection,
         ReplyAcquirePipelineTicketIsFresh
         DEF AcquireReplyPipelineTicket
    <2> QED BY <1>1, <2>1,
         ReplyPipelineReplacementPreservesTicketIdentity
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineTicketIdentityInvariant,
             AcquireReplyPipelineTicket,
             ReplyPipelineReplaceItem
  <1> QED BY <1>1

THEOREM ReplyAcquireTicketedItemFacts ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A ticketedItem:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      /\ ticketedItem =
           ReplyPipelineItemWithTicket(
             ReplyPipelineItemFor(owner, semantic, source),
             rpNextTicketId[owner])
      => LET item ==
               ReplyPipelineItemFor(owner, semantic, source)
         IN /\ item \in rpItems
            /\ item.owner = owner
            /\ item.semantic = semantic
            /\ item.source = source
            /\ ReplyPipelineItemHasType(item)
            /\ ticketedItem.owner = item.owner
            /\ ticketedItem.semantic = item.semantic
            /\ ticketedItem.source = item.source
            /\ ticketedItem.messageCursor = item.messageCursor
            /\ ticketedItem.chunkCursor = item.chunkCursor
            /\ ticketedItem.outputClass = item.outputClass
            /\ ticketedItem.flushRequired = item.flushRequired
            /\ ticketedItem.fifoOrdinal = item.fifoOrdinal
            /\ ticketedItem.fifoOrdinal \in Nat
            /\ ticketedItem.routeTenure = item.routeTenure
            /\ ticketedItem.phase = "Ticketed"
            /\ ticketedItem.ticketId = rpNextTicketId[owner]
            /\ ticketedItem.ticketTenure = item.routeTenure
            /\ ticketedItem.ticketPayload =
                 {ReplyPipelinePayloadForItem(item)}
            /\ ReplyPipelineItemIsFifoHead(item)
            /\ rpItems' =
                 ReplyPipelineReplaceItem(item, ticketedItem)
            /\ UNCHANGED ReplyRouteVars
            /\ rpPendingAttachments' = rpPendingAttachments
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW ticketedItem,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source),
                ticketedItem =
                  ReplyPipelineItemWithTicket(
                    ReplyPipelineItemFor(
                      owner, semantic, source),
                    rpNextTicketId[owner])
         PROVE LET item ==
                     ReplyPipelineItemFor(
                       owner, semantic, source)
               IN /\ item \in rpItems
                  /\ item.owner = owner
                  /\ item.semantic = semantic
                  /\ item.source = source
                  /\ ReplyPipelineItemHasType(item)
                  /\ ticketedItem.owner = item.owner
                  /\ ticketedItem.semantic = item.semantic
                  /\ ticketedItem.source = item.source
                  /\ ticketedItem.messageCursor = item.messageCursor
                  /\ ticketedItem.chunkCursor = item.chunkCursor
                  /\ ticketedItem.outputClass = item.outputClass
                  /\ ticketedItem.flushRequired = item.flushRequired
                  /\ ticketedItem.fifoOrdinal = item.fifoOrdinal
                  /\ ticketedItem.fifoOrdinal \in Nat
                  /\ ticketedItem.routeTenure = item.routeTenure
                  /\ ticketedItem.phase = "Ticketed"
                  /\ ticketedItem.ticketId =
                       rpNextTicketId[owner]
                  /\ ticketedItem.ticketTenure = item.routeTenure
                  /\ ticketedItem.ticketPayload =
                       {ReplyPipelinePayloadForItem(item)}
                  /\ ReplyPipelineItemIsFifoHead(item)
                  /\ rpItems' =
                       ReplyPipelineReplaceItem(item, ticketedItem)
                  /\ UNCHANGED ReplyRouteVars
                  /\ rpPendingAttachments' = rpPendingAttachments
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ item.owner = owner
              /\ item.semantic = semantic
              /\ item.source = source
              /\ ReplyPipelineItemHasType(item)
      BY <1>1, ReplyOwnedPipelineItemProjection, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             AcquireReplyPipelineTicket
    <2>2. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ ticketedItem.owner = item.owner
              /\ ticketedItem.semantic = item.semantic
              /\ ticketedItem.source = item.source
              /\ ticketedItem.messageCursor = item.messageCursor
              /\ ticketedItem.chunkCursor = item.chunkCursor
              /\ ticketedItem.outputClass = item.outputClass
              /\ ticketedItem.flushRequired = item.flushRequired
              /\ ticketedItem.fifoOrdinal = item.fifoOrdinal
              /\ ticketedItem.routeTenure = item.routeTenure
              /\ ticketedItem.phase = "Ticketed"
              /\ ticketedItem.ticketId = rpNextTicketId[owner]
              /\ ticketedItem.ticketTenure = item.routeTenure
              /\ ticketedItem.ticketPayload =
                   {ReplyPipelinePayloadForItem(item)}
      BY <1>1, ReplyPipelineItemWithTicketProjection
    <2>3. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ ticketedItem.fifoOrdinal \in Nat
              /\ ReplyPipelineItemIsFifoHead(item)
              /\ rpItems' =
                   ReplyPipelineReplaceItem(item, ticketedItem)
              /\ UNCHANGED ReplyRouteVars
              /\ rpPendingAttachments' = rpPendingAttachments
      BY <1>1, <2>1, <2>2, SMTT(15)
         DEF ReplyPipelineItemHasType,
             AcquireReplyPipelineTicket,
             ReplyRouteVars
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyAcquirePreservesExistingItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item \in rpItems:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      => ReplyPipelineItemCoreBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item \in rpItems,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineItemCoreBinding(item)'
    <2>1. ReplyPipelineItemCoreBinding(item)
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant
    <2>2. UNCHANGED ReplyRouteVars
      BY <1>1
         DEF AcquireReplyPipelineTicket
    <2> QED BY <2>1, <2>2,
         ReplyRouteStutterPreservesItemCoreBinding
  <1> QED BY <1>1

THEOREM ReplyAcquireEstablishesTicketedItemCoreBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A ticketedItem:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      /\ ticketedItem =
           ReplyPipelineItemWithTicket(
             ReplyPipelineItemFor(owner, semantic, source),
             rpNextTicketId[owner])
      => ReplyPipelineItemCoreBinding(ticketedItem)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW ticketedItem,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source),
                ticketedItem =
                  ReplyPipelineItemWithTicket(
                    ReplyPipelineItemFor(
                      owner, semantic, source),
                    rpNextTicketId[owner])
         PROVE ReplyPipelineItemCoreBinding(ticketedItem)'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ ReplyPipelineItemCoreBinding(item)
      BY <1>1, ReplyAcquireTicketedItemFacts, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant
    <2>2. ReplyPipelineItemCoreBinding(ticketedItem)
      BY <1>1, <2>1,
         ReplyPipelineItemWithTicketPreservesCoreBinding
    <2>3. UNCHANGED ReplyRouteVars
      BY <1>1
         DEF AcquireReplyPipelineTicket
    <2> QED BY <2>2, <2>3,
         ReplyRouteStutterPreservesItemCoreBinding
  <1> QED BY <1>1

THEOREM ReplyAcquirePreservesExistingItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item \in rpItems:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      => ReplyPipelineItemRouteBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item \in rpItems,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineItemRouteBinding(item)'
    <2>1. ReplyPipelineItemRouteBinding(item)
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant
    <2>2. /\ UNCHANGED ReplyRouteVars
           /\ rpPendingAttachments' = rpPendingAttachments
      BY <1>1, SMTT(5)
         DEF AcquireReplyPipelineTicket,
             ReplyRouteVars
    <2> QED BY <2>1, <2>2,
         ReplyRouteAndPendingStutterPreservesItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyAcquireEstablishesTicketedItemRouteBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A ticketedItem:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      /\ ticketedItem =
           ReplyPipelineItemWithTicket(
             ReplyPipelineItemFor(owner, semantic, source),
             rpNextTicketId[owner])
      => ReplyPipelineItemRouteBinding(ticketedItem)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW ticketedItem,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source),
                ticketedItem =
                  ReplyPipelineItemWithTicket(
                    ReplyPipelineItemFor(
                      owner, semantic, source),
                    rpNextTicketId[owner])
         PROVE ReplyPipelineItemRouteBinding(ticketedItem)'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ ReplyPipelineItemRouteBinding(item)
      BY <1>1, ReplyAcquireTicketedItemFacts, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant
    <2>2. ReplyPipelineItemRouteBinding(ticketedItem)
      BY <1>1, <2>1,
         ReplyPipelineItemWithTicketPreservesRouteBinding
    <2>3. /\ UNCHANGED ReplyRouteVars
           /\ rpPendingAttachments' = rpPendingAttachments
      BY <1>1, SMTT(5)
         DEF AcquireReplyPipelineTicket,
             ReplyRouteVars
    <2> QED BY <2>2, <2>3,
         ReplyRouteAndPendingStutterPreservesItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyAcquirePreservesExistingTicketValid ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      /\ item \in rpItems
      /\ ReplyPipelineTicketValid(item)
      => ReplyPipelineTicketValid(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source),
                item \in rpItems,
                ReplyPipelineTicketValid(item)
         PROVE ReplyPipelineTicketValid(item)'
    <2>1. /\ item.ticketId \in Nat \ {0}
           /\ item.ticketId < rpNextTicketId[item.owner]
      BY <1>1 DEF ReplyPipelineTicketValid
    <2>2. /\ rpNextTicketId
                  \in [ReplyOwners -> Nat \ {0}]
           /\ owner \in ReplyOwners
           /\ item.owner \in ReplyOwners
      BY <1>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineItemHasType,
             ReplyPipelineTicketValid
    <2>3. item.ticketId < rpNextTicketId'[item.owner]
      BY <1>1, <2>1, <2>2, SMTT(15)
         DEF AcquireReplyPipelineTicket
    <2> QED BY <1>1, <2>3, SMTT(10)
         DEF ReplyPipelineTicketValid
  <1> QED BY <1>1

THEOREM ReplyAcquireEstablishesTicketedItemValid ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A ticketedItem:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      /\ ticketedItem =
           ReplyPipelineItemWithTicket(
             ReplyPipelineItemFor(owner, semantic, source),
             rpNextTicketId[owner])
      => ReplyPipelineTicketValid(ticketedItem)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW ticketedItem,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source),
                ticketedItem =
                  ReplyPipelineItemWithTicket(
                    ReplyPipelineItemFor(
                      owner, semantic, source),
                    rpNextTicketId[owner])
         PROVE ReplyPipelineTicketValid(ticketedItem)'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ ticketedItem.owner = owner
              /\ ticketedItem.phase = "Ticketed"
              /\ ticketedItem.ticketId = rpNextTicketId[owner]
              /\ ticketedItem.ticketTenure = item.routeTenure
              /\ ticketedItem.routeTenure = item.routeTenure
              /\ ticketedItem.ticketPayload =
                   {ReplyPipelinePayloadForItem(item)}
              /\ ticketedItem.semantic = item.semantic
              /\ ticketedItem.messageCursor = item.messageCursor
              /\ ticketedItem.chunkCursor = item.chunkCursor
      BY <1>1, ReplyAcquireTicketedItemFacts
    <2>2. rpNextTicketId[owner] \in Nat \ {0}
      BY <1>1
         DEF AcquireReplyPipelineTicket
    <2>3. rpNextTicketId' =
             [rpNextTicketId EXCEPT
                ![owner] = rpNextTicketId[owner] + 1]
      BY <1>1
         DEF AcquireReplyPipelineTicket
    <2>4. rpNextTicketId
             \in [ReplyOwners -> Nat \ {0}]
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant
    <2>5. rpNextTicketId'[owner] =
             rpNextTicketId[owner] + 1
      BY <1>1, <2>3, <2>4,
         ReplyPipelineFunctionalUpdateAtKey
    <2>6. ticketedItem.ticketId <
             rpNextTicketId'[ticketedItem.owner]
      BY <2>1, <2>2, <2>5, SMT
    <2> QED BY <2>1, <2>2, <2>6, SMTT(10)
         DEF ReplyPipelineTicketValid,
             ReplyPipelinePayloadForItem
  <1> QED BY <1>1

THEOREM ReplyAcquirePreservesExistingItemPhaseBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A item \in rpItems:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      => ReplyPipelineItemPhaseBinding(item)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW item \in rpItems,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineItemPhaseBinding(item)'
    <2>1. ReplyPipelineItemPhaseBinding(item)
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant
    <2>2. CASE item.phase = "Queued"
      <3>1. ReplyPipelineQueuedItem(item)
        BY <2>1, <2>2
           DEF ReplyPipelineItemPhaseBinding
      <3>2. ReplyPipelineQueuedItem(item)'
        BY <3>1, SMTT(5)
           DEF ReplyPipelineQueuedItem
      <3> QED BY <3>2,
           ReplyQueuedItemEstablishesPhaseBindingPrime
    <2>3. CASE item.phase # "Queued"
      <3>1. /\ ReplyPipelineTicketValid(item)
             /\ ReplyPipelineItemIsFifoHead(item)
             /\ (item.phase \in {"Admitted", "Flushed"} =>
                   item.flushRequired)
        BY <2>1, <2>3
           DEF ReplyPipelineItemPhaseBinding
      <3>2. ReplyPipelineTicketValid(item)'
        BY <1>1, <3>1,
           ReplyAcquirePreservesExistingTicketValid
      <3>3. LET oldItem ==
                   ReplyPipelineItemFor(owner, semantic, source)
                 newItem ==
                   ReplyPipelineItemWithTicket(
                     oldItem, rpNextTicketId[owner])
             IN /\ oldItem \in rpItems
                /\ newItem.owner = oldItem.owner
                /\ newItem.source = oldItem.source
                /\ newItem.outputClass = oldItem.outputClass
                /\ newItem.fifoOrdinal = oldItem.fifoOrdinal
                /\ newItem.fifoOrdinal \in Nat
                /\ rpItems' =
                     ReplyPipelineReplaceItem(oldItem, newItem)
        BY <1>1, ReplyAcquireTicketedItemFacts
      <3>4. ReplyPipelineItemIsFifoHead(item)'
        BY <3>1, <3>3,
           ReplyPipelineReplacementPreservesFifoHead
      <3>5. (item.phase \in {"Admitted", "Flushed"} =>
                 item.flushRequired)'
        BY <3>1, SMTT(5)
      <3> QED BY <2>3, <3>2, <3>4, <3>5
           DEF ReplyPipelineItemPhaseBinding
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyAcquireEstablishesTicketedItemPhaseBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A ticketedItem:
      /\ ReplyPipelineInductiveInvariant
      /\ AcquireReplyPipelineTicket(owner, semantic, source)
      /\ ticketedItem =
           ReplyPipelineItemWithTicket(
             ReplyPipelineItemFor(owner, semantic, source),
             rpNextTicketId[owner])
      => ReplyPipelineItemPhaseBinding(ticketedItem)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW ticketedItem,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source),
                ticketedItem =
                  ReplyPipelineItemWithTicket(
                    ReplyPipelineItemFor(
                      owner, semantic, source),
                    rpNextTicketId[owner])
         PROVE ReplyPipelineItemPhaseBinding(ticketedItem)'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ ticketedItem.owner = item.owner
              /\ ticketedItem.source = item.source
              /\ ticketedItem.outputClass = item.outputClass
              /\ ticketedItem.fifoOrdinal = item.fifoOrdinal
              /\ ticketedItem.fifoOrdinal \in Nat
              /\ ticketedItem.phase = "Ticketed"
              /\ ReplyPipelineItemIsFifoHead(item)
              /\ rpItems' =
                   ReplyPipelineReplaceItem(item, ticketedItem)
      BY <1>1, ReplyAcquireTicketedItemFacts
    <2>2. ReplyPipelineTicketValid(ticketedItem)'
      BY <1>1,
         ReplyAcquireEstablishesTicketedItemValid
    <2>3. ReplyPipelineItemIsFifoHead(ticketedItem)'
      BY <2>1,
         ReplyPipelineReplacementEstablishesNewFifoHead
    <2> QED BY <2>1, <2>2, <2>3, SMTT(10)
         DEF ReplyPipelineItemPhaseBinding
  <1> QED BY <1>1

THEOREM ReplyAcquirePipelineTicketPreservesItemBindingInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineItemBindingInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineItemBindingInvariant'
    <2>1. ASSUME NEW checkedItem \in rpItems'
           PROVE /\ ReplyPipelineItemCoreBinding(checkedItem)'
                 /\ ReplyPipelineItemRouteBinding(checkedItem)'
                 /\ ReplyPipelineItemPhaseBinding(checkedItem)'
      <3>1. LET item ==
                   ReplyPipelineItemFor(owner, semantic, source)
                 ticketed ==
                   ReplyPipelineItemWithTicket(
                     item, rpNextTicketId[owner])
             IN \/ checkedItem \in rpItems
                \/ checkedItem = ticketed
        BY <1>1, <2>1,
           ReplyPipelineReplacementMembership
           DEF AcquireReplyPipelineTicket,
               ReplyPipelineReplaceItem
      <3>2. CASE checkedItem \in rpItems
        <4>1. ReplyPipelineItemCoreBinding(checkedItem)'
          BY <1>1, <3>2,
             ReplyAcquirePreservesExistingItemCoreBinding
        <4>2. ReplyPipelineItemRouteBinding(checkedItem)'
          BY <1>1, <3>2,
             ReplyAcquirePreservesExistingItemRouteBinding
        <4>3. ReplyPipelineItemPhaseBinding(checkedItem)'
          BY <1>1, <3>2,
             ReplyAcquirePreservesExistingItemPhaseBinding
        <4> QED BY <4>1, <4>2, <4>3
      <3>3. CASE checkedItem =
                    ReplyPipelineItemWithTicket(
                      ReplyPipelineItemFor(
                        owner, semantic, source),
                      rpNextTicketId[owner])
        <4>1. ReplyPipelineItemCoreBinding(checkedItem)'
          BY <1>1, <3>3,
             ReplyAcquireEstablishesTicketedItemCoreBinding
        <4>2. ReplyPipelineItemRouteBinding(checkedItem)'
          BY <1>1, <3>3,
             ReplyAcquireEstablishesTicketedItemRouteBinding
        <4>3. ReplyPipelineItemPhaseBinding(checkedItem)'
          BY <1>1, <3>3,
             ReplyAcquireEstablishesTicketedItemPhaseBinding
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>1
         DEF ReplyPipelineItemBindingInvariant
  <1> QED BY <1>1

THEOREM ReplyAcquirePreservesReconnectNoTicketedInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineReconnectNoTicketedInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineReconnectNoTicketedInvariant'
    <2>1. ASSUME NEW checkedOwner \in ReplyOwners,
                  NEW checkedSource \in ReplySources,
                  ReplyReconnectPendingForSource(
                    checkedOwner, checkedSource)'
           PROVE \A checkedItem \in rpItems':
                   (checkedItem.owner = checkedOwner /\
                    checkedItem.source = checkedSource) =>
                     checkedItem.phase # "Ticketed"
      <3>1. /\ ReplyReconnectPendingForSource(
                       checkedOwner, checkedSource)
             /\ ~ReplyReconnectPendingForSource(owner, source)
        BY <1>1, <2>1, SMTT(15)
           DEF AcquireReplyPipelineTicket,
               ReplyReconnectPendingForSource,
               ReplyReconnectPending,
               ReplyPendingAttachmentsFor,
               ReplyRouteVars
      <3>2. ASSUME NEW checkedItem \in rpItems',
                    checkedItem.owner = checkedOwner,
                    checkedItem.source = checkedSource
             PROVE checkedItem.phase # "Ticketed"
        <4>1. LET item ==
                     ReplyPipelineItemFor(owner, semantic, source)
                   ticketed ==
                     ReplyPipelineItemWithTicket(
                       item, rpNextTicketId[owner])
               IN \/ checkedItem \in rpItems
                  \/ checkedItem = ticketed
          BY <1>1, <3>2,
             ReplyPipelineReplacementMembership
             DEF AcquireReplyPipelineTicket,
                 ReplyPipelineReplaceItem
        <4>2. CASE checkedItem \in rpItems
          BY <1>1, <3>1, <3>2, <4>2, SMTT(15)
             DEF ReplyPipelineInductiveInvariant,
                 ReplyPipelineOwnershipInvariant,
                 ReplyPipelineReconnectBarrierInvariant,
                 ReplyPipelineReconnectNoTicketedInvariant
        <4>3. CASE checkedItem =
                      ReplyPipelineItemWithTicket(
                        ReplyPipelineItemFor(
                          owner, semantic, source),
                        rpNextTicketId[owner])
          BY <1>1, <3>1, <3>2, <4>3,
             ReplyAcquireTicketedItemFacts, SMTT(15)
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>2
    <2> QED BY <2>1
         DEF ReplyPipelineReconnectNoTicketedInvariant
  <1> QED BY <1>1

THEOREM ReplyAcquirePreservesReconnectWriterActiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineReconnectWriterActiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineReconnectWriterActiveInvariant'
    <2>1. LET oldItem ==
                 ReplyPipelineItemFor(owner, semantic, source)
               newItem ==
                 ReplyPipelineItemWithTicket(
                   oldItem, rpNextTicketId[owner])
           IN /\ oldItem \in rpItems
              /\ oldItem.phase = "Queued"
              /\ newItem.phase = "Ticketed"
              /\ newItem.owner = oldItem.owner
              /\ newItem.source = oldItem.source
              /\ rpItems' =
                   ReplyPipelineReplaceItem(oldItem, newItem)
      BY <1>1, ReplyAcquireTicketedItemFacts, SMTT(10)
         DEF AcquireReplyPipelineTicket,
             ReplyPipelineQueuedItem
    <2>2. \A checkedOwner, checkedSource:
             ReplyPipelineHasUnresolvedWriter(
               checkedOwner, checkedSource)' <=>
               ReplyPipelineHasUnresolvedWriter(
                 checkedOwner, checkedSource)
      BY <2>1,
         ReplyPipelineReplacementPreservesWriterView
    <2>3. \A checkedOwner, checkedSource:
             ReplyReconnectPendingForSource(
               checkedOwner, checkedSource)' <=>
               ReplyReconnectPendingForSource(
                 checkedOwner, checkedSource)
      BY <1>1, SMTT(15)
         DEF AcquireReplyPipelineTicket,
             ReplyReconnectPendingForSource,
             ReplyReconnectPending,
             ReplyPendingAttachmentsFor,
             ReplyRouteVars
    <2>4. rrSourceActive' = rrSourceActive
      BY <1>1, SMTT(5)
         DEF AcquireReplyPipelineTicket,
             ReplyRouteVars
    <2>5. ReplyPipelineReconnectWriterActiveInvariant
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineReconnectBarrierInvariant
    <2> QED BY <2>2, <2>3, <2>4, <2>5, SMTT(20)
         DEF ReplyPipelineReconnectWriterActiveInvariant
  <1> QED BY <1>1

THEOREM ReplyAcquirePipelineTicketPreservesReconnectBarrierInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineReconnectBarrierInvariant'
BY ReplyAcquirePreservesReconnectNoTicketedInvariant,
   ReplyAcquirePreservesReconnectWriterActiveInvariant,
   SMTT(10)
   DEF ReplyPipelineReconnectBarrierInvariant

THEOREM ReplyAcquirePipelineTicketPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AcquireReplyPipelineTicket(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AcquireReplyPipelineTicket(owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStutterPreservesInductiveInvariant
         DEF ReplyPipelineInductiveInvariant,
             AcquireReplyPipelineTicket, ReplyRouteVars
    <2>2. ReplyPipelineConfiguration'
      BY <1>1 DEF ReplyPipelineInductiveInvariant
    <2>3. ReplyPipelineTypeInvariant'
      BY <1>1,
         ReplyAcquirePipelineTicketPreservesTypeInvariant
    <2>4. ReplyPipelinePerIdentityInvariant'
      BY <1>1,
         ReplyAcquirePipelineTicketPreservesPerIdentityInvariant
    <2>5. ReplyPipelineFifoOrdinalInvariant'
      BY <1>1,
         ReplyAcquirePipelineTicketPreservesFifoOrdinalInvariant
    <2>6. ReplyPipelineTicketIdentityInvariant'
      BY <1>1,
         ReplyAcquirePipelineTicketPreservesTicketIdentityInvariant
    <2>7. ReplyPipelineItemBindingInvariant'
      BY <1>1,
         ReplyAcquirePipelineTicketPreservesItemBindingInvariant
    <2>8. ReplyPipelineReconnectBarrierInvariant'
      BY <1>1,
         ReplyAcquirePipelineTicketPreservesReconnectBarrierInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                  <2>5, <2>6, <2>7, <2>8
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
  <1> QED BY <1>1

THEOREM ReplyPipelineAdvancePreservesRouteInvariant ==
  \A item:
    /\ ReplyPipelineItemHasType(item)
    /\ ReplyRouteInductiveInvariant
    /\ ~ReplyAttemptComplete(
          ReplyAttemptFor(item.owner, item.semantic, item.source))
    /\ ReplyPipelineAdvanceAttempt(item)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW item,
                ReplyPipelineItemHasType(item),
                ReplyRouteInductiveInvariant,
                ~ReplyAttemptComplete(
                  ReplyAttemptFor(
                    item.owner, item.semantic, item.source)),
                ReplyPipelineAdvanceAttempt(item)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. /\ item.owner \in ReplyOwners
           /\ item.semantic \in ReplySemantics
           /\ item.source \in ReplySources
      BY <1>1 DEF ReplyPipelineItemHasType
    <2>2. AdvanceCurrentReplyAttempt(
             item.owner, item.semantic, item.source)
      BY <1>1 DEF ReplyPipelineAdvanceAttempt
    <2> QED BY <1>1, <2>1, <2>2,
         AdvanceCurrentReplyAttemptPreservesInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplyAdmitSelectedItemFacts ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineTypeInvariant
    /\ ReplyPipelineItemBindingInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => LET item == ReplyPipelineItemFor(owner, semantic, source)
       IN /\ item \in rpItems
          /\ ReplyPipelineItemHasType(item)
          /\ ReplyPipelineItemCoreBinding(item)
          /\ ReplyPipelineItemRouteBinding(item)
          /\ ReplyPipelineItemPhaseBinding(item)
          /\ item.owner = owner
          /\ item.semantic = semantic
          /\ item.source = source
          /\ item.phase = "Ticketed"
          /\ ReplyPipelineTicketValid(item)
          /\ ReplyPipelineItemIsFifoHead(item)
          /\ ~ReplyReconnectPendingForSource(owner, source)
          /\ UNCHANGED <<rpPendingAttachments,
                         rpNextFifoOrdinal, rpNextTicketId>>
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineTypeInvariant,
                ReplyPipelineItemBindingInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE LET item ==
                     ReplyPipelineItemFor(owner, semantic, source)
               IN /\ item \in rpItems
                  /\ ReplyPipelineItemHasType(item)
                  /\ ReplyPipelineItemCoreBinding(item)
                  /\ ReplyPipelineItemRouteBinding(item)
                  /\ ReplyPipelineItemPhaseBinding(item)
                  /\ item.owner = owner
                  /\ item.semantic = semantic
                  /\ item.source = source
                  /\ item.phase = "Ticketed"
                  /\ ReplyPipelineTicketValid(item)
                  /\ ReplyPipelineItemIsFifoHead(item)
                  /\ ~ReplyReconnectPendingForSource(owner, source)
                  /\ UNCHANGED <<rpPendingAttachments,
                                 rpNextFifoOrdinal, rpNextTicketId>>
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ item.owner = owner
              /\ item.semantic = semantic
              /\ item.source = source
      BY <1>1, ReplyOwnedPipelineItemProjection
         DEF AdmitReplyPipelineItem
    <2>2. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN ReplyPipelineItemHasType(item)
      BY <1>1, <2>1
         DEF ReplyPipelineTypeInvariant
    <2>3. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ ReplyPipelineItemCoreBinding(item)
              /\ ReplyPipelineItemRouteBinding(item)
              /\ ReplyPipelineItemPhaseBinding(item)
      BY <1>1, <2>1
         DEF ReplyPipelineItemBindingInvariant
    <2>4. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item.phase = "Ticketed"
              /\ ReplyPipelineTicketValid(item)
              /\ ReplyPipelineItemIsFifoHead(item)
              /\ ~ReplyReconnectPendingForSource(owner, source)
              /\ UNCHANGED <<rpPendingAttachments,
                             rpNextFifoOrdinal, rpNextTicketId>>
      BY <1>1 DEF AdmitReplyPipelineItem,
                     ReplyPipelineItemFor,
                     ReplyPipelineItemsFor
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplyAdmitPreservesRouteInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ ReplyPipelineItemHasType(item)
              /\ item.owner = owner
              /\ item.semantic = semantic
              /\ item.source = source
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>2. CASE ReplyPipelineItemFor(
                    owner, semantic, source).flushRequired
      <3>1. UNCHANGED ReplyRouteVars
        BY <1>1, <2>2, SMTT(5)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineFlushAdmission,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3> QED BY <1>1, <3>1,
           ReplyRouteStutterPreservesInductiveInvariant
           DEF ReplyPipelineInductiveInvariant
    <2>3. CASE ~ReplyPipelineItemFor(
                     owner, semantic, source).flushRequired
      <3>1. ReplyPipelineAdvanceAttempt(
               ReplyPipelineItemFor(owner, semantic, source))
        BY <1>1, <2>3, SMTT(5)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3>2. AdvanceCurrentReplyAttempt(
               owner, semantic, source)
        BY <2>1, <3>1, SMTT(10)
           DEF ReplyPipelineAdvanceAttempt
      <3> QED BY <1>1, <3>2,
           AdvanceCurrentReplyAttemptPreservesInductiveInvariant
           DEF ReplyPipelineInductiveInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyPipelinePhaseUpdateProjection ==
  \A item, phase:
    LET updated == ReplyPipelineItemWithPhase(item, phase)
    IN /\ updated.owner = item.owner
       /\ updated.semantic = item.semantic
       /\ updated.source = item.source
       /\ updated.messageCursor = item.messageCursor
       /\ updated.chunkCursor = item.chunkCursor
       /\ updated.outputClass = item.outputClass
       /\ updated.flushRequired = item.flushRequired
       /\ updated.fifoOrdinal = item.fifoOrdinal
       /\ updated.routeTenure = item.routeTenure
       /\ updated.phase = phase
       /\ updated.ticketId = item.ticketId
       /\ updated.ticketTenure = item.ticketTenure
       /\ updated.ticketPayload = item.ticketPayload
BY SMTT(10)
   DEF ReplyPipelineItemWithPhase,
       ReplyPipelineRawItem

THEOREM ReplyPipelinePhaseUpdateHasType ==
  \A item, phase:
    /\ ReplyPipelineItemHasType(item)
    /\ phase \in ReplyPipelinePhases
    => ReplyPipelineItemHasType(
         ReplyPipelineItemWithPhase(item, phase))
BY ReplyPipelinePhaseUpdateProjection, SMTT(15)
   DEF ReplyPipelineItemHasType

THEOREM ReplyPipelineReplacementPreservesSameTicketIdentity ==
  \A items, oldItem, newItem:
    /\ oldItem \in items
    /\ newItem.owner = oldItem.owner
    /\ newItem.ticketId = oldItem.ticketId
    /\ \A left, right \in items:
         /\ left.owner = right.owner
         /\ left.ticketId # NoReplyPipelineTicket
         /\ left.ticketId = right.ticketId
         => left = right
    => \A left, right \in
            ((items \ {oldItem}) \cup {newItem}):
         /\ left.owner = right.owner
         /\ left.ticketId # NoReplyPipelineTicket
         /\ left.ticketId = right.ticketId
         => left = right
BY SMTT(30)

THEOREM ReplyPipelineRemovalPreservesFifoHead ==
  \A items, removed, item:
    /\ ReplyPipelineItemIsFifoHead(item)
    /\ rpItems = items
    /\ rpItems' = items \ {removed}
    => ReplyPipelineItemIsFifoHead(item)'
BY SMTT(20)
   DEF ReplyPipelineItemIsFifoHead,
       ReplyPipelineItemsInLane

THEOREM ReplyAdmitPreservesTypeInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineConfiguration
    /\ ReplyPipelineTypeInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineTypeInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineConfiguration,
                ReplyPipelineTypeInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE ReplyPipelineTypeInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ ReplyPipelineItemHasType(item)
              /\ UNCHANGED <<rpPendingAttachments,
                             rpNextFifoOrdinal, rpNextTicketId>>
      BY <1>1, ReplyOwnedPipelineItemProjection, SMTT(10)
         DEF ReplyPipelineTypeInvariant,
             AdmitReplyPipelineItem,
             ReplyPipelineItemFor,
             ReplyPipelineItemsFor
    <2>2. CASE ReplyPipelineItemFor(
                    owner, semantic, source).flushRequired
      <3>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
                 admitted ==
                   ReplyPipelineItemWithPhase(item, "Admitted")
             IN /\ rpItems' =
                       ReplyPipelineReplaceItem(item, admitted)
                /\ ReplyPipelineItemHasType(admitted)
        BY <1>1, <2>1, <2>2,
           ReplyPipelinePhaseUpdateHasType, SMTT(10)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineFlushAdmission,
               ReplyPipelinePhases,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3>2. \A checked \in rpItems':
               ReplyPipelineItemHasType(checked)
        BY <1>1, <3>1,
           ReplyPipelineReplacementMembership, SMTT(20)
           DEF ReplyPipelineTypeInvariant,
               ReplyPipelineReplaceItem
      <3> QED BY <1>1, <2>1, <3>2, SMTT(10)
           DEF ReplyPipelineTypeInvariant
    <2>3. CASE ~ReplyPipelineItemFor(
                     owner, semantic, source).flushRequired
      <3>1. rpItems' =
               rpItems \ {ReplyPipelineItemFor(
                            owner, semantic, source)}
        BY <1>1, <2>3, SMTT(5)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3>2. \A checked \in rpItems':
               ReplyPipelineItemHasType(checked)
        BY <1>1, <3>1, SMTT(10)
           DEF ReplyPipelineTypeInvariant
      <3> QED BY <1>1, <2>1, <3>2, SMTT(10)
           DEF ReplyPipelineTypeInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyAdmitPreservesPerIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => ReplyPipelinePerIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE ReplyPipelinePerIdentityInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ UNCHANGED rpPendingAttachments
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>2. ReplyPipelinePendingPerIdentityInvariant'
      BY <1>1, <2>1, SMTT(10)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelinePerIdentityInvariant,
             ReplyPipelinePendingPerIdentityInvariant
    <2>3. CASE ReplyPipelineItemFor(
                    owner, semantic, source).flushRequired
      <3>1. LET item ==
                   ReplyPipelineItemFor(owner, semantic, source)
                 admitted ==
                   ReplyPipelineItemWithPhase(item, "Admitted")
             IN /\ rpItems' =
                       ReplyPipelineReplaceItem(item, admitted)
                /\ admitted.owner = item.owner
                /\ admitted.semantic = item.semantic
                /\ admitted.source = item.source
        BY <1>1, <2>3,
           ReplyPipelinePhaseUpdateProjection, SMTT(10)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineFlushAdmission,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3>2. ReplyPipelineItemPerIdentityInvariant'
        BY <1>1, <2>1, <3>1,
           ReplyPipelineReplacementPreservesPerIdentity
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelinePerIdentityInvariant,
               ReplyPipelineItemPerIdentityInvariant,
               ReplyPipelineReplaceItem
      <3> QED BY <2>2, <3>2
           DEF ReplyPipelinePerIdentityInvariant
    <2>4. CASE ~ReplyPipelineItemFor(
                     owner, semantic, source).flushRequired
      <3>1. rpItems' =
               rpItems \ {ReplyPipelineItemFor(
                            owner, semantic, source)}
        BY <1>1, <2>4, SMTT(5)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3>2. ReplyPipelineItemPerIdentityInvariant'
        BY <1>1, <3>1, SMTT(15)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelinePerIdentityInvariant,
               ReplyPipelineItemPerIdentityInvariant
      <3> QED BY <2>2, <3>2
           DEF ReplyPipelinePerIdentityInvariant
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplyAdmitPreservesFifoOrdinalInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineFifoOrdinalInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE ReplyPipelineFifoOrdinalInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN /\ item \in rpItems
              /\ rpNextFifoOrdinal' = rpNextFifoOrdinal
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>2. CASE ReplyPipelineItemFor(
                    owner, semantic, source).flushRequired
      <3>1. LET item ==
                   ReplyPipelineItemFor(owner, semantic, source)
                 admitted ==
                   ReplyPipelineItemWithPhase(item, "Admitted")
             IN /\ rpItems' =
                       ReplyPipelineReplaceItem(item, admitted)
                /\ admitted.owner = item.owner
                /\ admitted.fifoOrdinal = item.fifoOrdinal
        BY <1>1, <2>2,
           ReplyPipelinePhaseUpdateProjection, SMTT(10)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineFlushAdmission,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3>1a. \A left, right \in rpItems':
                 /\ left.owner = right.owner
                 /\ left.fifoOrdinal = right.fifoOrdinal
                 => left = right
        BY <1>1, <2>1, <3>1,
           ReplyPipelineReplacementPreservesFifoIdentity
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineFifoOrdinalInvariant,
               ReplyPipelineReplaceItem
      <3>1b. \A checked \in rpItems':
                 checked.fifoOrdinal <
                   rpNextFifoOrdinal'[checked.owner]
        BY <1>1, <2>1, <3>1,
           ReplyPipelineReplacementPreservesFifoBound
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineFifoOrdinalInvariant,
               ReplyPipelineReplaceItem
      <3> QED BY <3>1a, <3>1b
           DEF ReplyPipelineFifoOrdinalInvariant
    <2>3. CASE ~ReplyPipelineItemFor(
                     owner, semantic, source).flushRequired
      <3>1. rpItems' =
               rpItems \ {ReplyPipelineItemFor(
                            owner, semantic, source)}
        BY <1>1, <2>3, SMTT(5)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3> QED BY <1>1, <2>1, <3>1, SMTT(20)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineFifoOrdinalInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyAdmitPreservesTicketIdentityInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineTicketIdentityInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE ReplyPipelineTicketIdentityInvariant'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN item \in rpItems
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>2. CASE ReplyPipelineItemFor(
                    owner, semantic, source).flushRequired
      <3>1. LET item ==
                   ReplyPipelineItemFor(owner, semantic, source)
                 admitted ==
                   ReplyPipelineItemWithPhase(item, "Admitted")
             IN /\ rpItems' =
                       ReplyPipelineReplaceItem(item, admitted)
                /\ admitted.owner = item.owner
                /\ admitted.ticketId = item.ticketId
        BY <1>1, <2>2,
           ReplyPipelinePhaseUpdateProjection, SMTT(10)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineFlushAdmission,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3> QED BY <1>1, <2>1, <3>1,
           ReplyPipelineReplacementPreservesSameTicketIdentity
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineTicketIdentityInvariant,
               ReplyPipelineReplaceItem
    <2>3. CASE ~ReplyPipelineItemFor(
                     owner, semantic, source).flushRequired
      <3>1. rpItems' =
               rpItems \ {ReplyPipelineItemFor(
                            owner, semantic, source)}
        BY <1>1, <2>3, SMTT(5)
           DEF AdmitReplyPipelineItem,
               ReplyPipelineItemFor,
               ReplyPipelineItemsFor
      <3> QED BY <1>1, <3>1, SMTT(15)
           DEF ReplyPipelineInductiveInvariant,
               ReplyPipelineOwnershipInvariant,
               ReplyPipelineTicketIdentityInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyPipelinePhaseUpdatePreservesCoreBinding ==
  \A item, phase:
    ReplyPipelineItemCoreBinding(item) =>
      ReplyPipelineItemCoreBinding(
        ReplyPipelineItemWithPhase(item, phase))
BY ReplyPipelinePhaseUpdateProjection, SMTT(15)
   DEF ReplyPipelineItemCoreBinding,
       ReplyPipelineItemMatchesAttempt

THEOREM ReplyPipelinePhaseUpdatePreservesRouteBinding ==
  \A item, phase:
    ReplyPipelineItemRouteBinding(
      ReplyPipelineItemWithPhase(item, phase)) <=>
        ReplyPipelineItemRouteBinding(item)
BY ReplyPipelinePhaseUpdateProjection, SMTT(15)
   DEF ReplyPipelineItemRouteBinding

THEOREM ReplyPipelinePhaseUpdatePreservesTicketValid ==
  \A item, phase:
    /\ ReplyPipelineTicketValid(item)
    /\ phase \in {"Ticketed", "Admitted", "Flushed"}
    => ReplyPipelineTicketValid(
         ReplyPipelineItemWithPhase(item, phase))
BY ReplyPipelinePhaseUpdateProjection, SMTT(20)
   DEF ReplyPipelineTicketValid,
       ReplyPipelinePayloadForItem

THEOREM ReplyPipelinePhaseUpdateRouteStutterPreservesCoreBinding ==
  \A item, phase:
    /\ ReplyPipelineItemCoreBinding(item)
    /\ UNCHANGED ReplyRouteVars
    => ReplyPipelineItemCoreBinding(
         ReplyPipelineItemWithPhase(item, phase))'
PROOF
  <1>1. ASSUME NEW item, NEW phase,
                ReplyPipelineItemCoreBinding(item),
                UNCHANGED ReplyRouteVars
         PROVE ReplyPipelineItemCoreBinding(
                 ReplyPipelineItemWithPhase(item, phase))'
    <2>1. ReplyPipelineItemCoreBinding(
             ReplyPipelineItemWithPhase(item, phase))
      BY <1>1,
         ReplyPipelinePhaseUpdatePreservesCoreBinding
    <2> QED BY <1>1, <2>1,
         ReplyRouteStutterPreservesItemCoreBinding
  <1> QED BY <1>1

THEOREM ReplyPipelinePhaseUpdateStutterPreservesRouteBinding ==
  \A item, phase:
    /\ ReplyPipelineItemRouteBinding(item)
    /\ UNCHANGED ReplyRouteVars
    /\ rpPendingAttachments' = rpPendingAttachments
    => ReplyPipelineItemRouteBinding(
         ReplyPipelineItemWithPhase(item, phase))'
PROOF
  <1>1. ASSUME NEW item, NEW phase,
                ReplyPipelineItemRouteBinding(item),
                UNCHANGED ReplyRouteVars,
                rpPendingAttachments' = rpPendingAttachments
         PROVE ReplyPipelineItemRouteBinding(
                 ReplyPipelineItemWithPhase(item, phase))'
    <2>1. ReplyPipelineItemRouteBinding(
             ReplyPipelineItemWithPhase(item, phase))
      BY <1>1,
         ReplyPipelinePhaseUpdatePreservesRouteBinding
    <2> QED BY <1>1, <2>1,
         ReplyRouteAndPendingStutterPreservesItemRouteBinding
  <1> QED BY <1>1

THEOREM ReplyPipelinePhaseUpdatePreservesTicketValidPrime ==
  \A item, phase:
    /\ ReplyPipelineTicketValid(item)
    /\ phase \in {"Ticketed", "Admitted", "Flushed"}
    /\ rpNextTicketId' = rpNextTicketId
    => ReplyPipelineTicketValid(
         ReplyPipelineItemWithPhase(item, phase))'
BY ReplyPipelinePhaseUpdatePreservesTicketValid,
   SMTT(15)
   DEF ReplyPipelineTicketValid

THEOREM ReplyAdmitFlushPreservesExistingItemBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A checked \in rpItems:
      LET selected ==
            ReplyPipelineItemFor(owner, semantic, source)
      IN /\ ReplyPipelineInductiveInvariant
         /\ AdmitReplyPipelineItem(owner, semantic, source)
         /\ selected.flushRequired
         /\ checked # selected
         => /\ ReplyPipelineItemCoreBinding(checked)'
            /\ ReplyPipelineItemRouteBinding(checked)'
            /\ ReplyPipelineItemPhaseBinding(checked)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW checked \in rpItems,
                LET selected ==
                      ReplyPipelineItemFor(owner, semantic, source)
                IN /\ ReplyPipelineInductiveInvariant
                   /\ AdmitReplyPipelineItem(
                        owner, semantic, source)
                   /\ selected.flushRequired
                   /\ checked # selected
         PROVE /\ ReplyPipelineItemCoreBinding(checked)'
               /\ ReplyPipelineItemRouteBinding(checked)'
               /\ ReplyPipelineItemPhaseBinding(checked)'
    <2>1. LET selected ==
                 ReplyPipelineItemFor(owner, semantic, source)
               admitted ==
                 ReplyPipelineItemWithPhase(selected, "Admitted")
           IN /\ rpItems' =
                     ReplyPipelineReplaceItem(selected, admitted)
              /\ UNCHANGED ReplyRouteVars
              /\ rpPendingAttachments' = rpPendingAttachments
              /\ rpNextTicketId' = rpNextTicketId
      BY <1>1, SMTT(10)
         DEF AdmitReplyPipelineItem,
             ReplyPipelineFlushAdmission,
             ReplyPipelineItemFor,
             ReplyPipelineItemsFor
    <2>2a. LET selected ==
                  ReplyPipelineItemFor(owner, semantic, source)
            IN selected \in rpItems
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
    <2>2b. LET selected ==
                  ReplyPipelineItemFor(owner, semantic, source)
                admitted ==
                  ReplyPipelineItemWithPhase(selected, "Admitted")
            IN /\ admitted.owner = selected.owner
              /\ admitted.source = selected.source
              /\ admitted.outputClass = selected.outputClass
              /\ admitted.fifoOrdinal = selected.fifoOrdinal
      BY ReplyPipelinePhaseUpdateProjection
    <2>2c. LET admitted ==
                  ReplyPipelineItemWithPhase(
                    ReplyPipelineItemFor(
                      owner, semantic, source), "Admitted")
            IN admitted.fifoOrdinal \in Nat
      BY <1>1, <2>2a, <2>2b, SMTT(15)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineItemHasType
    <2>3. /\ ReplyPipelineItemCoreBinding(checked)
           /\ ReplyPipelineItemRouteBinding(checked)
           /\ ReplyPipelineItemPhaseBinding(checked)
      BY <1>1
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant
    <2>4. ReplyPipelineItemCoreBinding(checked)'
      BY <2>1, <2>3,
         ReplyRouteStutterPreservesItemCoreBinding
    <2>5. ReplyPipelineItemRouteBinding(checked)'
      BY <2>1, <2>3,
         ReplyRouteAndPendingStutterPreservesItemRouteBinding
    <2>6. checked.phase = "Queued" =>
             ReplyPipelineItemPhaseBinding(checked)'
      <3>1. ASSUME checked.phase = "Queued"
             PROVE ReplyPipelineItemPhaseBinding(checked)'
        <4>1. ReplyPipelineQueuedItem(checked)'
          BY <2>3, <3>1, SMTT(10)
             DEF ReplyPipelineItemPhaseBinding,
                 ReplyPipelineQueuedItem
        <4> QED BY <4>1,
             ReplyQueuedItemEstablishesPhaseBindingPrime
      <3> QED BY <3>1
    <2>7. checked.phase # "Queued" =>
             ReplyPipelineItemPhaseBinding(checked)'
      <3>1. ASSUME checked.phase # "Queued"
             PROVE ReplyPipelineItemPhaseBinding(checked)'
        <4>1. /\ ReplyPipelineTicketValid(checked)
               /\ ReplyPipelineItemIsFifoHead(checked)
               /\ (checked.phase \in {"Admitted", "Flushed"}
                     => checked.flushRequired)
          BY <2>3, <3>1
             DEF ReplyPipelineItemPhaseBinding
        <4>2. ReplyPipelineTicketValid(checked)'
          BY <2>1, <4>1, SMTT(15)
             DEF ReplyPipelineTicketValid
        <4>3. ReplyPipelineItemIsFifoHead(checked)'
          BY <2>1, <2>2a, <2>2b, <2>2c, <4>1,
             ReplyPipelineReplacementPreservesFifoHead
        <4> QED BY <3>1, <4>1, <4>2, <4>3, SMTT(10)
             DEF ReplyPipelineItemPhaseBinding
      <3> QED BY <3>1
    <2>8. ReplyPipelineItemPhaseBinding(checked)'
      BY <2>6, <2>7, SMTT(5)
    <2> QED BY <2>4, <2>5, <2>8
  <1> QED BY <1>1

THEOREM ReplyAdmitFlushEstablishesSelectedItemBinding ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET selected ==
          ReplyPipelineItemFor(owner, semantic, source)
        admitted ==
          ReplyPipelineItemWithPhase(selected, "Admitted")
    IN /\ ReplyPipelineInductiveInvariant
       /\ AdmitReplyPipelineItem(owner, semantic, source)
       /\ selected.flushRequired
       => /\ admitted \in rpItems'
          /\ ReplyPipelineItemCoreBinding(admitted)'
          /\ ReplyPipelineItemRouteBinding(admitted)'
          /\ ReplyPipelineItemPhaseBinding(admitted)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                LET selected ==
                      ReplyPipelineItemFor(owner, semantic, source)
                    admitted ==
                      ReplyPipelineItemWithPhase(
                        selected, "Admitted")
                IN /\ ReplyPipelineInductiveInvariant
                   /\ AdmitReplyPipelineItem(
                        owner, semantic, source)
                   /\ selected.flushRequired
         PROVE LET selected ==
                     ReplyPipelineItemFor(owner, semantic, source)
                   admitted ==
                     ReplyPipelineItemWithPhase(
                       selected, "Admitted")
               IN /\ admitted \in rpItems'
                  /\ ReplyPipelineItemCoreBinding(admitted)'
                  /\ ReplyPipelineItemRouteBinding(admitted)'
                  /\ ReplyPipelineItemPhaseBinding(admitted)'
    <2> DEFINE SelectedItem ==
          ReplyPipelineItemFor(owner, semantic, source)
    <2> DEFINE AdmittedItem ==
          ReplyPipelineItemWithPhase(SelectedItem, "Admitted")
    <2>1a. SelectedItem \in rpItems
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant, SelectedItem
    <2>1b. ReplyPipelineItemHasType(SelectedItem)
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant, SelectedItem
    <2>1c. ReplyPipelineItemCoreBinding(SelectedItem)
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant, SelectedItem
    <2>1d. ReplyPipelineItemRouteBinding(SelectedItem)
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant, SelectedItem
    <2>1e. ReplyPipelineTicketValid(SelectedItem)
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant, SelectedItem
    <2>1f. ReplyPipelineItemIsFifoHead(SelectedItem)
      BY <1>1, ReplyAdmitSelectedItemFacts
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant, SelectedItem
    <2>2a. /\ AdmittedItem.phase = "Admitted"
            /\ AdmittedItem.flushRequired
            /\ AdmittedItem.owner = SelectedItem.owner
            /\ AdmittedItem.semantic = SelectedItem.semantic
            /\ AdmittedItem.source = SelectedItem.source
            /\ AdmittedItem.messageCursor = SelectedItem.messageCursor
            /\ AdmittedItem.chunkCursor = SelectedItem.chunkCursor
            /\ AdmittedItem.outputClass = SelectedItem.outputClass
            /\ AdmittedItem.fifoOrdinal = SelectedItem.fifoOrdinal
            /\ AdmittedItem.routeTenure = SelectedItem.routeTenure
            /\ AdmittedItem.ticketId = SelectedItem.ticketId
            /\ AdmittedItem.ticketTenure = SelectedItem.ticketTenure
            /\ AdmittedItem.ticketPayload = SelectedItem.ticketPayload
            /\ AdmittedItem.fifoOrdinal \in Nat
      BY <1>1, <2>1b,
         ReplyPipelinePhaseUpdateProjection, SMTT(15)
         DEF ReplyPipelineItemHasType,
             SelectedItem, AdmittedItem
    <2>2b. /\ rpItems' =
                   ReplyPipelineReplaceItem(
                     SelectedItem, AdmittedItem)
            /\ UNCHANGED ReplyRouteVars
            /\ rpPendingAttachments' = rpPendingAttachments
            /\ rpNextTicketId' = rpNextTicketId
      BY <1>1, SMTT(10)
         DEF AdmitReplyPipelineItem,
             ReplyPipelineFlushAdmission,
             ReplyPipelineItemFor,
             ReplyPipelineItemsFor,
             SelectedItem, AdmittedItem
    <2>3a. ReplyPipelineItemCoreBinding(AdmittedItem)
      BY <2>1c, <2>2a, SMTT(20)
         DEF ReplyPipelineItemCoreBinding,
             ReplyPipelineItemMatchesAttempt
    <2>3b. rrAttempts' = rrAttempts
      BY <2>2b, SMTT(5) DEF ReplyRouteVars
    <2>3c. ReplyAttemptsForSource(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)' =
             ReplyAttemptsForSource(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)
      BY <2>3b, SMTT(10)
         DEF ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3d. ReplyAttemptFor(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)' =
             ReplyAttemptFor(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)
      BY <2>3c DEF ReplyAttemptFor
    <2>3e. ReplyAttemptOwned(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)' <=>
             ReplyAttemptOwned(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)
      BY <2>3c DEF ReplyAttemptOwned
    <2>3. ReplyPipelineItemCoreBinding(AdmittedItem)'
      BY <2>3a, <2>3d, <2>3e, SMTT(15)
         DEF ReplyPipelineItemCoreBinding,
             ReplyPipelineItemMatchesAttempt
    <2>5a. ReplyPipelineItemRouteBinding(AdmittedItem)
      BY <2>1d, <2>2a, SMTT(15)
         DEF ReplyPipelineItemRouteBinding
    <2>5b. ReplyAttemptCurrent(
               ReplyAttemptFor(
                 AdmittedItem.owner,
                 AdmittedItem.semantic,
                 AdmittedItem.source))' <=>
             ReplyAttemptCurrent(
               ReplyAttemptFor(
                 AdmittedItem.owner,
                 AdmittedItem.semantic,
                 AdmittedItem.source))
      BY <2>2b, <2>3d, SMTT(15)
         DEF ReplyAttemptCurrent, ReplyRouteVars
    <2>5c. ReplyRouteRebindPending(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)' <=>
             ReplyRouteRebindPending(
               AdmittedItem.owner,
               AdmittedItem.semantic,
               AdmittedItem.source)
      BY <2>2b, SMTT(15)
         DEF ReplyRouteRebindPending,
             ReplyPendingAttachmentsFor
    <2>5. ReplyPipelineItemRouteBinding(AdmittedItem)'
      BY <2>5a, <2>5b, <2>5c, SMTT(10)
         DEF ReplyPipelineItemRouteBinding
    <2>6. AdmittedItem \in rpItems'
      BY <2>1a, <2>2b, SMTT(10)
         DEF ReplyPipelineReplaceItem
    <2>7. ReplyPipelineTicketValid(AdmittedItem)'
      BY <2>1e, <2>2a, <2>2b, SMTT(20)
         DEF ReplyPipelineTicketValid,
             ReplyPipelinePayloadForItem
    <2>8. ReplyPipelineItemIsFifoHead(AdmittedItem)'
      <3>1. ASSUME NEW other \in
                      ReplyPipelineItemsInLane(
                        AdmittedItem.owner,
                        AdmittedItem.source,
                        AdmittedItem.outputClass)'
             PROVE AdmittedItem.fifoOrdinal <= other.fifoOrdinal
        <4>1. /\ other \in rpItems'
               /\ other.owner = AdmittedItem.owner
               /\ other.source = AdmittedItem.source
               /\ other.outputClass = AdmittedItem.outputClass
          BY <3>1 DEF ReplyPipelineItemsInLane
        <4>2. \/ other \in rpItems
               \/ other = AdmittedItem
          BY <2>2b, <4>1, SMTT(10)
             DEF ReplyPipelineReplaceItem
        <4>3. CASE other \in rpItems
          <5>1. other \in
                   ReplyPipelineItemsInLane(
                     SelectedItem.owner,
                     SelectedItem.source,
                     SelectedItem.outputClass)
            BY <2>2a, <4>1, <4>3
               DEF ReplyPipelineItemsInLane
          <5> QED BY <2>1f, <2>2a, <5>1, SMTT(10)
               DEF ReplyPipelineItemIsFifoHead
        <4>4. CASE other = AdmittedItem
          <5> QED BY <4>4
        <4> QED BY <4>2, <4>3, <4>4
      <3> QED BY <3>1
           DEF ReplyPipelineItemIsFifoHead
    <2>9a. AdmittedItem.phase # "Queued"
      BY <2>2a, SMTT(5)
    <2>9b. AdmittedItem.flushRequired
      BY <2>2a
    <2>9. ReplyPipelineItemPhaseBinding(AdmittedItem)'
      BY <2>7, <2>8, <2>9a, <2>9b, SMTT(10)
         DEF ReplyPipelineItemPhaseBinding
    <2> QED BY <2>3, <2>5, <2>6, <2>9
         DEF SelectedItem, AdmittedItem
  <1> QED BY <1>1

THEOREM ReplyAdmitFlushPreservesItemBindingInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET selected ==
          ReplyPipelineItemFor(owner, semantic, source)
    IN /\ ReplyPipelineInductiveInvariant
       /\ AdmitReplyPipelineItem(owner, semantic, source)
       /\ selected.flushRequired
       => ReplyPipelineItemBindingInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                LET selected ==
                      ReplyPipelineItemFor(owner, semantic, source)
                IN /\ ReplyPipelineInductiveInvariant
                   /\ AdmitReplyPipelineItem(
                        owner, semantic, source)
                   /\ selected.flushRequired
         PROVE ReplyPipelineItemBindingInvariant'
    <2>1. LET selected ==
                 ReplyPipelineItemFor(owner, semantic, source)
               admitted ==
                 ReplyPipelineItemWithPhase(selected, "Admitted")
           IN /\ rpItems' =
                     ReplyPipelineReplaceItem(selected, admitted)
              /\ admitted \in rpItems'
              /\ ReplyPipelineItemCoreBinding(admitted)'
              /\ ReplyPipelineItemRouteBinding(admitted)'
              /\ ReplyPipelineItemPhaseBinding(admitted)'
      BY <1>1,
         ReplyAdmitFlushEstablishesSelectedItemBinding,
         SMTT(10)
         DEF AdmitReplyPipelineItem,
             ReplyPipelineFlushAdmission,
             ReplyPipelineItemFor,
             ReplyPipelineItemsFor
    <2>2. ASSUME NEW checked \in rpItems'
           PROVE /\ ReplyPipelineItemCoreBinding(checked)'
                 /\ ReplyPipelineItemRouteBinding(checked)'
                 /\ ReplyPipelineItemPhaseBinding(checked)'
      <3>1. LET selected ==
                   ReplyPipelineItemFor(owner, semantic, source)
                 admitted ==
                   ReplyPipelineItemWithPhase(selected, "Admitted")
             IN \/ checked \in rpItems \ {selected}
                \/ checked = admitted
        BY <2>1, <2>2, SMTT(10)
           DEF ReplyPipelineReplaceItem
      <3>2. CASE LET selected ==
                         ReplyPipelineItemFor(
                           owner, semantic, source)
                   IN checked \in rpItems \ {selected}
        BY <1>1, <3>2,
           ReplyAdmitFlushPreservesExistingItemBinding
      <3>3. CASE LET admitted ==
                         ReplyPipelineItemWithPhase(
                           ReplyPipelineItemFor(
                             owner, semantic, source), "Admitted")
                   IN checked = admitted
        BY <2>1, <3>3, SMTT(5)
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>2
         DEF ReplyPipelineItemBindingInvariant
  <1> QED BY <1>1

THEOREM ReplyAdmitPipelineItemPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ AdmitReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                AdmitReplyPipelineItem(owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyPipelineAdvancePreservesRouteInvariant, Isa
         DEF ReplyPipelineInductiveInvariant,
             AdmitReplyPipelineItem,
             ReplyPipelineItemFor, ReplyPipelineItemsFor,
             ReplyPipelineTypeInvariant
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             AdmitReplyPipelineItem,
             ReplyPipelineFlushAdmission,
             ReplyPipelineAdvanceAttempt,
             ReplyAttemptAfterService, ReplaceReplyAttempt,
             ReplyPipelineReplaceItem,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyReconnectPendingForSource,
             ReplyRouteRebindPending,
             ReplyPipelineHasUnresolvedWriter,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent, ReplyAttemptComplete
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplyFlushPipelineItemPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ FlushAdmittedReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                FlushAdmittedReplyPipelineItem(
                  owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStutterPreservesInductiveInvariant
         DEF ReplyPipelineInductiveInvariant,
             FlushAdmittedReplyPipelineItem, ReplyRouteVars
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             FlushAdmittedReplyPipelineItem,
             ReplyPipelineReplaceItem,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyReconnectPendingForSource,
             ReplyRouteRebindPending,
             ReplyPipelineHasUnresolvedWriter,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent, ReplyAttemptComplete
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplyClosePipelineItemPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ CloseAdmittedReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                CloseAdmittedReplyPipelineItem(
                  owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStutterPreservesInductiveInvariant
         DEF ReplyPipelineInductiveInvariant,
             CloseAdmittedReplyPipelineItem, ReplyRouteVars
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             CloseAdmittedReplyPipelineItem,
             ReplyPipelineItemWithoutTicket,
             ReplyPipelineReplaceItem,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyReconnectPendingForSource,
             ReplyRouteRebindPending,
             ReplyPipelineHasUnresolvedWriter,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent, ReplyAttemptComplete,
             NoReplyPipelineTicket, NoReplyTicketTenure
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplyApplyFlushedItemPreservesInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineInductiveInvariant
    /\ ApplyFlushedReplyPipelineItem(owner, semantic, source)
    => ReplyPipelineInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyPipelineInductiveInvariant,
                ApplyFlushedReplyPipelineItem(
                  owner, semantic, source)
         PROVE ReplyPipelineInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyPipelineAdvancePreservesRouteInvariant, Isa
         DEF ReplyPipelineInductiveInvariant,
             ApplyFlushedReplyPipelineItem,
             ReplyPipelineItemFor, ReplyPipelineItemsFor,
             ReplyPipelineTypeInvariant
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             ApplyFlushedReplyPipelineItem,
             ReplyPipelineFlushedApplication,
             ReplyPipelineAdvanceAttempt,
             ReplyAttemptAfterService, ReplaceReplyAttempt,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyReconnectPendingForSource,
             ReplyRouteRebindPending,
             ReplyPipelineHasUnresolvedWriter,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent, ReplyAttemptComplete
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplyPipelineTicketBindsCanonicalFifoHead ==
  ReplyPipelineSafetyInvariant =>
    \A item \in rpItems:
      item.phase # "Queued" =>
        /\ ReplyPipelineTicketValid(item)
        /\ item.ticketPayload = {ReplyPipelinePayloadForItem(item)}
        /\ ReplyPipelineItemIsFifoHead(item)
BY SMTT(120)
   DEF ReplyPipelineSafetyInvariant,
       ReplyPipelineOwnershipInvariant,
       ReplyPipelineTicketValid

THEOREM ReplyAcquireRequiresLiveCurrentCursor ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET item == ReplyPipelineItemFor(owner, semantic, source)
        attempt == ReplyAttemptFor(owner, semantic, source)
    IN AcquireReplyPipelineTicket(owner, semantic, source)
       => /\ rrSourceActive[owner][source]
          /\ attempt.connectionTenure =
               rrConnectionTenure[owner][source]
          /\ ReplyPipelineItemMatchesAttempt(item, attempt)
          /\ item.routeTenure = attempt.connectionTenure
          /\ ReplyPipelineQueuedItem(item)
          /\ ReplyPipelineItemIsFifoHead(item)
BY SMTT(10)
   DEF AcquireReplyPipelineTicket,
       ReplyPipelineLiveCurrentCursor,
       ReplyAttemptCurrent,
       ReplyPipelineItemFor,
       ReplyPipelineItemsFor

THEOREM ReplyTerminalActionsRequireExactCurrentTicket ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET item == ReplyPipelineItemFor(owner, semantic, source)
        attempt == ReplyAttemptFor(owner, semantic, source)
        terminalAction ==
          \/ AdmitReplyPipelineItem(owner, semantic, source)
          \/ FlushAdmittedReplyPipelineItem(
               owner, semantic, source)
          \/ CloseAdmittedReplyPipelineItem(
               owner, semantic, source)
          \/ ApplyFlushedReplyPipelineItem(
               owner, semantic, source)
    IN terminalAction
       => /\ rrSourceActive[owner][source]
          /\ attempt.connectionTenure =
               rrConnectionTenure[owner][source]
          /\ ReplyPipelineItemMatchesAttempt(item, attempt)
          /\ item.routeTenure = attempt.connectionTenure
          /\ ReplyPipelineTicketValid(item)
BY SMTT(10)
   DEF AdmitReplyPipelineItem,
       FlushAdmittedReplyPipelineItem,
       CloseAdmittedReplyPipelineItem,
       ApplyFlushedReplyPipelineItem,
       ReplyPipelineExactTicketAuthority,
       ReplyPipelineLiveCurrentCursor,
       ReplyAttemptCurrent,
       ReplyPipelineItemFor,
       ReplyPipelineItemsFor

(***************************************************************************
After one semantic reconnect attaches, sibling reconnect observations become
Later rebinds.  Their old-tenure attempt and queued item remain durable, but
the explicit current-route guard prevents a ticket until that Later
attachment is consumed.
***************************************************************************)
THEOREM ReplySiblingLaterCannotTicketBeforeRebind ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteRebindPending(owner, semantic, source)
    /\ ReplyAttemptOwned(owner, semantic, source)
    /\ ~ReplyAttemptCurrent(
          ReplyAttemptFor(owner, semantic, source))
    => ~AcquireReplyPipelineTicket(owner, semantic, source)
BY SMTT(60)
   DEF AcquireReplyPipelineTicket,
       ReplyPipelineItemFor, ReplyPipelineItemsFor

THEOREM ReplyFlushAdmissionDoesNotAdvanceCursor ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET item == ReplyPipelineItemFor(owner, semantic, source)
    IN /\ ReplyPipelineItemOwned(owner, semantic, source)
       /\ item.flushRequired
       /\ AdmitReplyPipelineItem(owner, semantic, source)
       => /\ item.phase = "Ticketed"
          /\ ReplyPipelineFlushAdmission(item)
BY SMTT(5)
   DEF AdmitReplyPipelineItem, ReplyPipelineItemFor,
       ReplyPipelineItemsFor, ReplyPipelineItemOwned

THEOREM ReplyFlushAdmissionLeavesRouteCursorUnchanged ==
  \A item:
    ReplyPipelineFlushAdmission(item) => rrAttempts' = rrAttempts
BY SMTT(5)
   DEF ReplyPipelineFlushAdmission, ReplyRouteVars

THEOREM ReplyPipelineAdvanceKernelStrictlyIncreasesRank ==
  \A item:
    LET before ==
          ReplyAttemptFor(item.owner, item.semantic, item.source)
    IN ReplyPipelineAdvanceAttempt(item) =>
         ReplyAttemptRank(ReplyAttemptAfterService(before)) >
           ReplyAttemptRank(before)
BY SMTT(5)
   DEF ReplyPipelineAdvanceAttempt,
       AdvanceCurrentReplyAttempt,
       ReplyAttemptServiceKernelValid

THEOREM ReplyFlushedApplyAdvancesExactlyOnce ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET item == ReplyPipelineItemFor(owner, semantic, source)
        before == ReplyAttemptFor(owner, semantic, source)
    IN /\ ReplyPipelineSafetyInvariant
       /\ ReplyPipelineItemOwned(owner, semantic, source)
       /\ item.phase = "Flushed"
       /\ ApplyFlushedReplyPipelineItem(owner, semantic, source)
       => /\ ReplyPipelineFlushedApplication(item)
          /\ ReplyAttemptRank(
               ReplyAttemptAfterService(before)) >
               ReplyAttemptRank(before)
          /\ ~ReplyPipelineItemOwned(owner, semantic, source)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                LET item ==
                      ReplyPipelineItemFor(owner, semantic, source)
                    before ==
                      ReplyAttemptFor(owner, semantic, source)
                IN /\ ReplyPipelineSafetyInvariant
                   /\ ReplyPipelineItemOwned(
                        owner, semantic, source)
                   /\ item.phase = "Flushed"
                   /\ ApplyFlushedReplyPipelineItem(
                        owner, semantic, source)
         PROVE LET item ==
                     ReplyPipelineItemFor(owner, semantic, source)
                   before ==
                     ReplyAttemptFor(owner, semantic, source)
               IN /\ ReplyPipelineFlushedApplication(item)
                  /\ ReplyAttemptRank(
                       ReplyAttemptAfterService(before)) >
                       ReplyAttemptRank(before)
                  /\ ~ReplyPipelineItemOwned(
                        owner, semantic, source)'
    <2>1. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
           IN ReplyPipelineFlushedApplication(item)
      BY <1>1, SMTT(5)
         DEF ApplyFlushedReplyPipelineItem,
             ReplyPipelineItemFor,
             ReplyPipelineItemsFor
    <2>2. LET item ==
                 ReplyPipelineItemFor(owner, semantic, source)
             before == ReplyAttemptFor(owner, semantic, source)
           IN ReplyAttemptRank(
                ReplyAttemptAfterService(before)) >
                ReplyAttemptRank(before)
      BY <1>1, <2>1,
         ReplyPipelineAdvanceKernelStrictlyIncreasesRank,
         SMTT(10)
         DEF ReplyPipelineFlushedApplication
    <2>3. ~ReplyPipelineItemOwned(owner, semantic, source)'
      BY <1>1, <2>1, SMTT(30)
         DEF ReplyPipelineFlushedApplication,
             ReplyPipelineItemOwned,
             ReplyPipelineItemsFor,
             ReplyPipelineItemFor
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyReconnectWaitsForOldWriterResolution ==
  ReplyPipelineSafetyInvariant =>
    \A owner \in ReplyOwners, source \in ReplySources:
      /\ ReplyReconnectPendingForSource(owner, source)
      /\ ReplyPipelineHasUnresolvedWriter(owner, source)
      => /\ rrSourceActive[owner][source]
         /\ ~ENABLED RetirePendingReconnectSource(owner, source)
         /\ \A item \in rpItems:
              (item.owner = owner /\ item.source = source) =>
                item.phase # "Ticketed"
PROOF
  <1>1. ASSUME ReplyPipelineSafetyInvariant
         PROVE \A owner \in ReplyOwners, source \in ReplySources:
           /\ ReplyReconnectPendingForSource(owner, source)
           /\ ReplyPipelineHasUnresolvedWriter(owner, source)
           => /\ rrSourceActive[owner][source]
              /\ ~ENABLED RetirePendingReconnectSource(owner, source)
              /\ \A item \in rpItems:
                   (item.owner = owner /\ item.source = source) =>
                     item.phase # "Ticketed"
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW source \in ReplySources,
                  ReplyReconnectPendingForSource(owner, source),
                  ReplyPipelineHasUnresolvedWriter(owner, source)
           PROVE /\ rrSourceActive[owner][source]
                 /\ ~ENABLED RetirePendingReconnectSource(owner, source)
                 /\ \A item \in rpItems:
                      (item.owner = owner /\ item.source = source) =>
                        item.phase # "Ticketed"
      <3>1. /\ rrSourceActive[owner][source]
             /\ \A item \in rpItems:
                  (item.owner = owner /\ item.source = source) =>
                    item.phase # "Ticketed"
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyPipelineSafetyInvariant,
               ReplyPipelineOwnershipInvariant
      <3>2. ~ENABLED RetirePendingReconnectSource(owner, source)
        BY <2>1, ExpandENABLED, SMTT(30)
           DEF RetirePendingReconnectSource, RetireReplySource,
               ReplyPipelineEverySourceAttemptHasRebind,
               ReplyPipelineHasUnresolvedWriter,
               ReplyReconnectPendingForSource,
               ReplyReconnectPending
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
Pipeline-specific route projection and temporal safety.  These obligations
are proved over the concrete pipeline action graph: they do not import the
route spec or alias its temporal theorem.
***************************************************************************)
THEOREM ReplyObservedDeliveryProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    ObserveAuthenticatedReplyDelivery(owner, semantic, source, kind)
      => ReplyPipelineRouteStep
BY SMTT(10)
   DEF ObserveAuthenticatedReplyDelivery,
       ReplyPipelineRouteStep, ReplyRouteVars

THEOREM ReplyRetirePendingProjectsRouteStep ==
  \A owner \in ReplyOwners, source \in ReplySources:
    RetirePendingReconnectSource(owner, source)
      => ReplyPipelineRouteStep
BY SMTT(10)
   DEF RetirePendingReconnectSource, ReplyPipelineRouteStep

THEOREM ReplyAttachmentRouteActionProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources, kind \in ReplyAttachmentKinds:
    ReplyAttachmentRouteAction(owner, semantic, source, kind)
      => ReplyPipelineRouteStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW kind \in ReplyAttachmentKinds,
                ReplyAttachmentRouteAction(
                  owner, semantic, source, kind)
         PROVE ReplyPipelineRouteStep
    <2>1. CASE kind = "New"
      BY <1>1, <2>1, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             ReplyPipelineRouteStep
    <2>2. CASE kind = "Exact"
      BY <1>1, <2>2, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             ReplyPipelineRouteStep
    <2>3. CASE kind = "Later"
      BY <1>1, <2>3, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             ReplyPipelineRouteStep
    <2>4. CASE kind = "Reconnect"
      BY <1>1, <2>4, SMTT(5)
         DEF ReplyAttachmentRouteAction,
             ReplyPipelineRouteStep
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4
         DEF ReplyAttachmentKinds
  <1> QED BY <1>1

THEOREM ReplyAttachPendingProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    AttachPendingReplyDelivery(owner, semantic, source)
      => ReplyPipelineRouteStep
BY ReplyAttachmentRouteActionProjectsRouteStep, SMTT(5)
   DEF AttachPendingReplyDelivery

THEOREM ReplyEnqueueProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    EnqueueCurrentReplyItem(owner, semantic, source)
      => ReplyPipelineRouteStep
BY SMTT(10)
   DEF EnqueueCurrentReplyItem, ReplyPipelineRouteStep,
       ReplyRouteVars

THEOREM ReplyAcquirePipelineTicketProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    AcquireReplyPipelineTicket(owner, semantic, source)
      => ReplyPipelineRouteStep
BY SMTT(10)
   DEF AcquireReplyPipelineTicket, ReplyPipelineRouteStep,
       ReplyRouteVars

THEOREM ReplyFlushAdmissionProjectsRouteStep ==
  \A item:
    ReplyPipelineFlushAdmission(item) => ReplyPipelineRouteStep
BY SMTT(10)
   DEF ReplyPipelineFlushAdmission,
       ReplyPipelineRouteStep, ReplyRouteVars

THEOREM ReplyAdvanceCurrentProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    AdvanceCurrentReplyAttempt(owner, semantic, source)
      => ReplyPipelineRouteStep
BY SMTT(5) DEF ReplyPipelineRouteStep

THEOREM ReplyPipelineAdvanceProjectsRouteStep ==
  \A item:
    ReplyPipelineAdvanceAttempt(item) => ReplyPipelineRouteStep
PROOF
  <1>1. ASSUME NEW item, ReplyPipelineAdvanceAttempt(item)
         PROVE ReplyPipelineRouteStep
    <2>1. /\ item.owner \in ReplyOwners
           /\ item.semantic \in ReplySemantics
           /\ item.source \in ReplySources
      BY <1>1, SMTT(5)
         DEF ReplyPipelineAdvanceAttempt,
             AdvanceCurrentReplyAttempt
    <2>2. AdvanceCurrentReplyAttempt(
             item.owner, item.semantic, item.source)
      BY <1>1 DEF ReplyPipelineAdvanceAttempt
    <2> QED BY <2>1, <2>2,
         ReplyAdvanceCurrentProjectsRouteStep
  <1> QED BY <1>1

THEOREM ReplyAdmitProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    AdmitReplyPipelineItem(owner, semantic, source)
      => ReplyPipelineRouteStep
BY ReplyFlushAdmissionProjectsRouteStep,
   ReplyPipelineAdvanceProjectsRouteStep,
   SMTT(10)
   DEF AdmitReplyPipelineItem

THEOREM ReplyFlushProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    FlushAdmittedReplyPipelineItem(owner, semantic, source)
      => ReplyPipelineRouteStep
BY SMTT(10)
   DEF FlushAdmittedReplyPipelineItem,
       ReplyPipelineRouteStep, ReplyRouteVars

THEOREM ReplyCloseProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    CloseAdmittedReplyPipelineItem(owner, semantic, source)
      => ReplyPipelineRouteStep
BY SMTT(10)
   DEF CloseAdmittedReplyPipelineItem,
       ReplyPipelineRouteStep, ReplyRouteVars

THEOREM ReplyApplyProjectsRouteStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ApplyFlushedReplyPipelineItem(owner, semantic, source)
      => ReplyPipelineRouteStep
BY ReplyPipelineAdvanceProjectsRouteStep, SMTT(10)
   DEF ApplyFlushedReplyPipelineItem,
       ReplyPipelineFlushedApplication

THEOREM ReplyPipelineNextProjectsRouteStep ==
  ReplyPipelineNext => ReplyPipelineRouteStep
BY ReplyObservedDeliveryProjectsRouteStep,
   ReplyRetirePendingProjectsRouteStep,
   ReplyAttachPendingProjectsRouteStep,
   ReplyEnqueueProjectsRouteStep,
   ReplyAcquirePipelineTicketProjectsRouteStep,
   ReplyAdmitProjectsRouteStep,
   ReplyFlushProjectsRouteStep,
   ReplyCloseProjectsRouteStep,
   ReplyApplyProjectsRouteStep
   DEF ReplyPipelineNext

THEOREM ReplyAttemptSelfReplayValid ==
  \A attempt \in ReplyAttemptSet:
    ReplyAttemptReplayValid(attempt, attempt)
BY SMTT(10)
   DEF ReplyAttemptReplayValid, ReplyAttemptCursor,
       ReplyAttemptSet, ReplyDeliveryOrdinals,
       ReplyConnectionTenures

THEOREM ReplyAttemptRouteUpdateReplayValid ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    /\ deliveryOrdinal > attempt.deliveryOrdinal
    /\ connectionTenure >= attempt.connectionTenure
    => ReplyAttemptReplayValid(
         attempt,
         ReplyAttemptWithRoute(
           attempt, deliveryOrdinal, connectionTenure))
BY SMTT(30)
   DEF ReplyAttemptReplayValid, ReplyAttemptWithRoute,
       ReplyAttemptHasNoTicket, ReplyAttemptCursor,
       ReplyAttemptSet, ReplyDeliveryOrdinals,
       ReplyConnectionTenures, NoReplyTicketTenure

THEOREM ReplyAttemptTicketClearReplayValid ==
  \A attempt \in ReplyAttemptSet:
    ReplyAttemptReplayValid(
      attempt, ReplyAttemptWithoutTicket(attempt))
BY SMTT(20)
   DEF ReplyAttemptReplayValid, ReplyAttemptWithoutTicket,
       ReplyAttemptCursor, ReplyAttemptSet,
       ReplyDeliveryOrdinals, ReplyConnectionTenures

THEOREM ReplyAttemptServiceReplayValid ==
  \A attempt \in ReplyAttemptSet:
    ~ReplyAttemptComplete(attempt) =>
      ReplyAttemptReplayValid(
        attempt, ReplyAttemptAfterService(attempt))
BY SMTT(30)
   DEF ReplyAttemptReplayValid, ReplyAttemptAfterService,
       ReplyAttemptComplete, ReplyAttemptCursor,
       ReplyAttemptSet, ReplyDeliveryOrdinals,
       ReplyConnectionTenures

THEOREM ReplyAttemptServicePreservesIdentity ==
  \A attempt \in ReplyAttemptSet:
    SameReplyAttemptIdentity(
      attempt, ReplyAttemptAfterService(attempt))
BY SMTT(10)
   DEF ReplyAttemptAfterService, SameReplyAttemptIdentity,
       ReplyAttemptSet, ReplyDeliveryOrdinals,
       ReplyConnectionTenures

THEOREM ReplyReconnectAttemptsEffect ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
        routed ==
          ReplyAttemptWithRoute(
            oldAttempt, rrNextDeliveryOrdinal[owner],
            rrConnectionTenure[owner][source] + 1)
    IN ReconnectReplySource(owner, semantic, source) =>
         rrAttempts' = ReplyAttemptsAfterReconnect(
                         oldAttempt, routed)
BY SMTT(5) DEF ReconnectReplySource

THEOREM ReplyReconnectRouteFacts ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
        deliveryOrdinal == rrNextDeliveryOrdinal[owner]
        connectionTenure == rrConnectionTenure[owner][source] + 1
    IN /\ ReplyRouteTypeInvariant
       /\ ReconnectReplySource(owner, semantic, source)
       => /\ oldAttempt \in ReplyAttemptSet
          /\ deliveryOrdinal \in ReplyDeliveryOrdinals
          /\ connectionTenure \in ReplyConnectionTenures
          /\ deliveryOrdinal > oldAttempt.deliveryOrdinal
          /\ connectionTenure > oldAttempt.connectionTenure
          /\ ReplySourceHasNoTickets(owner, source)'
          /\ rrConnectionTenure' =
               [rrConnectionTenure EXCEPT
                  ![owner][source] = connectionTenure]
BY SMTT(30)
   DEF ReplyRouteTypeInvariant, ReconnectReplySource,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource

THEOREM ReplyAdvanceAttemptsEffect ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
    IN AdvanceCurrentReplyAttempt(owner, semantic, source) =>
         rrAttempts' =
           ReplaceReplyAttempt(
             oldAttempt, ReplyAttemptAfterService(oldAttempt))
BY SMTT(5) DEF AdvanceCurrentReplyAttempt

THEOREM ReplyRouteStutterProvidesReplayAndIsolation ==
  /\ ReplyRouteSafetyInvariant
  /\ UNCHANGED ReplyRouteVars
  =>
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME ReplyRouteSafetyInvariant,
                UNCHANGED ReplyRouteVars
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      BY <1>1, ReplyAttemptSelfReplayValid, SMTT(10)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttemptReplayStep, ReplyRouteVars
    <2>2. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(10)
         DEF ReplySourceTenureInvalidationStep, ReplyRouteVars
    <2>3. ReplyAttemptSurvivalStep
      BY <1>1, SMTT(10)
         DEF ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity, ReplyRouteVars
    <2>4. ReplyOtherCursorIsolationStep
      BY <1>1, SMTT(10)
         DEF ReplyOtherCursorIsolationStep,
             SameReplyAttemptIdentity, ReplyRouteVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyObserveNewProvidesReplayAndIsolation ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ObserveNewReplySource(owner, semantic, source)
    =>
      /\ ReplyTenureAwareReplayStep
      /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                ObserveNewReplySource(owner, semantic, source)
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      BY <1>1, ReplyAttemptSelfReplayValid,
         FS_AddElement, SMTT(30)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ObserveNewReplySource, ReplyAttemptReplayStep
    <2>2. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(10)
         DEF ObserveNewReplySource,
             ReplySourceTenureInvalidationStep
    <2>3. ReplyAttemptSurvivalStep
      BY <1>1, FS_AddElement, SMTT(30)
         DEF ObserveNewReplySource, ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity
    <2>4. ReplyOtherCursorIsolationStep
      BY <1>1, FS_AddElement, SMTT(60)
         DEF ObserveNewReplySource,
             ReplyOtherCursorIsolationStep,
             SameReplyAttemptIdentity,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyObserveLaterProvidesReplayAndIsolation ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ObserveLaterReplyDelivery(owner, semantic, source)
    =>
      /\ ReplyTenureAwareReplayStep
      /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                ObserveLaterReplyDelivery(
                  owner, semantic, source)
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      BY <1>1, ReplyAttemptSelfReplayValid,
         ReplyAttemptRouteUpdateReplayValid, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ObserveLaterReplyDelivery,
             ReplyAttemptReplayStep,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplaceReplyAttempt
    <2>2. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(10)
         DEF ObserveLaterReplyDelivery,
             ReplySourceTenureInvalidationStep
    <2>3. ReplyAttemptSurvivalStep
      BY <1>1,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         SMTT(30)
         DEF ObserveLaterReplyDelivery,
             ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity,
             ReplyAttemptWithRoute, ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. ReplyOtherCursorIsolationStep
      BY <1>1,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         SMTT(60)
         DEF ObserveLaterReplyDelivery,
             ReplyOtherCursorIsolationStep,
             SameReplyAttemptIdentity,
             ReplyAttemptWithRoute, ReplaceReplyAttempt,
             ReplyAttemptCursor,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyRetireProvidesReplayAndIsolation ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ RetireReplySource(owner, source)
    =>
      /\ ReplyTenureAwareReplayStep
      /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                RetireReplySource(owner, source)
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      BY <1>1, ReplyAttemptSelfReplayValid,
         ReplyAttemptTicketClearReplayValid, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             RetireReplySource,
             ReplyAttemptReplayStep,
             ReplyAttemptWithoutTicket
    <2>2. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(10)
         DEF RetireReplySource,
             ReplySourceTenureInvalidationStep
    <2>3. ReplyAttemptSurvivalStep
      BY <1>1,
         ReplyRetireAttemptsEffect,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity,
             ReplyAttemptWithoutTicket
    <2>4. ReplyOtherCursorIsolationStep
      BY <1>1,
         ReplyRetireAttemptsEffect,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyOtherCursorIsolationStep,
             SameReplyAttemptIdentity,
             ReplyAttemptWithoutTicket, ReplyAttemptCursor
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyReconnectProvidesReplayAndIsolation ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ ReconnectReplySource(owner, semantic, source)
    =>
      /\ ReplyTenureAwareReplayStep
      /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                ReconnectReplySource(owner, semantic, source)
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      BY <1>1, ReplyAttemptSelfReplayValid,
         ReplyAttemptRouteUpdateReplayValid,
         ReplyAttemptTicketClearReplayValid,
         ReplyReconnectAttemptsEffect,
         ReplyReconnectRouteFacts, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttemptReplayStep,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. ReplySourceTenureInvalidationStep
      <3>1. ASSUME NEW checkedOwner \in ReplyOwners,
                    NEW checkedSource \in ReplySources,
                    rrConnectionTenure'[checkedOwner][checkedSource] >
                      rrConnectionTenure[checkedOwner][checkedSource]
             PROVE ReplySourceHasNoTickets(
                     checkedOwner, checkedSource)'
        <4>1. CASE /\ checkedOwner = owner
                    /\ checkedSource = source
          BY <1>1, <3>1, <4>1,
             ReplyReconnectRouteFacts, SMTT(10)
        <4>2. CASE checkedOwner # owner
          BY <1>1, <3>1, <4>2,
             ReplyReconnectRouteFacts,
             ReplyNestedTenureUpdatePreservesOtherOwner,
             SMTT(10)
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>3. CASE /\ checkedOwner = owner
                    /\ checkedSource # source
          BY <1>1, <3>1, <4>3,
             ReplyReconnectRouteFacts,
             ReplyNestedTenureUpdatePreservesOtherSource,
             SMTT(10)
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1
           DEF ReplySourceTenureInvalidationStep
    <2>3. ReplyAttemptSurvivalStep
      BY <1>1,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         ReplyReconnectAttemptsEffect,
         ReplyReconnectRouteFacts, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity,
             ReplyAttemptWithRoute,
             ReplyAttemptWithoutTicket,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. ReplyOtherCursorIsolationStep
      BY <1>1,
         ReplyAttemptRouteUpdatePreservesIdentityAndCursor,
         ReplyAttemptTicketClearPreservesIdentityAndCursor,
         ReplyReconnectAttemptsEffect,
         ReplyReconnectRouteFacts, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyOtherCursorIsolationStep,
             SameReplyAttemptIdentity,
             ReplyAttemptWithRoute,
             ReplyAttemptWithoutTicket,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptCursor,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyAdvanceProvidesReplayAndIsolation ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteSafetyInvariant
    /\ AdvanceCurrentReplyAttempt(owner, semantic, source)
    =>
      /\ ReplyTenureAwareReplayStep
      /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteSafetyInvariant,
                AdvanceCurrentReplyAttempt(owner, semantic, source)
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      BY <1>1, ReplyAttemptSelfReplayValid,
         ReplyAttemptServiceReplayValid, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             AdvanceCurrentReplyAttempt,
             ReplyAttemptReplayStep,
             ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>2. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(10)
         DEF AdvanceCurrentReplyAttempt,
             ReplySourceTenureInvalidationStep
    <2>3. ReplyAttemptSurvivalStep
      BY <1>1, ReplyAdvanceAttemptsEffect,
         ReplyAttemptServicePreservesIdentity, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity,
             ReplyAttemptAfterService, ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. ReplyOtherCursorIsolationStep
      BY <1>1, ReplyAdvanceAttemptsEffect,
         ReplyAttemptServicePreservesIdentity,
         ReplyRouteSafetyUniqueAttemptIdentity, SMTT(60)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyOtherCursorIsolationStep,
             SameReplyAttemptIdentity,
             ReplyAttemptAfterService, ReplaceReplyAttempt,
             ReplyAttemptCursor,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyPipelineRouteStepProvidesReplayAndIsolation ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplyPipelineRouteStep
  =>
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
BY ReplyRouteStutterProvidesReplayAndIsolation,
   ReplyObserveNewProvidesReplayAndIsolation,
   ReplyObserveLaterProvidesReplayAndIsolation,
   ReplyRetireProvidesReplayAndIsolation,
   ReplyReconnectProvidesReplayAndIsolation,
   ReplyAdvanceProvidesReplayAndIsolation,
   SMTT(10)
   DEF ReplyPipelineRouteStep, RetryExactReplySource

THEOREM ReplyPipelineNextProvidesReplayAndIsolation ==
  ReplyPipelineNext =>
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
BY ReplyPipelineNextProjectsRouteStep,
   ReplyPipelineRouteStepProvidesReplayAndIsolation,
   SMTT(5)
   DEF ReplyPipelineNext

THEOREM ReplyPipelineBracketProvidesReplayAndIsolation ==
  [ReplyPipelineNext]_ReplyPipelineVars =>
    /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
    /\ [ReplySourceIsolationStep]_ReplyRouteVars
PROOF
  <1>1. ASSUME [ReplyPipelineNext]_ReplyPipelineVars
         PROVE /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
               /\ [ReplySourceIsolationStep]_ReplyRouteVars
    <2>1. CASE ReplyPipelineNext
      BY <2>1, ReplyPipelineNextProvidesReplayAndIsolation, PTL
    <2>2. CASE UNCHANGED ReplyPipelineVars
      <3>1. UNCHANGED ReplyRouteVars
        BY <2>2
           DEF ReplyPipelineVars, ReplyPipelineLocalVars,
               ReplyRouteVars
      <3> QED BY <3>1, PTL
    <2> QED BY <1>1, <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM ReplyPipelineNextPreservesInvariant ==
  /\ ReplyPipelineInductiveInvariant
  /\ ReplyPipelineNext
  => ReplyPipelineInductiveInvariant'
BY ReplyObservedDeliveryPreservesInvariant,
   ReplyRetirePendingReconnectPreservesInvariant,
   ReplyAttachPendingDeliveryPreservesInvariant,
   ReplyEnqueueCurrentItemPreservesInvariant,
   ReplyAcquirePipelineTicketPreservesInvariant,
   ReplyAdmitPipelineItemPreservesInvariant,
   ReplyFlushPipelineItemPreservesInvariant,
   ReplyClosePipelineItemPreservesInvariant,
   ReplyApplyFlushedItemPreservesInvariant
   DEF ReplyPipelineNext

THEOREM ReplyPipelineSpecAlwaysSafe ==
  ReplyPipelineSpec => []ReplyPipelineSafetyInvariant
PROOF
  <1>1. ReplyPipelineInit => ReplyPipelineInductiveInvariant
    BY ReplyPipelineInitEstablishesInvariant
  <1>2. /\ ReplyPipelineInductiveInvariant
           /\ [ReplyPipelineNext]_ReplyPipelineVars
          => ReplyPipelineInductiveInvariant'
    BY ReplyPipelineNextPreservesInvariant, PTL
       DEF ReplyPipelineVars
  <1> QED BY <1>1, <1>2, PTL
       DEF ReplyPipelineSpec, ReplyPipelineInductiveInvariant

THEOREM ReplyPipelineSpecAlwaysReplayAndIsolation ==
  ReplyPipelineSpec =>
    /\ ReplyTenureAwareReplay
    /\ ReplySourceIsolation
PROOF
  <1>1. [] [ReplyPipelineNext]_ReplyPipelineVars =>
           /\ [] [ReplyTenureAwareReplayStep]_ReplyRouteVars
           /\ [] [ReplySourceIsolationStep]_ReplyRouteVars
    BY ReplyPipelineBracketProvidesReplayAndIsolation, PTL
  <1> QED BY <1>1, PTL
       DEF ReplyPipelineSpec, ReplyTenureAwareReplay,
           ReplySourceIsolation

(***************************************************************************
The remaining temporal leaves explicitly expose the environment boundary.
Observation itself is not assumed fair.  Once a delivery is in
`rpPendingAttachments`, local attachment/retirement is weakly fair.  A
responsive source additionally needs strong writer fairness for every
semantic item in every source/output-class lane.  This source/class-wide
premise excludes an older semantic item repeatedly taking Admit/Close/Queue
while starving its FIFO sibling; it is not granted to failed sources.
***************************************************************************)

THEOREM ReplyPendingAttachmentLeadsToConsumption ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyPipelineSpec =>
      ReplyPendingAttachmentEventuallyConsumed(
        owner, semantic, source)
BY PTL
   DEF ReplyPipelineSpec, ReplyPipelineFairness,
       ReplyPendingAttachmentEventuallyConsumed,
       ReplyPipelineNext, ReplyReconnectPendingForSource,
       ReplyPipelineHasUnresolvedWriter,
       AttachPendingReplyDelivery,
       RetirePendingReconnectSource,
       FlushAdmittedReplyPipelineItem,
       CloseAdmittedReplyPipelineItem,
       ApplyFlushedReplyPipelineItem,
       ReplyPipelineVars

THEOREM ReplyResponsivePipelineItemLeadsToAdvance ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyPipelineSpec
    /\ ReplyPipelineResponsiveSource(owner, source)
    /\ []ReplySourceRouteStable(owner, semantic, source)
    /\ []~ReplyReconnectPendingForSource(owner, source)
    => ReplyPipelineItemEventuallyAdvances(
         owner, semantic, source)
BY PTL
   DEF ReplyPipelineSpec, ReplyPipelineFairness,
       ReplyPipelineResponsiveSource,
       ReplyPipelineResponsiveOutputClass,
       ReplyPipelineItemEventuallyAdvances,
       EnqueueCurrentReplyItem, AcquireReplyPipelineTicket,
       AdmitReplyPipelineItem, FlushAdmittedReplyPipelineItem,
       FlushAdmittedReplyPipelineClassItem,
       ApplyFlushedReplyPipelineItem,
       ReplyPipelineItemIsFifoHead,
       ReplyPipelineVars

THEOREM ReplyRoutePipelineModelObligation ==
  ReplyPipelineSpec =>
    /\ []ReplyPipelineSafetyInvariant
    /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
         source \in ReplySources:
         ReplyPendingAttachmentEventuallyConsumed(
           owner, semantic, source)
BY ReplyPipelineSpecAlwaysSafe,
   ReplyPendingAttachmentLeadsToConsumption

=============================================================================
