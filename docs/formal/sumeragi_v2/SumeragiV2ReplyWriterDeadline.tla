---- MODULE SumeragiV2ReplyWriterDeadline ----
EXTENDS Naturals, TLC

(***************************************************************************
Orthogonal executable model for the exact-reply peer-writer deadline.

The existing reply-route pipeline models authenticated ownership, FIFO
attachment, and flush-before-cursor-advance.  This model isolates the local
termination guarantee added at the network actor boundary:

  * the deadline is acquired on the first actor dispatch, before admission to
    the bounded peer-writer queue;
  * a full peer-writer queue cannot restart that absolute deadline;
  * only peer-writer publication can establish the immutable flush witness,
    and a ready witnessed receipt wins at the deadline boundary;
  * the production terminal close-and-immediate-poll fence is abstracted by
    making every destructive exact-reply exit incompatible with a ready
    witnessed receipt, including publication after an optimistic actor poll;
  * timeout is distinct from a successful flush and is the only outcome that
    increases the adaptive timeout attempt;
  * the actor receipt retains that admission attempt independently, and polling
    may advance the cursor only when receipt and target attempts are equal;
  * timeout may retire only the exact accepting connection occurrence;
  * topology output never acquires this exact-reply deadline.

`ReachExactDeadline` is the abstraction of a monotone local timer. It is weakly
fair in `ReplyWriterDeadlineSpec`; publication of a peer-writer receipt is
deliberately not fair there.  Consequently local actor ownership terminates
without assuming that an admitted writer will ever flush.  The stronger
`ResponsiveReplyWriterSpec` adds the separate environmental assumptions used
to state and model-check the responsive cursor property.
***************************************************************************)

CONSTANTS
  BaseDeadline,
  DeadlineCap,
  MaxTimeoutAttempt,
  MaxDispatchRetries

(***************************************************************************
`DeadlineCap` is the finite-model abstraction of production saturation at
`Duration::MAX` on scaling overflow; it is not a configurable protocol
deadline cap. `MaxTimeoutAttempt` abstracts the bounded `u8` attempt, while
the smaller values in TLC configurations are finite state-space bounds.
`MaxDispatchRetries` is also model-only: production retry count is bounded by
the immutable absolute deadline, not by a separate retry-count cap.

The production `u8` attempt and finite `Duration` make each individual expiry
qualitatively reachable, but exponential scaling does not provide a fixed
operational wall-clock SLA. A recovered responsive writer may still publish
and be polled immediately before even a very long current deadline.
***************************************************************************)

Kinds == {"None", "ExactReply", "Topology"}
Phases == {"Idle", "ActorOwned", "WriterPending", "Parked", "Delivered"}
Outcomes == {"None", "Pending", "Flushed", "Closed", "TimedOut"}
Connections == {"NoConnection", "OldConnection", "ReplacementConnection"}

ReplyWriterDeadlineConfiguration ==
  /\ BaseDeadline \in Nat \ {0}
  /\ DeadlineCap \in Nat \ {0}
  /\ BaseDeadline <= DeadlineCap
  /\ MaxTimeoutAttempt \in Nat \ {0}
  /\ MaxDispatchRetries \in Nat \ {0}

SaturatingIncrement(attempt) ==
  IF attempt < MaxTimeoutAttempt THEN attempt + 1 ELSE MaxTimeoutAttempt

ScaledDeadline(attempt) ==
  LET raw == BaseDeadline * (2 ^ attempt)
  IN IF raw <= DeadlineCap THEN raw ELSE DeadlineCap

VARIABLES
  kind,
  phase,
  outcome,
  cursor,
  timeoutAttempt,
  timedOutCount,
  dispatchStarted,
  dispatchRetries,
  deadlineSet,
  deadlineBudget,
  deadlineOrigin,
  deadlineDue,
  ackReady,
  ackTimeoutAttempt,
  writerFlushObserved,
  ackPublished,
  routeWritable,
  occurrenceConnection,
  currentConnection,
  protectedReplacement

