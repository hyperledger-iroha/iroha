---- MODULE SumeragiV2ReplyWriterDeadlineProofs ----
EXTENDS SumeragiV2ReplyWriterDeadline, TLAPS

(***************************************************************************
Deductive boundary for exact-reply writer termination.

The safety induction checks the fixed-deadline identity, immutable
writer-observation origin, distinct timeout outcome, adaptive-attempt
accounting, exact receipt-attempt identity, topology exclusion, and exact
accepting connection isolation. The local temporal proof uses only weak
fairness of actor dispatch, the monotone timer, expiry, and receipt polling; it
does not assume fairness of `PublishPeerWriterFlush`.

Responsive cursor liveness is deliberately separate.  A conditional theorem
isolates eventual ready-receipt publication, while the responsive theorem
derives that publication from weakly fair reconnect/dispatch and strongly fair
admission/publication.  This abstraction states qualitative temporal
termination; it does not establish a Rust refinement or a wall-clock SLA.
***************************************************************************)

THEOREM ReplyWriterDeadlineInitEstablishesInvariant ==
  Init => ReplyWriterDeadlineInvariant
BY SMTT(30)
   DEF Init, ReplyWriterDeadlineInvariant,
       ReplyWriterDeadlineTypeInvariant,
       DeadlineAcquiredAtFirstDispatchInvariant,
       TopologyHasNoExactDeadlineInvariant,
       AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
       LifecycleShapeInvariant,
       FlushOutcomeInvariant,
       ExactConnectionIsolationInvariant,
       ReplyWriterDeadlineConfiguration, Kinds, Phases, Outcomes,
       Connections, ExactActive, ScaledDeadline

THEOREM FirstDispatchAcquiresOneFixedDeadline ==
  FirstExactActorDispatch =>
    /\ deadlineSet'
    /\ deadlineBudget' = ScaledDeadline(timeoutAttempt)
    /\ deadlineOrigin' = 0
    /\ timeoutAttempt' = timeoutAttempt
BY DEF FirstExactActorDispatch

THEOREM FullQueueRetryPreservesFixedDeadline ==
  RetryFullPeerWriterQueue =>
    /\ deadlineSet' = deadlineSet
    /\ deadlineBudget' = deadlineBudget
    /\ deadlineOrigin' = deadlineOrigin
    /\ deadlineDue' = deadlineDue
BY DEF RetryFullPeerWriterQueue

THEOREM PublishedWriterReceiptBindsTimeoutAttempt ==
  PublishPeerWriterFlush =>
    ackTimeoutAttempt' = timeoutAttempt
BY DEF PublishPeerWriterFlush

THEOREM PollRequiresMatchingReceiptTimeoutAttempt ==
  PollPeerWriterFlush =>
    ackTimeoutAttempt = timeoutAttempt
BY DEF PollPeerWriterFlush, ExactFlushReady

THEOREM ReadyReceiptWinsBeforeTimeout ==
  /\ deadlineDue
  /\ ackReady
  /\ Next
  => ~ExpireExactDeadline
BY SMTT(10)
   DEF Next, ExpireExactDeadline, ExactDeadlineDue

THEOREM ReadyReceiptDisablesRouteRetirement ==
  ExactFlushReady =>
    /\ ~ExpireExactDeadline
    /\ ~ClosePeerWriter
    /\ ~RetireOldExactRoute
BY SMTT(10)
   DEF ExactFlushReady, ExpireExactDeadline, ExactDeadlineDue,
       ClosePeerWriter, RetireOldExactRoute

THEOREM TerminalFenceReadyReceiptWinsEveryDestructiveExit ==
  TerminalFenceReadyWinsEveryDestructiveExitAction
BY ReadyReceiptDisablesRouteRetirement
   DEF TerminalFenceReadyWinsEveryDestructiveExitAction

THEOREM ReadyReceiptSurvivesConnectionReplacement ==
  /\ ExactFlushReady
  /\ InstallReplacementBeforeTerminal
  => ExactFlushReady'
BY SMTT(10)
   DEF ExactFlushReady, ExactActive,
       InstallReplacementBeforeTerminal

THEOREM WriterFlushObservationComesOnlyFromPublish ==
  WriterFlushObservationOriginAction
BY SMTT(30)
   DEF WriterFlushObservationOriginAction, Next,
       AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute, InstallReplacementBeforeTerminal,
       ReconnectExactReply, FinishTopologyOutput

THEOREM WriterFlushObservationIsNeverErased ==
  WriterFlushObservationMonotonicAction
BY SMTT(30)
   DEF WriterFlushObservationMonotonicAction, Next,
       AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute, InstallReplacementBeforeTerminal,
       ReconnectExactReply, FinishTopologyOutput

THEOREM TimeoutPublishesDistinctTerminalOutcome ==
  ExpireExactDeadline =>
    /\ outcome' = "TimedOut"
    /\ cursor' = cursor
    /\ ~writerFlushObserved'
    /\ ~ackPublished'
    /\ timeoutAttempt' = SaturatingIncrement(timeoutAttempt)
    /\ timedOutCount' = SaturatingIncrement(timedOutCount)
BY DEF ExpireExactDeadline

