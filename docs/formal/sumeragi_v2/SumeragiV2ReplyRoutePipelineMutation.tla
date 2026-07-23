---- MODULE SumeragiV2ReplyRoutePipelineMutation ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
Adversarial finite prefixes for the authenticated reply pipeline.

The fixed path attaches two semantic requests owned by one authenticated
source, preserves their source/class FIFO order, and begins a reconnect while
request A still owns an admitted sidecar writer.  Both A and B observe the
new physical connection.  A's successful old-tenure flush is applied before
retirement; the first reconnect attachment then turns B's sibling reconnect
into a Later rebind.  B cannot acquire a ticket in that intermediate state
and becomes serviceable only after its own attempt and queued item are rebound
to the source's new tenure.

Each mutation removes exactly one production boundary: fair pending attach,
source/class FIFO admission, cursor non-regression, cross-attempt cursor
isolation, exact tenure/payload ticket binding, reconnect retirement ordering,
or one-shot old-flush application.
***************************************************************************)

CONSTANT PipelineMutationMode

MutationOwners == {0}
MutationSourceOrder == <<0, 1>>
MutationSources ==
  {MutationSourceOrder[index]: index \in 1..Len(MutationSourceOrder)}
MutationSemantics == {"request-a", "request-b"}
MutationTargets == {"origin-a", "origin-b"}
MutationSemanticTarget(semantic) ==
  IF semantic = "request-a" THEN "origin-a" ELSE "origin-b"
MutationSourceCapacity == 2
MutationDeliveryOrdinalLimit == 12
MutationMessageCount == 2
MutationChunkCount == 1
MutationOutputClasses == {"reply-lane"}
MutationItemClass(semantic, messageCursor, chunkCursor) == "reply-lane"
MutationItemRequiresFlush(semantic, messageCursor, chunkCursor) ==
  semantic = "request-a"

VARIABLES
  attempts,
  payloads,
  nextDeliveryOrdinal,
  connectionTenure,
  sourceActive,
  nextServiceIndex,
  pendingAttachments,
  items,
  nextFifoOrdinal,
  nextTicketId,
  oldFlushAppliedTwice,
  phase

MutationPipeline ==
  INSTANCE SumeragiV2ReplyRoutePipeline WITH
    ReplyOwners <- MutationOwners,
    ReplySourceOrder <- MutationSourceOrder,
    ReplySemantics <- MutationSemantics,
    ReplyTargets <- MutationTargets,
    ReplySemanticTarget <- MutationSemanticTarget,
    ReplySourceCapacity <- MutationSourceCapacity,
    ReplyDeliveryOrdinalLimit <- MutationDeliveryOrdinalLimit,
    ReplyMessageCount <- MutationMessageCount,
    ReplyChunkCount <- MutationChunkCount,
    ReplyOutputClasses <- MutationOutputClasses,
    ReplyItemClass <- MutationItemClass,
    ReplyItemRequiresFlush <- MutationItemRequiresFlush,
    rrAttempts <- attempts,
    rrPayloads <- payloads,
    rrNextDeliveryOrdinal <- nextDeliveryOrdinal,
    rrConnectionTenure <- connectionTenure,
    rrSourceActive <- sourceActive,
    rrNextServiceIndex <- nextServiceIndex,
    rpPendingAttachments <- pendingAttachments,
    rpItems <- items,
    rpNextFifoOrdinal <- nextFifoOrdinal,
    rpNextTicketId <- nextTicketId

MutationPipelineVars == MutationPipeline!ReplyPipelineVars
MutationVars ==
  <<MutationPipelineVars, oldFlushAppliedTwice, phase>>

RequestA == "request-a"
RequestB == "request-b"
Source == 0

SourceAttempt(semantic) ==
  MutationPipeline!ReplyAttemptFor(0, semantic, Source)

SourceItem(semantic) ==
  MutationPipeline!ReplyPipelineItemFor(0, semantic, Source)

AdvancePhase(action, nextPhase) ==
  /\ action
  /\ UNCHANGED oldFlushAppliedTwice
  /\ phase' = nextPhase

BuggyTicketItem(item, ticketTenure, ticketPayload) ==
  [item EXCEPT
     !.phase = "Ticketed",
     !.ticketId = nextTicketId[0],
     !.ticketTenure = ticketTenure,
     !.ticketPayload = ticketPayload]