replyWriterDeadlineVars ==
  <<kind, phase, outcome, cursor, timeoutAttempt, timedOutCount,
    dispatchStarted, dispatchRetries, deadlineSet, deadlineBudget,
    deadlineOrigin, deadlineDue, ackReady, writerFlushObserved,
    ackTimeoutAttempt,
    ackPublished, routeWritable, occurrenceConnection, currentConnection,
    protectedReplacement>>

ExactActive ==
  /\ kind = "ExactReply"
  /\ phase \in {"ActorOwned", "WriterPending"}
  /\ outcome = "Pending"

ExactTerminal ==
  /\ kind = "ExactReply"
  /\ outcome \in {"Flushed", "Closed", "TimedOut"}

ExactOutstanding ==
  /\ kind = "ExactReply"
  /\ cursor = 0

ExactUndispatched ==
  /\ ExactActive
  /\ ~dispatchStarted

ExactWaitingForDeadline ==
  /\ ExactActive
  /\ dispatchStarted
  /\ ~deadlineDue
  /\ ~ackReady

ExactDeadlineDue ==
  /\ ExactActive
  /\ deadlineDue
  /\ ~ackReady
  /\ ~writerFlushObserved

ExactFlushReady ==
  /\ ExactActive
  /\ phase = "WriterPending"
  /\ ackReady
  /\ ackTimeoutAttempt = timeoutAttempt
  /\ writerFlushObserved

(***************************************************************************
`ExactFlushReady` is the atomic abstraction of a peer-writer send which won
the production terminal receiver-close race. The Rust implementation reaches
that boundary by closing the exact oneshot receiver and polling it immediately
before inactive-authority close, replacement retirement, or timeout side
effects, and before pending-queue cancellation, shutdown, or abort drops. This
predicate also requires the receipt-bound timeout attempt to equal the mutable
target attempt before polling can advance. This model states that boundary; it
is not itself a Rust-to-TLA refinement proof.
***************************************************************************)

ResponsiveCursorAdvanced ==
  /\ kind = "ExactReply"
  /\ cursor = 1
  /\ phase = "Delivered"
  /\ outcome = "Flushed"

Init ==
  /\ ReplyWriterDeadlineConfiguration
  /\ kind = "None"
  /\ phase = "Idle"
  /\ outcome = "None"
  /\ cursor = 0
  /\ timeoutAttempt = 0
  /\ timedOutCount = 0
  /\ dispatchStarted = FALSE
  /\ dispatchRetries = 0
  /\ deadlineSet = FALSE
  /\ deadlineBudget = 0
  /\ deadlineOrigin = 0
  /\ deadlineDue = FALSE
  /\ ackReady = FALSE
  /\ ackTimeoutAttempt = 0
  /\ writerFlushObserved = FALSE
  /\ ackPublished = FALSE
  /\ routeWritable = FALSE
  /\ occurrenceConnection = "NoConnection"
  /\ currentConnection = "NoConnection"
  /\ protectedReplacement = "NoConnection"

AdmitExactReply ==
  /\ phase = "Idle"
  /\ ~writerFlushObserved
  /\ kind' = "ExactReply"
  /\ phase' = "ActorOwned"
  /\ outcome' = "Pending"
  /\ cursor' = 0
  /\ timeoutAttempt' = 0
  /\ timedOutCount' = 0
  /\ dispatchStarted' = FALSE
  /\ dispatchRetries' = 0
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ writerFlushObserved' = FALSE
  /\ ackPublished' = FALSE
  /\ routeWritable' = TRUE
  /\ occurrenceConnection' = "OldConnection"
  /\ currentConnection' = "OldConnection"
  /\ protectedReplacement' = "NoConnection"