THEOREM ClosedAndReconnectPreserveAdaptiveAttempt ==
  /\ (ClosePeerWriter \/ RetireOldExactRoute \/ ReconnectExactReply)
  => /\ timeoutAttempt' = timeoutAttempt
     /\ timedOutCount' = timedOutCount
BY SMTT(10)
   DEF ClosePeerWriter, RetireOldExactRoute, ReconnectExactReply

THEOREM ExpiryCannotTerminateReplacementConnection ==
  /\ ReplyWriterDeadlineInvariant
  /\ protectedReplacement = "ReplacementConnection"
  /\ ExpireExactDeadline
  => /\ currentConnection' = "ReplacementConnection"
     /\ protectedReplacement' = "ReplacementConnection"
BY SMTT(10)
   DEF ReplyWriterDeadlineInvariant,
       ExactConnectionIsolationInvariant,
       ExpireExactDeadline, ExactDeadlineDue, ExactActive

(***************************************************************************
The fixed `Next` relation has fifteen actions. The induction below enumerates
all fifteen explicitly; adding the receipt-bound timeout-attempt dimension
strengthens their assignments and guards without introducing a hidden branch.
***************************************************************************)
THEOREM ReplyWriterDeadlineNextPreservesInvariant ==
  /\ ReplyWriterDeadlineInvariant
  /\ Next
  => ReplyWriterDeadlineInvariant'
PROOF
  <1>1. CASE AdmitExactReply
    BY <1>1, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, AdmitExactReply,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>2. CASE AdmitTopologyOutput
    BY <1>2, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, AdmitTopologyOutput,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>3. CASE FirstExactActorDispatch
    BY <1>3, SMTT(60)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, FirstExactActorDispatch,
           ExactUndispatched, ExactActive,
           Kinds, Phases, Outcomes, Connections
  <1>4. CASE FirstTopologyActorDispatch
    BY <1>4, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, FirstTopologyActorDispatch,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>5. CASE RetryFullPeerWriterQueue
    BY <1>5, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, RetryFullPeerWriterQueue,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>6. CASE AdmitPeerWriter
    BY <1>6, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, AdmitPeerWriter,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>7. CASE PublishPeerWriterFlush
    BY <1>7, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, PublishPeerWriterFlush,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>8. CASE PollPeerWriterFlush
    BY <1>8, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, PollPeerWriterFlush,
           ExactFlushReady, ExactActive, ScaledDeadline,
           Kinds, Phases, Outcomes, Connections
  <1>9. CASE ReachExactDeadline
    BY <1>9, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, ReachExactDeadline,
           ExactWaitingForDeadline, ExactActive, ScaledDeadline,
           Kinds, Phases, Outcomes, Connections
  <1>10. CASE ExpireExactDeadline
    BY <1>10, SMTT(60)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, ExpireExactDeadline,
           ExactDeadlineDue, ExactActive, SaturatingIncrement,
           ScaledDeadline, Kinds, Phases, Outcomes, Connections
  <1>11. CASE ClosePeerWriter
    BY <1>11, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, ClosePeerWriter,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>12. CASE RetireOldExactRoute
    BY <1>12, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, RetireOldExactRoute,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>13. CASE InstallReplacementBeforeTerminal
    BY <1>13, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration,
           InstallReplacementBeforeTerminal, ExactActive,
           ScaledDeadline, Kinds, Phases, Outcomes, Connections
  <1>14. CASE ReconnectExactReply
    BY <1>14, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, ReconnectExactReply,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1>15. CASE FinishTopologyOutput
    BY <1>15, SMTT(30)
       DEF ReplyWriterDeadlineInvariant,
           ReplyWriterDeadlineTypeInvariant,
           DeadlineAcquiredAtFirstDispatchInvariant,
           TopologyHasNoExactDeadlineInvariant,
           AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
           LifecycleShapeInvariant,
           FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
           ReplyWriterDeadlineConfiguration, FinishTopologyOutput,
           ExactActive, ScaledDeadline, Kinds, Phases, Outcomes,
           Connections
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
               <1>8, <1>9, <1>10, <1>11, <1>12, <1>13, <1>14,
               <1>15
         DEF Next

THEOREM ReplyWriterDeadlineStutterPreservesInvariant ==
  /\ ReplyWriterDeadlineInvariant
  /\ UNCHANGED replyWriterDeadlineVars
  => ReplyWriterDeadlineInvariant'
BY SMTT(30)
   DEF replyWriterDeadlineVars, ReplyWriterDeadlineInvariant,
       ReplyWriterDeadlineTypeInvariant,
       DeadlineAcquiredAtFirstDispatchInvariant,
       TopologyHasNoExactDeadlineInvariant,
       AdaptiveAttemptInvariant, FlushAttemptIdentityInvariant,
       LifecycleShapeInvariant,
       FlushOutcomeInvariant, ExactConnectionIsolationInvariant,
       ReplyWriterDeadlineConfiguration, ExactActive, ScaledDeadline,
       Kinds, Phases, Outcomes, Connections

THEOREM ReplyWriterDeadlineBracketPreservesInvariant ==
  /\ ReplyWriterDeadlineInvariant
  /\ [Next]_replyWriterDeadlineVars
  => ReplyWriterDeadlineInvariant'
PROOF
  <1>1. CASE Next
    BY <1>1, ReplyWriterDeadlineNextPreservesInvariant
  <1>2. CASE UNCHANGED replyWriterDeadlineVars
    BY <1>2, ReplyWriterDeadlineStutterPreservesInvariant
  <1> QED BY <1>1, <1>2