(***************************************************************************
Ticket request B even though request A has the smaller stable FIFO ordinal in
the same authenticated source/output-class lane.
***************************************************************************)
BuggyBypassSourceClassFifo ==
  LET item == SourceItem(RequestB)
      ticketed ==
        BuggyTicketItem(
          item, item.routeTenure,
          {MutationPipeline!ReplyPipelinePayloadForItem(item)})
  IN /\ phase = 6
     /\ SourceItem(RequestA).fifoOrdinal < item.fifoOrdinal
     /\ items' =
          MutationPipeline!ReplyPipelineReplaceItem(item, ticketed)
     /\ nextTicketId' = [nextTicketId EXCEPT ![0] = @ + 1]
     /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    pendingAttachments, nextFifoOrdinal,
                    oldFlushAppliedTwice>>
     /\ phase' = 30

(***************************************************************************
Mint one actor ticket with both the wrong source tenure and request B's
canonical payload while the item being authorized belongs to request A.
***************************************************************************)
BuggyReuseWrongTenureAndPayloadTicket ==
  LET item == SourceItem(RequestA)
      wrongPayload ==
        MutationPipeline!ReplyPipelinePayload(
          RequestB, item.messageCursor, item.chunkCursor)
      ticketed ==
        BuggyTicketItem(
          item, item.routeTenure + 1, {wrongPayload})
  IN /\ phase = 6
     /\ item.routeTenure + 1
          \in MutationPipeline!ReplyConnectionTenures
     /\ items' =
          MutationPipeline!ReplyPipelineReplaceItem(item, ticketed)
     /\ nextTicketId' = [nextTicketId EXCEPT ![0] = @ + 1]
     /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    pendingAttachments, nextFifoOrdinal,
                    oldFlushAppliedTwice>>
     /\ phase' = 31

(***************************************************************************
Retire the source directly while the old-tenure sidecar writer is admitted
and both reconnect attachments are pending.  The production action is
disabled until the writer flushes or closes.
***************************************************************************)
BuggyRetireBeforeOldWriterResolution ==
  /\ phase = 10
  /\ MutationPipeline!ReplyPipelineHasUnresolvedWriter(0, Source)
  /\ MutationPipeline!ReplyReconnectPendingForSource(0, Source)
  /\ MutationPipeline!RetireReplySource(0, Source)
  /\ UNCHANGED <<pendingAttachments, items,
                 nextFifoOrdinal, nextTicketId,
                 oldFlushAppliedTwice>>
  /\ phase' = 32

(***************************************************************************
Reuse the already consumed old-writer flush receipt after request A advanced
once and its pipeline item was removed.  This simulates the forbidden second
application without manufacturing a new exact item or ticket.
***************************************************************************)
BuggyApplyOldFlushTwice ==
  LET current == SourceAttempt(RequestA)
      advanced == MutationPipeline!ReplyAttemptAfterService(current)
  IN /\ phase = 12
     /\ current.messageCursor = 1
     /\ ~MutationPipeline!ReplyPipelineItemOwned(
           0, RequestA, Source)
     /\ attempts' =
          MutationPipeline!ReplaceReplyAttempt(current, advanced)
     /\ oldFlushAppliedTwice' = TRUE
     /\ UNCHANGED <<payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    pendingAttachments, items,
                    nextFifoOrdinal, nextTicketId>>
     /\ phase' = 33

(***************************************************************************
Regress the applied request A cursor while retaining every other route and
pipeline field.  No production action has this transition.
***************************************************************************)
BuggyRegressAppliedCursor ==
  LET current == SourceAttempt(RequestA)
      regressed == [current EXCEPT !.messageCursor = 0]
  IN /\ phase = 12
     /\ current.messageCursor = 1
     /\ attempts' =
          MutationPipeline!ReplaceReplyAttempt(current, regressed)
     /\ UNCHANGED <<payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    pendingAttachments, items,
                    nextFifoOrdinal, nextTicketId,
                    oldFlushAppliedTwice>>
     /\ phase' = 34

