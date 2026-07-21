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

THEOREM ReplyPipelineInitEstablishesInvariant ==
  ReplyPipelineInit => ReplyPipelineInductiveInvariant
PROOF
  <1>1. ReplyPipelineInit => ReplyRouteInductiveInvariant
    BY ReplyRouteInitEstablishesInductiveInvariant
       DEF ReplyPipelineInit
  <1>2. ReplyPipelineInit =>
           /\ ReplyPipelineConfiguration
           /\ ReplyPipelineTypeInvariant
           /\ ReplyPipelineOwnershipInvariant
    BY FS_EmptySet, Isa
       DEF ReplyPipelineInit, ReplyPipelineTypeInvariant,
           ReplyPipelineOwnershipInvariant,
           ReplyPendingAttachmentsFor, ReplyPipelineItemsFor,
           ReplyPipelineHasUnresolvedWriter,
           ReplyReconnectPendingForSource,
           ReplyPipelineConfiguration, ReplyPipelineOrdinalLimit,
           NoReplyPipelineTicket, NoReplyTicketTenure
  <1> QED BY <1>1, <1>2
       DEF ReplyPipelineInductiveInvariant

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
      BY <1>1, SMTT(10)
         DEF ObserveAuthenticatedReplyDelivery,
             ReplyPipelineInductiveInvariant, ReplyRouteVars
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             ObserveAuthenticatedReplyDelivery,
             ReplyPendingAfterReconnectObservation,
             ReplyPipelineItemsAfterReconnectObservation,
             ReplyPipelineItemWithoutTicket,
             ReplyPendingAttachmentsFor,
             ReplyPendingAttachmentOwned,
             ReplyReconnectPending,
             ReplyReconnectPendingForSource,
             ReplyRouteRebindPending,
             ReplyOwnerOrdinalReservations,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineHasUnresolvedWriter,
             ReplyPipelineItemMatchesAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
  <1> QED BY <1>1

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
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             RetirePendingReconnectSource,
             ReplyAttemptWithoutTicket,
             ReplyPipelineEverySourceAttemptHasRebind,
             ReplyPipelineHasUnresolvedWriter,
             ReplyReconnectPendingForSource,
             ReplyReconnectPending,
             ReplyPendingAttachmentsFor,
             ReplyRouteRebindPending,
             ReplyPipelineItemsFor,
             ReplyPipelineQueuedItem,
             ReplyPipelineTicketValid,
             ReplyPipelineItemIsFifoHead,
             ReplyPipelineItemsInLane,
             ReplyPipelineItemMatchesAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptCurrent, ReplyRouteVars
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
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
         ObserveNewReplySourcePreservesInductiveInvariant,
         RetryExactReplySourcePreservesInductiveInvariant,
         ObserveLaterReplyDeliveryPreservesInductiveInvariant,
         ReconnectReplySourcePreservesInductiveInvariant, Isa
         DEF ReplyPipelineInductiveInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor
    <2>2. /\ ReplyPipelineConfiguration'
           /\ ReplyPipelineTypeInvariant'
           /\ ReplyPipelineOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyPipelineInductiveInvariant,
             ReplyPipelineTypeInvariant,
             ReplyPipelineOwnershipInvariant,
             AttachPendingReplyDelivery,
             ReplyPendingAttachmentFor,
             ReplyPendingAttachmentsFor,
             ReplyPendingAttachmentOwned,
             ReplyPendingAfterReconnectAttach,
             ReplyPipelineItemsAfterRouteRebind,
             ObserveNewReplySource, RetryExactReplySource,
             ObserveLaterReplyDelivery, ReconnectReplySource,
             ReplyAttempt, ReplyAttemptWithRoute,
             ReplyAttemptsAfterReconnect,
             ReplyAttemptWithoutTicket,
             ReplaceReplyAttempt,
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
             ReplyAttemptCurrent,
             ReplyAttemptComplete, NoReplyTicketTenure
    <2> QED BY <2>1, <2>2
         DEF ReplyPipelineInductiveInvariant
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
      BY <1>1, SMTT(10)
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
      BY <1>1, SMTT(10)
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
  \A item \in ReplyPipelineItemSet:
    /\ ReplyRouteInductiveInvariant
    /\ ~ReplyAttemptComplete(
          ReplyAttemptFor(item.owner, item.semantic, item.source))
    /\ ReplyPipelineAdvanceAttempt(item)
    => ReplyRouteInductiveInvariant'
BY AdvanceCurrentReplyAttemptPreservesInductiveInvariant, Isa
   DEF ReplyPipelineAdvanceAttempt,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource

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
      BY <1>1, SMTT(10)
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
      BY <1>1, SMTT(10)
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
BY SMTT(60)
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
BY SMTT(60)
   DEF AcquireReplyPipelineTicket,
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
BY SMTT(90)
   DEF AdmitReplyPipelineItem,
       FlushAdmittedReplyPipelineItem,
       CloseAdmittedReplyPipelineItem,
       ApplyFlushedReplyPipelineItem,
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
       => /\ rrAttempts' = rrAttempts
          /\ item.phase = "Ticketed"
          /\ \E admitted \in rpItems':
               /\ ReplyPipelineItemMatchesAttempt(
                    admitted,
                    ReplyAttemptFor(owner, semantic, source))
               /\ admitted.phase = "Admitted"
BY SMTT(90)
   DEF AdmitReplyPipelineItem, ReplyPipelineItemFor,
       ReplyPipelineItemsFor, ReplyPipelineItemOwned,
       ReplyPipelineReplaceItem,
       ReplyPipelineItemMatchesAttempt

THEOREM ReplyFlushedApplyAdvancesExactlyOnce ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    LET item == ReplyPipelineItemFor(owner, semantic, source)
        before == ReplyAttemptFor(owner, semantic, source)
    IN /\ ReplyPipelineItemOwned(owner, semantic, source)
       /\ item.phase = "Flushed"
       /\ ApplyFlushedReplyPipelineItem(owner, semantic, source)
       => /\ ReplyAttemptRank(
                  ReplyAttemptFor(owner, semantic, source)')
                > ReplyAttemptRank(before)
          /\ ~ReplyPipelineItemOwned(owner, semantic, source)'
BY SMTT(120)
   DEF ApplyFlushedReplyPipelineItem,
       ReplyPipelineAdvanceAttempt,
       ReplyAttemptAfterService, ReplyAttemptRank,
       ReplaceReplyAttempt,
       ReplyPipelineItemFor, ReplyPipelineItemsFor,
       ReplyPipelineItemOwned, ReplyPipelineItemMatchesAttempt,
       ReplyAttemptFor, ReplyAttemptsForSource,
       ReplyAttemptsFor

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
BY SMTT(60)
   DEF ReplyPipelineSafetyInvariant,
       ReplyPipelineOwnershipInvariant,
       RetirePendingReconnectSource,
       ReplyPipelineHasUnresolvedWriter

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