THEOREM ReplyWriterDeadlineSpecAlwaysInvariant ==
  ReplyWriterDeadlineSpec => []ReplyWriterDeadlineInvariant
PROOF
  <1>1. Init => ReplyWriterDeadlineInvariant
    BY ReplyWriterDeadlineInitEstablishesInvariant
  <1>2. /\ ReplyWriterDeadlineInvariant
           /\ [Next]_replyWriterDeadlineVars
          => ReplyWriterDeadlineInvariant'
    BY ReplyWriterDeadlineBracketPreservesInvariant
  <1> QED BY <1>1, <1>2, PTL DEF ReplyWriterDeadlineSpec

THEOREM ReachableTopologyNeverAcquiresExactDeadline ==
  ReplyWriterDeadlineSpec =>
    [](kind = "Topology" =>
        /\ ~deadlineSet
        /\ deadlineBudget = 0
        /\ deadlineOrigin = 0
        /\ ~deadlineDue)
BY ReplyWriterDeadlineSpecAlwaysInvariant, PTL
   DEF ReplyWriterDeadlineInvariant,
       TopologyHasNoExactDeadlineInvariant

LocalTerminationRank1 ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactFlushReady
LocalTerminationRank2 ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactDeadlineDue
LocalTerminationRank3 ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactWaitingForDeadline
LocalTerminationRank4 ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactUndispatched

LocalTerminationRank1Exit == ResponsiveCursorAdvanced
LocalTerminationRank2Exit == ExactTerminal \/ LocalTerminationRank1
LocalTerminationRank3Exit ==
  ExactTerminal \/ LocalTerminationRank1 \/ LocalTerminationRank2
LocalTerminationRank4Exit ==
  ExactTerminal
    \/ LocalTerminationRank1
    \/ LocalTerminationRank2
    \/ LocalTerminationRank3

THEOREM LocalRank1IsNotOrphaned ==
  /\ LocalTerminationRank1
  /\ [Next]_replyWriterDeadlineVars
  => LocalTerminationRank1' \/ LocalTerminationRank1Exit'
PROOF
  <1>1. ASSUME LocalTerminationRank1,
                [Next]_replyWriterDeadlineVars
         PROVE LocalTerminationRank1' \/ LocalTerminationRank1Exit'
    <2>1. ReplyWriterDeadlineInvariant'
      BY <1>1, ReplyWriterDeadlineBracketPreservesInvariant
         DEF LocalTerminationRank1
    <2>2. ExactFlushReady' \/ ResponsiveCursorAdvanced'
      <3>1. CASE PollPeerWriterFlush
        BY <1>1, <3>1, SMTT(10)
           DEF PollPeerWriterFlush, ExactFlushReady,
               ResponsiveCursorAdvanced, ExactActive
      <3>2. CASE InstallReplacementBeforeTerminal
        BY <1>1, <3>2, SMTT(10)
           DEF InstallReplacementBeforeTerminal,
               LocalTerminationRank1, ExactFlushReady, ExactActive
      <3>3. CASE UNCHANGED replyWriterDeadlineVars
        BY <1>1, <3>3, SMTT(10)
           DEF LocalTerminationRank1, ExactFlushReady,
               ResponsiveCursorAdvanced, ExactActive,
               replyWriterDeadlineVars
      <3>4. CASE /\ ~PollPeerWriterFlush
                    /\ ~InstallReplacementBeforeTerminal
                    /\ ~UNCHANGED replyWriterDeadlineVars
        BY <1>1, <3>4, SMTT(60)
           DEF LocalTerminationRank1, ExactUndispatched,
               ExactWaitingForDeadline, ExactDeadlineDue,
               ExactFlushReady, ExactActive,
               Next, AdmitExactReply, AdmitTopologyOutput,
               FirstExactActorDispatch, FirstTopologyActorDispatch,
               RetryFullPeerWriterQueue, AdmitPeerWriter,
               PublishPeerWriterFlush, PollPeerWriterFlush,
               ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
               RetireOldExactRoute,
               InstallReplacementBeforeTerminal, ReconnectExactReply,
               FinishTopologyOutput, replyWriterDeadlineVars,
               ReplyWriterDeadlineInvariant,
               DeadlineAcquiredAtFirstDispatchInvariant,
               LifecycleShapeInvariant, FlushOutcomeInvariant
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1, <2>2
         DEF LocalTerminationRank1, LocalTerminationRank1Exit,
             ExactFlushReady, ResponsiveCursorAdvanced
  <1> QED BY <1>1

THEOREM LocalRank2IsNotOrphaned ==
  /\ LocalTerminationRank2
  /\ [Next]_replyWriterDeadlineVars
  => LocalTerminationRank2' \/ LocalTerminationRank2Exit'
BY ReplyWriterDeadlineNextPreservesInvariant,
   ReplyWriterDeadlineStutterPreservesInvariant, SMTT(60)
   DEF LocalTerminationRank2, LocalTerminationRank2Exit,
       LocalTerminationRank1, ExactDeadlineDue, ExactFlushReady,
       ExactTerminal, ExactActive,
       Next, AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute,
       InstallReplacementBeforeTerminal, ReconnectExactReply,
       FinishTopologyOutput, replyWriterDeadlineVars