(***************************************************************************
Advance both semantic attempts in one atomic terminal transition.  Each
individual cursor is non-regressing, so tenure-aware replay still holds, but
changing request A no longer leaves request B's independently owned cursor
unchanged.  This is the source-isolation mutation.
***************************************************************************)
BuggyAdvanceTwoAttemptsAtOnce ==
  LET attemptA == SourceAttempt(RequestA)
      attemptB == SourceAttempt(RequestB)
      itemA == SourceItem(RequestA)
      advancedA == MutationPipeline!ReplyAttemptAfterService(attemptA)
      advancedB == MutationPipeline!ReplyAttemptAfterService(attemptB)
  IN /\ phase = 11
     /\ ~MutationPipeline!ReplyAttemptComplete(attemptA)
     /\ ~MutationPipeline!ReplyAttemptComplete(attemptB)
     /\ MutationPipeline!ReplyPipelineItemOwned(
           0, RequestA, Source)
     /\ itemA.phase = "Flushed"
     /\ attempts' =
          (attempts \ {attemptA, attemptB})
            \cup {advancedA, advancedB}
     /\ items' = items \ {itemA}
     /\ UNCHANGED <<payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    pendingAttachments,
                    nextFifoOrdinal, nextTicketId,
                    oldFlushAppliedTwice>>
     /\ phase' = 35

(***************************************************************************
Bypass reconnect-observation readiness after request A has attached the new
source tenure but request B still owns its old-tenure queued item and pending
Later rebind.  A second reconnect observation would discard B's sole rebind;
the production guard keeps this transition disabled until B attaches.
***************************************************************************)
BuggyObserveReconnectWithoutReadiness ==
  LET attachment ==
        MutationPipeline!ReplyAttachment(
          0, RequestA, Source, "Reconnect")
  IN /\ phase = 14
     /\ ~MutationPipeline!ReplyPipelineReconnectObservationReady(
           0, Source)
     /\ sourceActive[0][Source]
     /\ ~MutationPipeline!ReplyPendingAttachmentOwned(
           0, RequestA, Source)
     /\ MutationPipeline!ReplyAttemptOwned(
           0, RequestA, Source)
     /\ connectionTenure[0][Source]
          < MutationDeliveryOrdinalLimit
     /\ nextDeliveryOrdinal[0]
          + MutationPipeline!ReplyOwnerOrdinalReservations(0)
          <= MutationDeliveryOrdinalLimit
     /\ pendingAttachments' =
          MutationPipeline!ReplyPendingAfterReconnectObservation(
            0, Source, attachment)
     /\ items' =
          MutationPipeline!ReplyPipelineItemsAfterReconnectObservation(
            0, Source)
     /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    nextFifoOrdinal, nextTicketId,
                    oldFlushAppliedTwice>>
     /\ phase' = 36

PipelineMutationInit ==
  /\ MutationPipeline!ReplyPipelineInit
  /\ oldFlushAppliedTwice = FALSE
  /\ phase = 0