AdmitTopologyOutput ==
  /\ phase = "Idle"
  /\ ~writerFlushObserved
  /\ kind' = "Topology"
  /\ phase' = "ActorOwned"
  /\ outcome' = "Pending"
  /\ cursor' = 0
  /\ timeoutAttempt' = 0
  /\ timedOutCount' = 0
  /\ dispatchStarted' = FALSE
  /\ dispatchRetries' = 0
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ writerFlushObserved' = FALSE
  /\ ackPublished' = FALSE
  /\ routeWritable' = TRUE
  /\ occurrenceConnection' = "NoConnection"
  /\ currentConnection' = "NoConnection"
  /\ protectedReplacement' = "NoConnection"

FirstExactActorDispatch ==
  /\ ExactUndispatched
  /\ dispatchStarted' = TRUE
  /\ deadlineSet' = TRUE
  /\ deadlineBudget' = ScaledDeadline(timeoutAttempt)
  /\ deadlineOrigin' = 0
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchRetries, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

FirstTopologyActorDispatch ==
  /\ kind = "Topology"
  /\ phase = "ActorOwned"
  /\ ~dispatchStarted
  /\ dispatchStarted' = TRUE
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchRetries, deadlineSet,
                 deadlineBudget, deadlineOrigin, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

RetryFullPeerWriterQueue ==
  /\ phase = "ActorOwned"
  /\ dispatchStarted
  /\ dispatchRetries < MaxDispatchRetries
  /\ dispatchRetries' = dispatchRetries + 1
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchStarted, deadlineSet,
                 deadlineBudget, deadlineOrigin, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

AdmitPeerWriter ==
  /\ ExactActive
  /\ phase = "ActorOwned"
  /\ dispatchStarted
  /\ phase' = "WriterPending"
  /\ UNCHANGED <<kind, outcome, cursor, timeoutAttempt, timedOutCount,
                 dispatchStarted, dispatchRetries, deadlineSet,
                 deadlineBudget, deadlineOrigin, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

PublishPeerWriterFlush ==
  /\ ExactActive
  /\ phase = "WriterPending"
  /\ ~ackReady
  /\ ackReady' = TRUE
  /\ ackTimeoutAttempt' = timeoutAttempt
  /\ writerFlushObserved' = TRUE
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchStarted, dispatchRetries,
                 deadlineSet, deadlineBudget, deadlineOrigin, deadlineDue,
                 ackPublished, routeWritable, occurrenceConnection,
                 currentConnection, protectedReplacement>>

PollPeerWriterFlush ==
  /\ ExactFlushReady
  /\ phase' = "Delivered"
  /\ outcome' = "Flushed"
  /\ cursor' = 1
  /\ timeoutAttempt' = 0
  /\ timedOutCount' = 0
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ ackPublished' = TRUE
  /\ UNCHANGED <<kind, dispatchStarted, dispatchRetries,
                 writerFlushObserved, routeWritable, occurrenceConnection,
                 currentConnection, protectedReplacement>>

ReachExactDeadline ==
  /\ ExactWaitingForDeadline
  /\ deadlineDue' = TRUE
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchStarted, dispatchRetries,
                 deadlineSet, deadlineBudget, deadlineOrigin, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

ExpireExactDeadline ==
  /\ ExactDeadlineDue
  /\ phase' = "Parked"
  /\ outcome' = "TimedOut"
  /\ timeoutAttempt' = SaturatingIncrement(timeoutAttempt)
  /\ timedOutCount' = SaturatingIncrement(timedOutCount)
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ ackPublished' = FALSE
  /\ currentConnection' =
       IF currentConnection = occurrenceConnection
       THEN "NoConnection"
       ELSE currentConnection
  /\ UNCHANGED <<kind, cursor, dispatchStarted, dispatchRetries,
                 ackReady, writerFlushObserved, occurrenceConnection,
                 protectedReplacement>>