THEOREM LocalRank3IsNotOrphaned ==
  /\ LocalTerminationRank3
  /\ [Next]_replyWriterDeadlineVars
  => LocalTerminationRank3' \/ LocalTerminationRank3Exit'
BY ReplyWriterDeadlineNextPreservesInvariant,
   ReplyWriterDeadlineStutterPreservesInvariant, SMTT(60)
   DEF LocalTerminationRank3, LocalTerminationRank3Exit,
       LocalTerminationRank1, LocalTerminationRank2,
       ExactWaitingForDeadline, ExactDeadlineDue, ExactFlushReady,
       ExactTerminal, ExactActive,
       Next, AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute,
       InstallReplacementBeforeTerminal, ReconnectExactReply,
       FinishTopologyOutput, replyWriterDeadlineVars

THEOREM LocalRank4IsNotOrphaned ==
  /\ LocalTerminationRank4
  /\ [Next]_replyWriterDeadlineVars
  => LocalTerminationRank4' \/ LocalTerminationRank4Exit'
PROOF
  <1>1. ASSUME LocalTerminationRank4,
                [Next]_replyWriterDeadlineVars
         PROVE LocalTerminationRank4' \/ LocalTerminationRank4Exit'
    <2>1. ReplyWriterDeadlineInvariant'
      BY <1>1, ReplyWriterDeadlineBracketPreservesInvariant
         DEF LocalTerminationRank4
    <2>2. ExactUndispatched' \/ ExactWaitingForDeadline'
      <3>1. CASE FirstExactActorDispatch
        BY <1>1, <3>1, SMTT(10)
           DEF LocalTerminationRank4, FirstExactActorDispatch,
               ExactUndispatched, ExactWaitingForDeadline, ExactActive,
               ReplyWriterDeadlineInvariant,
               DeadlineAcquiredAtFirstDispatchInvariant,
               LifecycleShapeInvariant, FlushOutcomeInvariant
      <3>2. CASE InstallReplacementBeforeTerminal
        BY <1>1, <3>2, SMTT(10)
           DEF LocalTerminationRank4,
               InstallReplacementBeforeTerminal,
               ExactUndispatched, ExactActive
      <3>3. CASE UNCHANGED replyWriterDeadlineVars
        BY <1>1, <3>3, SMTT(10)
           DEF LocalTerminationRank4, ExactUndispatched,
               ExactActive, replyWriterDeadlineVars
      <3>4. CASE /\ ~FirstExactActorDispatch
                    /\ ~InstallReplacementBeforeTerminal
                    /\ ~UNCHANGED replyWriterDeadlineVars
        BY <1>1, <3>4, SMTT(60)
           DEF LocalTerminationRank4,
               ExactUndispatched, ExactWaitingForDeadline,
               ExactDeadlineDue, ExactFlushReady, ExactActive,
               Next, AdmitExactReply, AdmitTopologyOutput,
               FirstExactActorDispatch, FirstTopologyActorDispatch,
               RetryFullPeerWriterQueue, AdmitPeerWriter,
               PublishPeerWriterFlush, PollPeerWriterFlush,
               ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
               RetireOldExactRoute,
               InstallReplacementBeforeTerminal, ReconnectExactReply,
               FinishTopologyOutput, replyWriterDeadlineVars,
               ReplyWriterDeadlineInvariant,
               DeadlineAcquiredAtFirstDispatchInvariant,
               LifecycleShapeInvariant, FlushOutcomeInvariant
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2> QED BY <2>1, <2>2
         DEF LocalTerminationRank4, LocalTerminationRank4Exit,
             LocalTerminationRank1, LocalTerminationRank2,
             LocalTerminationRank3, ExactUndispatched,
             ExactWaitingForDeadline, ExactDeadlineDue, ExactFlushReady,
             ExactTerminal, ExactActive
  <1> QED BY <1>1

THEOREM LocalRank1FairActionEnabled ==
  LocalTerminationRank1 =>
    ENABLED <<PollPeerWriterFlush>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF LocalTerminationRank1, PollPeerWriterFlush,
       ExactFlushReady, ExactActive, replyWriterDeadlineVars

THEOREM LocalRank2FairActionEnabled ==
  LocalTerminationRank2 =>
    ENABLED <<ExpireExactDeadline>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF LocalTerminationRank2, ExpireExactDeadline,
       ExactDeadlineDue, ExactActive, SaturatingIncrement,
       replyWriterDeadlineVars

THEOREM LocalRank3FairActionEnabled ==
  LocalTerminationRank3 =>
    ENABLED <<ReachExactDeadline>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF LocalTerminationRank3, ReachExactDeadline,
       ExactWaitingForDeadline, ExactActive, replyWriterDeadlineVars

THEOREM LocalRank4FairActionEnabled ==
  LocalTerminationRank4 =>
    ENABLED <<FirstExactActorDispatch>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF LocalTerminationRank4, FirstExactActorDispatch,
       ExactUndispatched, ExactActive, ScaledDeadline,
       replyWriterDeadlineVars

THEOREM LocalRank1FairActionExits ==
  /\ LocalTerminationRank1
  /\ <<PollPeerWriterFlush>>_replyWriterDeadlineVars
  => LocalTerminationRank1Exit'