PipelineMutationNext ==
  \/ /\ phase = 0
     /\ AdvancePhase(
          MutationPipeline!ObserveAuthenticatedReplyDelivery(
            0, RequestA, Source, "New"), 1)
  \/ /\ phase = 1
     /\ AdvancePhase(
          MutationPipeline!AttachPendingReplyDelivery(
            0, RequestA, Source), 2)
  \/ /\ phase = 2
     /\ AdvancePhase(
          MutationPipeline!ObserveAuthenticatedReplyDelivery(
            0, RequestB, Source, "New"), 3)
  \/ /\ phase = 3
     /\ AdvancePhase(
          MutationPipeline!AttachPendingReplyDelivery(
            0, RequestB, Source), 4)
  \/ /\ phase = 4
     /\ AdvancePhase(
          MutationPipeline!EnqueueCurrentReplyItem(
            0, RequestA, Source), 5)
  \/ /\ phase = 5
     /\ AdvancePhase(
          MutationPipeline!EnqueueCurrentReplyItem(
            0, RequestB, Source), 6)
  \/ /\ phase = 6
     /\ PipelineMutationMode \notin
          {"FifoBypass", "WrongTicketReuse"}
     /\ AdvancePhase(
          MutationPipeline!AcquireReplyPipelineTicket(
            0, RequestA, Source), 7)
  \/ /\ PipelineMutationMode = "FifoBypass"
     /\ BuggyBypassSourceClassFifo
  \/ /\ PipelineMutationMode = "WrongTicketReuse"
     /\ BuggyReuseWrongTenureAndPayloadTicket
  \/ /\ phase = 7
     /\ AdvancePhase(
          MutationPipeline!AdmitReplyPipelineItem(
            0, RequestA, Source), 8)
  \/ /\ phase = 8
     /\ AdvancePhase(
          MutationPipeline!ObserveAuthenticatedReplyDelivery(
            0, RequestA, Source, "Reconnect"), 9)
  \/ /\ phase = 9
     /\ AdvancePhase(
          MutationPipeline!ObserveAuthenticatedReplyDelivery(
            0, RequestB, Source, "Reconnect"), 10)
  \/ /\ phase = 10
     /\ PipelineMutationMode # "PrematureRetire"
     /\ AdvancePhase(
          MutationPipeline!FlushAdmittedReplyPipelineItem(
            0, RequestA, Source), 11)
  \/ /\ PipelineMutationMode = "PrematureRetire"
     /\ BuggyRetireBeforeOldWriterResolution
  \/ /\ phase = 11
     /\ PipelineMutationMode # "CrossAttemptIsolation"
     /\ AdvancePhase(
          MutationPipeline!ApplyFlushedReplyPipelineItem(
            0, RequestA, Source), 12)
  \/ /\ PipelineMutationMode = "CrossAttemptIsolation"
     /\ BuggyAdvanceTwoAttemptsAtOnce
  \/ /\ phase = 12
     /\ PipelineMutationMode \notin
          {"CursorRegression", "OldFlushDoubleApply"}
     /\ AdvancePhase(
          MutationPipeline!RetirePendingReconnectSource(
            0, Source), 13)
  \/ /\ PipelineMutationMode = "CursorRegression"
     /\ BuggyRegressAppliedCursor
  \/ /\ PipelineMutationMode = "OldFlushDoubleApply"
     /\ BuggyApplyOldFlushTwice
  \/ /\ phase = 13
     /\ AdvancePhase(
          MutationPipeline!AttachPendingReplyDelivery(
            0, RequestA, Source), 14)
  \/ /\ phase = 14
     /\ PipelineMutationMode # "ReconnectObservationNotReady"
     /\ AdvancePhase(
          MutationPipeline!AttachPendingReplyDelivery(
            0, RequestB, Source), 15)
  \/ /\ PipelineMutationMode = "ReconnectObservationNotReady"
     /\ BuggyObserveReconnectWithoutReadiness
  \/ /\ phase = 15
     /\ AdvancePhase(
          MutationPipeline!AcquireReplyPipelineTicket(
            0, RequestB, Source), 16)
  \/ /\ phase = 16
     /\ AdvancePhase(
          MutationPipeline!AdmitReplyPipelineItem(
            0, RequestB, Source), 17)

(***************************************************************************
The unfair model records one authenticated delivery and then exposes only a
stutter.  The pending attachment remains continuously attachable, so the
leads-to property below must fail without the production weak-fairness term.
***************************************************************************)
UnfairPendingAttachNext ==
  /\ phase = 0
  /\ AdvancePhase(
       MutationPipeline!ObserveAuthenticatedReplyDelivery(
         0, RequestA, Source, "New"), 1)

UnfairPendingAttachSpec ==
  PipelineMutationInit /\ [][UnfairPendingAttachNext]_MutationVars

PendingAttachmentEventuallyConsumed ==
  MutationPipeline!ReplyPendingAttachmentOwned(
    0, RequestA, Source)
    ~> ~MutationPipeline!ReplyPendingAttachmentOwned(
         0, RequestA, Source)

(***************************************************************************
Two semantics share one authenticated source/output-class lane.  Request A is
the older flush-required item.  A weakly fair writer may repeatedly admit and
close A, satisfying the Admit/(Flush-or-Close) fairness terms while starving
request B forever.  The fixed spec adds the production source/class-wide
strong flush fairness, which eventually applies A and releases B's FIFO head.
The production ticket ordinal is logically unbounded.  `ClassWriterCloseA`
alpha-renames the destroyed, never-admitted ticket back to one so TLC obtains
a finite quotient for the retry loop; no live capability or state predicate
observes the retired ordinal.
***************************************************************************)
ClassWriterAcquireA ==
  /\ phase = 6
  /\ AdvancePhase(
       MutationPipeline!AcquireReplyPipelineTicket(
         0, RequestA, Source), 7)

ClassWriterAdmitA ==
  /\ phase = 7
  /\ AdvancePhase(
       MutationPipeline!AdmitReplyPipelineItem(
         0, RequestA, Source), 8)

ClassWriterFlushA ==
  /\ phase = 8
  /\ AdvancePhase(
       MutationPipeline!FlushAdmittedReplyPipelineItem(
         0, RequestA, Source), 9)

