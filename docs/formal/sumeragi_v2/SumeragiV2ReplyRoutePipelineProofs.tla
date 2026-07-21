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

THEOREM ReplyRetireAttemptsEffect ==
  \A owner \in ReplyOwners, source \in ReplySources:
    RetireReplySource(owner, source) =>
      rrAttempts' =
        {IF attempt.owner = owner /\ attempt.source = source
         THEN ReplyAttemptWithoutTicket(attempt)
         ELSE attempt: attempt \in rrAttempts}
BY SMTT(5) DEF RetireReplySource

ReplyAttemptAfterRetire(owner, source, attempt) ==
  IF attempt.owner = owner /\ attempt.source = source
  THEN ReplyAttemptWithoutTicket(attempt)
  ELSE attempt

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

THEOREM ReplyPipelineRouteRebindPreservesFifoOrdinal ==
  \A owner, semantic, source, connectionTenure:
    ReplyPipelineFifoOrdinalInvariant =>
      LET rebound ==
            ReplyPipelineItemsAfterRouteRebind(
              owner, semantic, source, connectionTenure)
      IN \A left, right \in rebound:
           /\ left.owner = right.owner
           /\ left.fifoOrdinal = right.fifoOrdinal
           => left = right
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
      BY <1>1, <2>1,
         ReplyPipelineRouteRebindPreservesItemPerIdentity,
         ReplyReconnectAttachPreservesPendingPerIdentity,
         SMTT(30)
         DEF ReplyPipelinePerIdentityInvariant
    <2>2. CASE selected.kind = "Later"
      BY <1>1, <2>2,
         ReplyPipelineRouteRebindPreservesItemPerIdentity,
         SMTT(30)
         DEF ReplyPipelinePerIdentityInvariant,
             ReplyPipelinePendingPerIdentityInvariant
    <2>3. CASE selected.kind = "New"
      BY <1>1, <2>3, SMTT(20)
         DEF ReplyPipelinePerIdentityInvariant,
             ReplyPipelinePendingPerIdentityInvariant
    <2>4. CASE selected.kind = "Exact"
      BY <1>1, <2>4, SMTT(20)
         DEF ReplyPipelinePerIdentityInvariant,
             ReplyPipelinePendingPerIdentityInvariant
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                  SMTT(10)
         DEF ReplyAttachmentSet, ReplyAttachmentKinds
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
                  ELSE rpItems
         PROVE ReplyPipelineFifoOrdinalInvariant'
    <2>1. CASE selected.kind \in {"Later", "Reconnect"}
      BY <1>1, <2>1,
         ReplyPipelineRouteRebindPreservesFifoOrdinal,
         SMTT(20)
    <2>2. CASE selected.kind \notin {"Later", "Reconnect"}
      BY <1>1, <2>2
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
    <2>2. CASE selected.kind \notin {"Later", "Reconnect"}
      BY <1>1, <2>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

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
      BY <1>1, SMTT(120)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineItemBindingInvariant,
             ReplyPipelineItemCoreBinding,
             ReplyPipelineItemRouteBinding,
             ReplyPipelineItemPhaseBinding,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor,
             ReplyPendingAfterReconnectAttach,
             ReplyPipelineItemsAfterRouteRebind,
             ReplyAttachmentRouteAction,
             ObserveNewReplySource, RetryExactReplySource,
             ObserveLaterReplyDelivery, ReconnectReplySource,
             ReplyAttemptWithRoute,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptWithoutTicket,
             ReplaceReplyAttempt,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyReconnectPending,
             ReplyRouteRebindPending,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent,
             ReplyAttemptComplete, NoReplyTicketTenure
    <2>8. ReplyPipelineReconnectBarrierInvariant'
      BY <1>1, SMTT(120)
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant,
             ReplyPipelineReconnectBarrierInvariant,
             ReplyPipelineReconnectNoTicketedInvariant,
             ReplyPipelineReconnectWriterActiveInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor,
             ReplyPendingAfterReconnectAttach,
             ReplyPipelineItemsAfterRouteRebind,
             ReplyAttachmentRouteAction,
             ReplyReconnectPending,
             ReplyReconnectPendingForSource,
             ReplyPipelineHasUnresolvedWriter,
             ObserveNewReplySource, RetryExactReplySource,
             ObserveLaterReplyDelivery, ReconnectReplySource,
             ReplyRouteVars
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                  <2>5, <2>6, <2>7, <2>8
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineOwnershipInvariant
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
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             EnqueueCurrentReplyItem, ReplyPipelineItem,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyReconnectPending,
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
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             AcquireReplyPipelineTicket,
             ReplyPipelineItemWithTicket,
             ReplyPipelineReplaceItem,
             ReplyPipelinePayloadForItem,
             ReplyPipelinePayload,
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