BY SMTT(10)
   DEF LocalTerminationRank1, LocalTerminationRank1Exit,
       PollPeerWriterFlush, ExactFlushReady, ResponsiveCursorAdvanced,
       ExactActive

THEOREM LocalRank2FairActionExits ==
  /\ LocalTerminationRank2
  /\ <<ExpireExactDeadline>>_replyWriterDeadlineVars
  => LocalTerminationRank2Exit'
BY SMTT(10)
   DEF LocalTerminationRank2, LocalTerminationRank2Exit,
       LocalTerminationRank1, ExpireExactDeadline,
       ExactDeadlineDue, ExactFlushReady, ExactTerminal,
       ExactActive, SaturatingIncrement

THEOREM LocalRank3FairActionExits ==
  /\ LocalTerminationRank3
  /\ <<ReachExactDeadline>>_replyWriterDeadlineVars
  => LocalTerminationRank3Exit'
PROOF
  <1>1. ASSUME LocalTerminationRank3,
                <<ReachExactDeadline>>_replyWriterDeadlineVars
         PROVE LocalTerminationRank3Exit'
    <2>1. ReplyWriterDeadlineInvariant'
      BY <1>1, ReplyWriterDeadlineBracketPreservesInvariant, SMTT(10)
         DEF LocalTerminationRank3, ReachExactDeadline, Next,
             replyWriterDeadlineVars
    <2> QED BY <1>1, <2>1, SMTT(10)
         DEF LocalTerminationRank3, LocalTerminationRank3Exit,
             LocalTerminationRank1, LocalTerminationRank2,
             ReachExactDeadline, ExactWaitingForDeadline,
             ExactDeadlineDue, ExactFlushReady, ExactTerminal, ExactActive
  <1> QED BY <1>1

THEOREM LocalRank4FairActionExits ==
  /\ LocalTerminationRank4
  /\ <<FirstExactActorDispatch>>_replyWriterDeadlineVars
  => LocalTerminationRank4Exit'
PROOF
  <1>1. ASSUME LocalTerminationRank4,
                <<FirstExactActorDispatch>>_replyWriterDeadlineVars
         PROVE LocalTerminationRank4Exit'
    <2>1. ReplyWriterDeadlineInvariant'
      BY <1>1, ReplyWriterDeadlineBracketPreservesInvariant, SMTT(10)
         DEF LocalTerminationRank4, FirstExactActorDispatch, Next,
             replyWriterDeadlineVars
    <2>2. ExactWaitingForDeadline'
      BY <1>1, SMTT(10)
         DEF LocalTerminationRank4, FirstExactActorDispatch,
             ExactUndispatched, ExactWaitingForDeadline, ExactActive,
             ReplyWriterDeadlineInvariant,
             DeadlineAcquiredAtFirstDispatchInvariant,
             LifecycleShapeInvariant, FlushOutcomeInvariant
    <2> QED BY <2>1, <2>2
         DEF LocalTerminationRank4, LocalTerminationRank4Exit,
             LocalTerminationRank1, LocalTerminationRank2,
             LocalTerminationRank3, FirstExactActorDispatch,
             ExactUndispatched, ExactWaitingForDeadline,
             ExactDeadlineDue, ExactFlushReady, ExactTerminal,
             ExactActive, ScaledDeadline
  <1> QED BY <1>1

THEOREM LocalRank1LeadsToExit ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank1 ~> LocalTerminationRank1Exit)
BY LocalRank1IsNotOrphaned, LocalRank1FairActionEnabled,
   LocalRank1FairActionExits, PTL
   DEF ReplyWriterDeadlineSpec

THEOREM LocalRank2LeadsToExit ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank2 ~> LocalTerminationRank2Exit)
BY LocalRank2IsNotOrphaned, LocalRank2FairActionEnabled,
   LocalRank2FairActionExits, PTL
   DEF ReplyWriterDeadlineSpec

THEOREM LocalRank3LeadsToExit ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank3 ~> LocalTerminationRank3Exit)
BY LocalRank3IsNotOrphaned, LocalRank3FairActionEnabled,
   LocalRank3FairActionExits, PTL
   DEF ReplyWriterDeadlineSpec

THEOREM LocalRank4LeadsToExit ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank4 ~> LocalTerminationRank4Exit)
BY LocalRank4IsNotOrphaned, LocalRank4FairActionEnabled,
   LocalRank4FairActionExits, PTL
   DEF ReplyWriterDeadlineSpec

THEOREM ResponsiveCursorAdvanceIsExactTerminal ==
  ResponsiveCursorAdvanced => ExactTerminal
BY SMTT(5)
   DEF ResponsiveCursorAdvanced, ExactTerminal

THEOREM LocalRank1LeadsToTerminal ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank1 ~> ExactTerminal)
BY LocalRank1LeadsToExit, ResponsiveCursorAdvanceIsExactTerminal, PTL
   DEF LocalTerminationRank1Exit

THEOREM LocalRank2LeadsToTerminal ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank2 ~> ExactTerminal)
BY LocalRank2LeadsToExit, LocalRank1LeadsToTerminal, PTL
   DEF LocalTerminationRank2Exit