ClassWriterCloseA ==
  LET item == SourceItem(RequestA)
      attempt == SourceAttempt(RequestA)
  IN /\ phase = 8
     /\ MutationPipeline!ReplyPipelineItemOwned(
           0, RequestA, Source)
     /\ item.phase = "Admitted"
     /\ item.flushRequired
     /\ MutationPipeline!ReplyPipelineTicketValid(item)
     /\ MutationPipeline!ReplyAttemptOwned(
           0, RequestA, Source)
     /\ MutationPipeline!ReplyAttemptCurrent(attempt)
     /\ MutationPipeline!ReplyPipelineItemMatchesAttempt(
           item, attempt)
     /\ item.routeTenure = attempt.connectionTenure
     /\ items' =
          MutationPipeline!ReplyPipelineReplaceItem(
            item,
            MutationPipeline!ReplyPipelineItemWithoutTicket(item))
     /\ nextTicketId' = [nextTicketId EXCEPT ![0] = 1]
     /\ UNCHANGED <<attempts, payloads, nextDeliveryOrdinal,
                    connectionTenure, sourceActive, nextServiceIndex,
                    pendingAttachments, nextFifoOrdinal,
                    oldFlushAppliedTwice>>
     /\ phase' = 6

ClassWriterApplyA ==
  /\ phase = 9
  /\ AdvancePhase(
       MutationPipeline!ApplyFlushedReplyPipelineItem(
         0, RequestA, Source), 10)

ClassWriterAcquireB ==
  /\ phase = 10
  /\ AdvancePhase(
       MutationPipeline!AcquireReplyPipelineTicket(
         0, RequestB, Source), 11)

ClassWriterAdmitB ==
  /\ phase = 11
  /\ AdvancePhase(
       MutationPipeline!AdmitReplyPipelineItem(
         0, RequestB, Source), 12)

ClassWriterNext ==
  \/ /\ phase \in 0..5
     /\ PipelineMutationNext
  \/ ClassWriterAcquireA
  \/ ClassWriterAdmitA
  \/ ClassWriterFlushA
  \/ ClassWriterCloseA
  \/ ClassWriterApplyA
  \/ ClassWriterAcquireB
  \/ ClassWriterAdmitB

ClassWriterWeakFairness ==
  /\ WF_MutationVars(ClassWriterAcquireA)
  /\ WF_MutationVars(ClassWriterAdmitA)
  /\ WF_MutationVars(ClassWriterFlushA \/ ClassWriterCloseA)
  /\ WF_MutationVars(ClassWriterApplyA)
  /\ WF_MutationVars(ClassWriterAcquireB)
  /\ WF_MutationVars(ClassWriterAdmitB)

ClassWriterWeakSpec ==
  PipelineMutationInit
    /\ [][ClassWriterNext]_MutationVars
    /\ ClassWriterWeakFairness

ClassWriterResponsiveSpec ==
  ClassWriterWeakSpec
    /\ MutationPipeline!ReplyPipelineResponsiveOutputClass(
         0, Source, "reply-lane")

ClassWriterSiblingAtCursor(messageCursor) ==
  IF MutationPipeline!ReplyAttemptOwned(0, RequestB, Source)
  THEN SourceAttempt(RequestB).messageCursor = messageCursor
  ELSE FALSE

ClassWriterSiblingEventuallyAdvances ==
  (phase = 6 /\ ClassWriterSiblingAtCursor(0))
    ~> ClassWriterSiblingAtCursor(1)

RequestACursorNeverRegressesAfterApply ==
  phase < 12
    \/ SourceAttempt(RequestA).messageCursor >= 1

SiblingLaterRebindBlocksOldTenureTicket ==
  phase # 14
    \/ /\ MutationPipeline!ReplyRouteRebindPending(
             0, RequestB, Source)
       /\ ~MutationPipeline!ReplyAttemptCurrent(
             SourceAttempt(RequestB))
       /\ SourceItem(RequestB).routeTenure
            # connectionTenure[0][Source]
       /\ ~ENABLED MutationPipeline!AcquireReplyPipelineTicket(
             0, RequestB, Source)

OldFlushAppliedAtMostOnce == ~oldFlushAppliedTwice

PipelineMutationSafety ==
  /\ MutationPipeline!ReplyPipelineSafetyInvariant
  /\ RequestACursorNeverRegressesAfterApply
  /\ SiblingLaterRebindBlocksOldTenureTicket
  /\ OldFlushAppliedAtMostOnce

PipelineTenureAwareReplay ==
  MutationPipeline!ReplyTenureAwareReplay

PipelineSourceIsolation ==
  MutationPipeline!ReplySourceIsolation

=============================================================================