(***************************************************************************
This action abstracts the outer exact actor occurrence ending with caller ACK
status `Closed`. A downstream writer oneshot closing is retried by production
inside the same actor item and immutable deadline; it does not directly
terminalize the caller ACK.
***************************************************************************)
ClosePeerWriter ==
  /\ ExactActive
  /\ phase = "WriterPending"
  /\ ~ackReady
  /\ phase' = "Parked"
  /\ outcome' = "Closed"
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount,
                 dispatchStarted, dispatchRetries, ackReady,
                 writerFlushObserved, ackPublished, occurrenceConnection,
                 currentConnection, protectedReplacement>>

RetireOldExactRoute ==
  /\ ExactActive
  /\ dispatchStarted
  /\ ~ackReady
  /\ occurrenceConnection = "OldConnection"
  /\ phase' = "Parked"
  /\ outcome' = "Closed"
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount,
                 dispatchStarted, dispatchRetries, ackReady,
                 writerFlushObserved, ackPublished, occurrenceConnection,
                 currentConnection, protectedReplacement>>

InstallReplacementBeforeTerminal ==
  /\ ExactActive
  /\ occurrenceConnection = "OldConnection"
  /\ currentConnection = "OldConnection"
  /\ currentConnection' = "ReplacementConnection"
  /\ protectedReplacement' = "ReplacementConnection"
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchStarted, dispatchRetries,
                 deadlineSet, deadlineBudget, deadlineOrigin, deadlineDue,
                 ackReady, ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection>>

ReconnectExactReply ==
  /\ kind = "ExactReply"
  /\ phase = "Parked"
  /\ phase' = "ActorOwned"
  /\ outcome' = "Pending"
  /\ dispatchStarted' = FALSE
  /\ dispatchRetries' = 0
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ ackPublished' = FALSE
  /\ routeWritable' = TRUE
  /\ occurrenceConnection' = "ReplacementConnection"
  /\ currentConnection' = "ReplacementConnection"
  /\ protectedReplacement' = "NoConnection"
  /\ UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount,
                 writerFlushObserved>>

FinishTopologyOutput ==
  /\ kind = "Topology"
  /\ phase = "ActorOwned"
  /\ dispatchStarted
  /\ phase' = "Delivered"
  /\ outcome' = "Flushed"
  /\ cursor' = 1
  /\ ackPublished' = TRUE
  /\ UNCHANGED <<kind, timeoutAttempt, timedOutCount, dispatchStarted,
                 dispatchRetries, deadlineSet, deadlineBudget,
                 deadlineOrigin, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, routeWritable, occurrenceConnection,
                 currentConnection,
                 protectedReplacement>>

Next ==
  \/ AdmitExactReply
  \/ AdmitTopologyOutput
  \/ FirstExactActorDispatch
  \/ FirstTopologyActorDispatch
  \/ RetryFullPeerWriterQueue
  \/ AdmitPeerWriter
  \/ PublishPeerWriterFlush
  \/ PollPeerWriterFlush
  \/ ReachExactDeadline
  \/ ExpireExactDeadline
  \/ ClosePeerWriter
  \/ RetireOldExactRoute
  \/ InstallReplacementBeforeTerminal
  \/ ReconnectExactReply
  \/ FinishTopologyOutput

ReplyWriterDeadlineTypeInvariant ==
  /\ ReplyWriterDeadlineConfiguration
  /\ kind \in Kinds
  /\ phase \in Phases
  /\ outcome \in Outcomes
  /\ cursor \in 0..1
  /\ timeoutAttempt \in 0..MaxTimeoutAttempt
  /\ timedOutCount \in 0..MaxTimeoutAttempt
  /\ dispatchStarted \in BOOLEAN
  /\ dispatchRetries \in 0..MaxDispatchRetries
  /\ deadlineSet \in BOOLEAN
  /\ (\/ deadlineBudget \in 0..DeadlineCap
      \/ deadlineBudget = ScaledDeadline(timeoutAttempt))
  /\ deadlineOrigin \in 0..MaxDispatchRetries
  /\ deadlineDue \in BOOLEAN
  /\ ackReady \in BOOLEAN
  /\ ackTimeoutAttempt \in 0..MaxTimeoutAttempt
  /\ writerFlushObserved \in BOOLEAN
  /\ ackPublished \in BOOLEAN
  /\ routeWritable \in BOOLEAN
  /\ occurrenceConnection \in Connections
  /\ currentConnection \in Connections
  /\ protectedReplacement \in Connections