THEOREM LocalRank3LeadsToTerminal ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank3 ~> ExactTerminal)
BY LocalRank3LeadsToExit, LocalRank1LeadsToTerminal,
   LocalRank2LeadsToTerminal, PTL
   DEF LocalTerminationRank3Exit

THEOREM LocalRank4LeadsToTerminal ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank4 ~> ExactTerminal)
BY LocalRank4LeadsToExit, LocalRank1LeadsToTerminal,
   LocalRank2LeadsToTerminal, LocalRank3LeadsToTerminal, PTL
   DEF LocalTerminationRank4Exit

THEOREM ExactActiveHasLocalTerminationRank ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactActive
  => \/ LocalTerminationRank1
     \/ LocalTerminationRank2
     \/ LocalTerminationRank3
     \/ LocalTerminationRank4
BY SMTT(10)
   DEF LocalTerminationRank1, LocalTerminationRank2,
       LocalTerminationRank3, LocalTerminationRank4,
       ExactFlushReady, ExactDeadlineDue,
       ExactWaitingForDeadline, ExactUndispatched,
       ReplyWriterDeadlineInvariant, FlushAttemptIdentityInvariant,
       FlushOutcomeInvariant

THEOREM ReplyWriterDeadlineLocalActorTermination ==
  ReplyWriterDeadlineSpec => LocalActorTermination
BY ReplyWriterDeadlineSpecAlwaysInvariant,
   ExactActiveHasLocalTerminationRank,
   LocalRank1LeadsToTerminal, LocalRank2LeadsToTerminal,
   LocalRank3LeadsToTerminal, LocalRank4LeadsToTerminal, PTL
   DEF LocalActorTermination

ResponsiveWriterReceiptAssumption ==
  ExactOutstanding ~> ExactFlushReady

(***************************************************************************
The responsive proof below is suffix-based.  It never assumes receipt
publication.  Instead, local deadline termination repeatedly returns an
outstanding item to a reconnect/dispatch admission window.  Strong fairness
then forces peer-writer admission across those fragmented windows.  Every such
admission creates a publication window, so strong fairness of publication
forces the immutable receipt observation.  The observation is persistent and,
while the cursor is still outstanding, is exactly `ExactFlushReady`.
***************************************************************************)

OutstandingWithoutReceipt ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactOutstanding
  /\ ~writerFlushObserved

ExactAdmissionWindow ==
  /\ OutstandingWithoutReceipt
  /\ ExactActive
  /\ phase = "ActorOwned"
  /\ dispatchStarted

ExactPublicationWindow ==
  /\ OutstandingWithoutReceipt
  /\ ExactActive
  /\ phase = "WriterPending"
  /\ ~ackReady

ExactParkedTerminal ==
  /\ OutstandingWithoutReceipt
  /\ ExactTerminal
  /\ phase = "Parked"

THEOREM OutstandingWithoutReceiptShape ==
  OutstandingWithoutReceipt =>
    ExactActive \/ ExactTerminal
BY SMTT(10)
   DEF OutstandingWithoutReceipt, ExactOutstanding, ExactActive,
       ExactTerminal, ReplyWriterDeadlineInvariant, LifecycleShapeInvariant

THEOREM OutstandingWithoutReceiptIsNotOrphaned ==
  /\ OutstandingWithoutReceipt
  /\ [Next]_replyWriterDeadlineVars
  => OutstandingWithoutReceipt' \/ writerFlushObserved'
BY ReplyWriterDeadlineBracketPreservesInvariant, SMTT(120)
   DEF OutstandingWithoutReceipt, ExactOutstanding, ExactActive,
       ExactFlushReady, ReplyWriterDeadlineInvariant,
       LifecycleShapeInvariant, FlushOutcomeInvariant,
       Next, AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute, InstallReplacementBeforeTerminal,
       ReconnectExactReply, FinishTopologyOutput,
       replyWriterDeadlineVars

THEOREM OutstandingTerminalWithoutReceiptIsParked ==
  /\ OutstandingWithoutReceipt
  /\ ExactTerminal
  => ExactParkedTerminal
BY SMTT(10)
   DEF OutstandingWithoutReceipt, ExactParkedTerminal,
       ExactOutstanding, ExactTerminal,
       ReplyWriterDeadlineInvariant, LifecycleShapeInvariant

THEOREM ExactParkedTerminalIsNotOrphaned ==
  /\ ExactParkedTerminal
  /\ [Next]_replyWriterDeadlineVars
  => ExactParkedTerminal' \/ LocalTerminationRank4'
BY ReplyWriterDeadlineBracketPreservesInvariant, SMTT(120)
   DEF ExactParkedTerminal, OutstandingWithoutReceipt,
       LocalTerminationRank4, ExactOutstanding, ExactTerminal,
       ExactUndispatched, ExactActive,
       Next, AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute, InstallReplacementBeforeTerminal,
       ReconnectExactReply, FinishTopologyOutput,
       replyWriterDeadlineVars

THEOREM ExactParkedTerminalReconnectEnabled ==
  ExactParkedTerminal =>
    ENABLED <<ReconnectExactReply>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF ExactParkedTerminal, OutstandingWithoutReceipt,
       ReconnectExactReply, ExactOutstanding, ExactTerminal,
       replyWriterDeadlineVars

THEOREM ExactParkedTerminalReconnectsToUndispatched ==
  /\ ExactParkedTerminal
  /\ <<ReconnectExactReply>>_replyWriterDeadlineVars
  => LocalTerminationRank4'
BY ReplyWriterDeadlineNextPreservesInvariant, SMTT(20)
   DEF ExactParkedTerminal, OutstandingWithoutReceipt,
       LocalTerminationRank4, ReconnectExactReply,
       ExactOutstanding, ExactTerminal, ExactUndispatched,
       Next, replyWriterDeadlineVars

THEOREM ExactParkedTerminalLeadsToUndispatched ==
  ResponsiveReplyWriterSpec =>
    (ExactParkedTerminal ~> LocalTerminationRank4)
BY ExactParkedTerminalIsNotOrphaned,
   ExactParkedTerminalReconnectEnabled,
   ExactParkedTerminalReconnectsToUndispatched, PTL
   DEF ResponsiveReplyWriterSpec, ReplyWriterDeadlineSpec

THEOREM UndispatchedIsNotOrphanedForAdmission ==
  /\ LocalTerminationRank4
  /\ [Next]_replyWriterDeadlineVars
  => LocalTerminationRank4' \/ ExactAdmissionWindow'
BY ReplyWriterDeadlineBracketPreservesInvariant, SMTT(120)
   DEF LocalTerminationRank4, ExactAdmissionWindow,
       OutstandingWithoutReceipt, ExactOutstanding,
       ExactUndispatched, ExactActive,
       Next, AdmitExactReply, AdmitTopologyOutput,
       FirstExactActorDispatch, FirstTopologyActorDispatch,
       RetryFullPeerWriterQueue, AdmitPeerWriter,
       PublishPeerWriterFlush, PollPeerWriterFlush,
       ReachExactDeadline, ExpireExactDeadline, ClosePeerWriter,
       RetireOldExactRoute, InstallReplacementBeforeTerminal,
       ReconnectExactReply, FinishTopologyOutput,
       replyWriterDeadlineVars

THEOREM FirstDispatchCreatesAdmissionWindow ==
  /\ LocalTerminationRank4
  /\ <<FirstExactActorDispatch>>_replyWriterDeadlineVars
  => ExactAdmissionWindow'
BY ReplyWriterDeadlineNextPreservesInvariant, SMTT(20)
   DEF LocalTerminationRank4, ExactAdmissionWindow,
       OutstandingWithoutReceipt, ExactOutstanding,
       ExactUndispatched, ExactActive, FirstExactActorDispatch,
       Next, replyWriterDeadlineVars

THEOREM UndispatchedLeadsToAdmissionWindow ==
  ReplyWriterDeadlineSpec =>
    (LocalTerminationRank4 ~> ExactAdmissionWindow)
BY UndispatchedIsNotOrphanedForAdmission,
   LocalRank4FairActionEnabled,
   FirstDispatchCreatesAdmissionWindow, PTL
   DEF ReplyWriterDeadlineSpec

THEOREM ActiveWithoutReceiptLeadsToParkedOrReceipt ==
  ReplyWriterDeadlineSpec =>
    ((/\ OutstandingWithoutReceipt
      /\ ExactActive)
       ~> (writerFlushObserved \/ ExactParkedTerminal))
BY OutstandingWithoutReceiptIsNotOrphaned,
   ReplyWriterDeadlineLocalActorTermination,
   OutstandingTerminalWithoutReceiptIsParked, PTL
   DEF LocalActorTermination

THEOREM OutstandingWithoutReceiptLeadsToAdmissionOrReceipt ==
  ResponsiveReplyWriterSpec =>
    (OutstandingWithoutReceipt
       ~> (writerFlushObserved \/ ExactAdmissionWindow))
BY OutstandingWithoutReceiptShape,
   OutstandingWithoutReceiptIsNotOrphaned,
   ActiveWithoutReceiptLeadsToParkedOrReceipt,
   OutstandingTerminalWithoutReceiptIsParked,
   ExactParkedTerminalLeadsToUndispatched,
   UndispatchedLeadsToAdmissionWindow, PTL
   DEF ResponsiveReplyWriterSpec

THEOREM AdmissionWindowEnablesAdmission ==
  ExactAdmissionWindow =>
    ENABLED <<AdmitPeerWriter>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF ExactAdmissionWindow, OutstandingWithoutReceipt,
       AdmitPeerWriter, ExactOutstanding, ExactActive,
       replyWriterDeadlineVars

THEOREM AdmissionCreatesPublicationWindow ==
  /\ ExactAdmissionWindow
  /\ <<AdmitPeerWriter>>_replyWriterDeadlineVars
  => ExactPublicationWindow'
BY ReplyWriterDeadlineNextPreservesInvariant, SMTT(20)
   DEF ExactAdmissionWindow, ExactPublicationWindow,
       OutstandingWithoutReceipt, ExactOutstanding, ExactActive,
       AdmitPeerWriter, Next, replyWriterDeadlineVars

THEOREM PublicationWindowEnablesPublish ==
  ExactPublicationWindow =>
    ENABLED <<PublishPeerWriterFlush>>_replyWriterDeadlineVars
BY ExpandENABLED, SMTT(10)
   DEF ExactPublicationWindow, OutstandingWithoutReceipt,
       PublishPeerWriterFlush, ExactOutstanding, ExactActive,
       replyWriterDeadlineVars