DeadlineAcquiredAtFirstDispatchInvariant ==
  /\ deadlineSet =>
       /\ ExactActive
       /\ dispatchStarted
       /\ deadlineBudget = ScaledDeadline(timeoutAttempt)
       /\ deadlineOrigin = 0
  /\ ExactActive /\ dispatchStarted => deadlineSet
  /\ deadlineDue => deadlineSet

TopologyHasNoExactDeadlineInvariant ==
  kind = "Topology" =>
    /\ ~deadlineSet
    /\ deadlineBudget = 0
    /\ deadlineOrigin = 0
    /\ ~deadlineDue

AdaptiveAttemptInvariant ==
  timeoutAttempt = timedOutCount

FlushAttemptIdentityInvariant ==
  /\ ackReady => ackTimeoutAttempt = timeoutAttempt
  /\ ~ackReady => ackTimeoutAttempt = 0

LifecycleShapeInvariant ==
  /\ (phase = "Idle" <=> kind = "None")
  /\ phase \in {"ActorOwned", "WriterPending"} =>
       /\ kind \in {"ExactReply", "Topology"}
       /\ outcome = "Pending"
       /\ cursor = 0
       /\ routeWritable
       /\ ~ackPublished
  /\ phase = "Parked" =>
       /\ kind = "ExactReply"
       /\ outcome \in {"Closed", "TimedOut"}
       /\ cursor = 0
       /\ dispatchStarted
       /\ ~routeWritable
       /\ ~ackReady
       /\ ~ackPublished
       /\ ~deadlineSet
  /\ phase = "Delivered" =>
       /\ kind \in {"ExactReply", "Topology"}
       /\ outcome = "Flushed"
       /\ cursor = 1
       /\ dispatchStarted
       /\ ackPublished
       /\ ~ackReady
       /\ ~deadlineSet
  /\ phase = "WriterPending" => dispatchStarted

FlushOutcomeInvariant ==
  /\ ackReady =>
       /\ ExactActive
       /\ phase = "WriterPending"
  /\ writerFlushObserved =>
       kind = "ExactReply"
  /\ kind = "ExactReply" =>
       (writerFlushObserved <=> (ackReady \/ ackPublished))
  /\ ackPublished <=>
       /\ phase = "Delivered"
       /\ outcome = "Flushed"
       /\ cursor = 1
  /\ outcome = "Flushed" => ackPublished
  /\ outcome \in {"Closed", "TimedOut"} =>
       /\ kind = "ExactReply"
       /\ phase = "Parked"
       /\ cursor = 0
       /\ ~routeWritable
       /\ ~ackReady

ExactConnectionIsolationInvariant ==
  protectedReplacement # "NoConnection" =>
    /\ protectedReplacement = "ReplacementConnection"
    /\ currentConnection = protectedReplacement
    /\ occurrenceConnection = "OldConnection"

ReplyWriterDeadlineInvariant ==
  /\ ReplyWriterDeadlineTypeInvariant
  /\ DeadlineAcquiredAtFirstDispatchInvariant
  /\ TopologyHasNoExactDeadlineInvariant
  /\ AdaptiveAttemptInvariant
  /\ FlushAttemptIdentityInvariant
  /\ LifecycleShapeInvariant
  /\ FlushOutcomeInvariant
  /\ ExactConnectionIsolationInvariant

DeadlineRetryIdentityAction ==
  RetryFullPeerWriterQueue =>
    /\ deadlineSet' = deadlineSet
    /\ deadlineBudget' = deadlineBudget
    /\ deadlineOrigin' = deadlineOrigin
    /\ deadlineDue' = deadlineDue