THEOREM PublishCreatesReceiptObservation ==
  /\ ExactPublicationWindow
  /\ <<PublishPeerWriterFlush>>_replyWriterDeadlineVars
  => writerFlushObserved'
BY SMTT(10)
   DEF ExactPublicationWindow, OutstandingWithoutReceipt,
       PublishPeerWriterFlush, ExactOutstanding, ExactActive,
       replyWriterDeadlineVars

THEOREM ResponsiveStrongFairnessToReceiptObservation ==
  ResponsiveReplyWriterSpec =>
    (ExactOutstanding ~> writerFlushObserved)
BY ReplyWriterDeadlineSpecAlwaysInvariant,
   OutstandingWithoutReceiptIsNotOrphaned,
   OutstandingWithoutReceiptLeadsToAdmissionOrReceipt,
   AdmissionWindowEnablesAdmission,
   AdmissionCreatesPublicationWindow,
   PublicationWindowEnablesPublish,
   PublishCreatesReceiptObservation, PTL
   DEF ResponsiveReplyWriterSpec

THEOREM OutstandingReceiptObservationIsReady ==
  /\ ReplyWriterDeadlineInvariant
  /\ ExactOutstanding
  /\ writerFlushObserved
  => ExactFlushReady
BY SMTT(10)
   DEF ExactOutstanding, ExactFlushReady, ExactActive,
       ReplyWriterDeadlineInvariant, FlushAttemptIdentityInvariant,
       LifecycleShapeInvariant, FlushOutcomeInvariant

THEOREM ResponsiveStrongFairnessToReceiptResidual ==
  ResponsiveReplyWriterSpec => ResponsiveWriterReceiptAssumption
BY ResponsiveStrongFairnessToReceiptObservation,
   ReplyWriterDeadlineSpecAlwaysInvariant,
   OutstandingReceiptObservationIsReady, PTL
   DEF ResponsiveReplyWriterSpec, ResponsiveWriterReceiptAssumption

THEOREM ReadyReceiptLeadsToCursorAdvance ==
  ReplyWriterDeadlineSpec =>
    (ExactFlushReady ~> ResponsiveCursorAdvanced)
BY LocalRank1LeadsToExit, ReplyWriterDeadlineSpecAlwaysInvariant, PTL
   DEF LocalTerminationRank1, LocalTerminationRank1Exit,
       ResponsiveCursorAdvanced, ExactFlushReady, ExactActive

THEOREM ConditionalResponsiveWriterCursorLiveness ==
  /\ ReplyWriterDeadlineSpec
  /\ ResponsiveWriterReceiptAssumption
  => ResponsiveReplyWriterCursorLiveness
BY ReadyReceiptLeadsToCursorAdvance, PTL
   DEF ResponsiveWriterReceiptAssumption,
       ResponsiveReplyWriterCursorLiveness

THEOREM ResponsiveReplyWriterCursorLivenessFromStrongFairness ==
  ResponsiveReplyWriterSpec => ResponsiveReplyWriterCursorLiveness
BY ResponsiveStrongFairnessToReceiptResidual,
   ConditionalResponsiveWriterCursorLiveness, PTL
   DEF ResponsiveReplyWriterSpec

THEOREM ReplyWriterDeadlineModelObligation ==
  ReplyWriterDeadlineSpec
    => /\ []ReplyWriterDeadlineInvariant
       /\ [][ReplyWriterDeadlineActionSafety]_replyWriterDeadlineVars
       /\ LocalActorTermination
PROOF
  <1>1. ReplyWriterDeadlineSpec => []ReplyWriterDeadlineInvariant
    BY ReplyWriterDeadlineSpecAlwaysInvariant
  <1>2. [][ReplyWriterDeadlineActionSafety]_replyWriterDeadlineVars
    BY FirstDispatchAcquiresOneFixedDeadline,
       FullQueueRetryPreservesFixedDeadline,
       TimeoutPublishesDistinctTerminalOutcome,
       ClosedAndReconnectPreserveAdaptiveAttempt,
       PublishedWriterReceiptBindsTimeoutAttempt,
       PollRequiresMatchingReceiptTimeoutAttempt,
       ReadyReceiptDisablesRouteRetirement,
       TerminalFenceReadyReceiptWinsEveryDestructiveExit,
       ReadyReceiptSurvivesConnectionReplacement,
       WriterFlushObservationComesOnlyFromPublish,
       WriterFlushObservationIsNeverErased,
       ExpiryCannotTerminateReplacementConnection, PTL
       DEF ReplyWriterDeadlineActionSafety,
           DeadlineRetryIdentityAction, TimeoutIsNotFlushAction,
           ClosedPreservesAttemptAction, ReconnectPreservesAttemptAction,
           WriterFlushAttemptIdentityAction,
           ReadyFlushRetirementExclusionAction,
           TerminalFenceReadyWinsEveryDestructiveExitAction,
           ReadyFlushSurvivesReplacementAction,
           WriterFlushObservationOriginAction,
           WriterFlushObservationMonotonicAction,
           ExactOccurrenceReplacementIsolationAction
  <1>3. ReplyWriterDeadlineSpec => LocalActorTermination
    BY ReplyWriterDeadlineLocalActorTermination
  <1> QED BY <1>1, <1>2, <1>3

=============================================================================