TimeoutIsNotFlushAction ==
  ExpireExactDeadline =>
    /\ outcome' = "TimedOut"
    /\ cursor' = cursor
    /\ ~writerFlushObserved'
    /\ ~ackPublished'

ClosedPreservesAttemptAction ==
  (ClosePeerWriter \/ RetireOldExactRoute) =>
    /\ timeoutAttempt' = timeoutAttempt
    /\ timedOutCount' = timedOutCount

ReconnectPreservesAttemptAction ==
  ReconnectExactReply =>
    /\ timeoutAttempt' = timeoutAttempt
    /\ timedOutCount' = timedOutCount

WriterFlushAttemptIdentityAction ==
  PublishPeerWriterFlush =>
    ackTimeoutAttempt' = timeoutAttempt

TerminalFenceReadyWinsEveryDestructiveExitAction ==
  ExactFlushReady =>
    /\ ~ExpireExactDeadline
    /\ ~ClosePeerWriter
    /\ ~RetireOldExactRoute

ReadyFlushRetirementExclusionAction ==
  TerminalFenceReadyWinsEveryDestructiveExitAction

ReadyFlushSurvivesReplacementAction ==
  /\ ExactFlushReady
  /\ InstallReplacementBeforeTerminal
  => ExactFlushReady'

WriterFlushObservationOriginAction ==
  Next =>
    (/\ ~writerFlushObserved
     /\ writerFlushObserved'
     => PublishPeerWriterFlush)

WriterFlushObservationMonotonicAction ==
  /\ ReplyWriterDeadlineInvariant
  /\ Next
  /\ writerFlushObserved
  => writerFlushObserved'

TopologyNeverAcquiresDeadlineAction ==
  /\ ReplyWriterDeadlineInvariant
  /\ kind = "Topology"
  /\ Next
  => ~deadlineSet'

ExactOccurrenceReplacementIsolationAction ==
  /\ ReplyWriterDeadlineInvariant
  /\ protectedReplacement = "ReplacementConnection"
  /\ ExpireExactDeadline
  => /\ currentConnection' = "ReplacementConnection"
     /\ protectedReplacement' = "ReplacementConnection"

ReplyWriterDeadlineActionSafety ==
  /\ DeadlineRetryIdentityAction
  /\ TimeoutIsNotFlushAction
  /\ ClosedPreservesAttemptAction
  /\ ReconnectPreservesAttemptAction
  /\ WriterFlushAttemptIdentityAction
  /\ ReadyFlushRetirementExclusionAction
  /\ TerminalFenceReadyWinsEveryDestructiveExitAction
  /\ ReadyFlushSurvivesReplacementAction
  /\ WriterFlushObservationOriginAction
  /\ WriterFlushObservationMonotonicAction
  /\ ExactOccurrenceReplacementIsolationAction

ReplyWriterDeadlineSpec ==
  /\ Init
  /\ [][Next]_replyWriterDeadlineVars
  /\ WF_replyWriterDeadlineVars(FirstExactActorDispatch)
  /\ WF_replyWriterDeadlineVars(ReachExactDeadline)
  /\ WF_replyWriterDeadlineVars(ExpireExactDeadline)
  /\ WF_replyWriterDeadlineVars(PollPeerWriterFlush)

ResponsiveReplyWriterSpec ==
  /\ ReplyWriterDeadlineSpec
  /\ WF_replyWriterDeadlineVars(ReconnectExactReply)
  /\ SF_replyWriterDeadlineVars(AdmitPeerWriter)
  /\ SF_replyWriterDeadlineVars(PublishPeerWriterFlush)

LocalActorTermination ==
  ExactActive ~> ExactTerminal

ResponsiveReplyWriterCursorLiveness ==
  ExactOutstanding ~> ResponsiveCursorAdvanced

=============================================================================
