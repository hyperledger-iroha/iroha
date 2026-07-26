---- MODULE SumeragiV2ReplyWriterDeadlineMutation ----
EXTENDS SumeragiV2ReplyWriterDeadline

(***************************************************************************
Adversarial variants for the exact-reply writer deadline.

Each mode substitutes exactly one unsafe transition. Eleven paired TLC
configurations demonstrate that the fixed state invariant detects deadline
restart, fabricated flush, wrong adaptive-attempt accounting, scaling beyond
the finite abstraction of production `Duration::MAX` saturation, topology
coupling, stale connection retirement, and timeout or retirement consuming an
already-ready witnessed receipt. The wrong-receipt-attempt mutant reaches a
retry at attempt one and publishes an actor receipt still bound to attempt
zero. The inactive-close mutant independently removes the terminal
close-and-poll fence. The separate witness-erasure mutant preserves that state
invariant and is rejected by the explicit monotonic transition property.
***************************************************************************)

CONSTANT MutationMode

MutationModes ==
  {"Fixed",
   "ResetDeadlineOnQueueRetry",
   "TimeoutAsFlushed",
   "IncrementAttemptOnClosed",
   "ResetAttemptOnReconnect",
   "UncappedAdaptiveDeadline",
   "TopologyAcquiresDeadline",
   "TerminateReplacementConnection",
   "TimeoutBeatsReadyFlush",
   "PublishWrongTimeoutAttempt",
   "CloseReadyFlushWithoutTerminalFence",
   "RetireReadyFlush",
   "EraseReadyFlushWitness"}

MutationConfiguration ==
  /\ ReplyWriterDeadlineConfiguration
  /\ MutationMode \in MutationModes

ResetDeadlineOnQueueRetry ==
  /\ phase = "ActorOwned"
  /\ dispatchStarted
  /\ deadlineSet
  /\ dispatchRetries < MaxDispatchRetries
  /\ dispatchRetries' = dispatchRetries + 1
  /\ deadlineOrigin' = dispatchRetries + 1
  /\ deadlineDue' = FALSE
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchStarted, deadlineSet,
                 deadlineBudget, ackReady, ackTimeoutAttempt, writerFlushObserved,
                 ackPublished, routeWritable, occurrenceConnection,
                 currentConnection,
                 protectedReplacement>>

TimeoutAsFlushed ==
  /\ ExactDeadlineDue
  /\ phase' = "Delivered"
  /\ outcome' = "Flushed"
  /\ cursor' = 1
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ ackPublished' = TRUE
  /\ UNCHANGED <<kind, timeoutAttempt, timedOutCount, dispatchStarted,
                 dispatchRetries, ackReady, writerFlushObserved,
                 routeWritable,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

IncrementAttemptOnClosed ==
  /\ ExactActive
  /\ phase = "WriterPending"
  /\ ~ackReady
  /\ phase' = "Parked"
  /\ outcome' = "Closed"
  /\ timeoutAttempt' = SaturatingIncrement(timeoutAttempt)
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ UNCHANGED <<kind, cursor, timedOutCount, dispatchStarted,
                 dispatchRetries, ackReady, writerFlushObserved,
                 ackPublished,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

ResetAttemptOnReconnect ==
  /\ kind = "ExactReply"
  /\ phase = "Parked"
  /\ phase' = "ActorOwned"
  /\ outcome' = "Pending"
  /\ timeoutAttempt' = 0
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
  /\ UNCHANGED <<kind, cursor, timedOutCount, writerFlushObserved>>

UncappedAdaptiveDeadline ==
  /\ ExactUndispatched
  /\ dispatchStarted' = TRUE
  /\ deadlineSet' = TRUE
  /\ deadlineBudget' = BaseDeadline * (2 ^ timeoutAttempt)
  /\ deadlineOrigin' = 0
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchRetries, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection,
                 currentConnection, protectedReplacement>>

TopologyAcquiresDeadline ==
  /\ kind = "Topology"
  /\ phase = "ActorOwned"
  /\ ~dispatchStarted
  /\ dispatchStarted' = TRUE
  /\ deadlineSet' = TRUE
  /\ deadlineBudget' = BaseDeadline
  /\ deadlineOrigin' = 0
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchRetries, deadlineDue, ackReady,
                 ackTimeoutAttempt, writerFlushObserved, ackPublished, routeWritable,
                 occurrenceConnection,
                 currentConnection, protectedReplacement>>

TerminateReplacementConnection ==
  /\ ExactDeadlineDue
  /\ protectedReplacement = "ReplacementConnection"
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
  /\ currentConnection' = "NoConnection"
  /\ UNCHANGED <<kind, cursor, dispatchStarted, dispatchRetries,
                 ackReady, writerFlushObserved, ackPublished,
                 occurrenceConnection,
                 protectedReplacement>>

TimeoutBeatsReadyFlush ==
  /\ ExactActive
  /\ deadlineDue
  /\ ackReady
  /\ phase' = "Parked"
  /\ outcome' = "TimedOut"
  /\ timeoutAttempt' = SaturatingIncrement(timeoutAttempt)
  /\ timedOutCount' = SaturatingIncrement(timedOutCount)
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ UNCHANGED <<kind, cursor, dispatchStarted, dispatchRetries,
                 writerFlushObserved, ackPublished, occurrenceConnection,
                 currentConnection, protectedReplacement>>

PublishWrongTimeoutAttempt ==
  /\ ExactActive
  /\ phase = "WriterPending"
  /\ timeoutAttempt > 0
  /\ ~ackReady
  /\ ackReady' = TRUE
  /\ ackTimeoutAttempt' = timeoutAttempt - 1
  /\ writerFlushObserved' = TRUE
  /\ UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt,
                 timedOutCount, dispatchStarted, dispatchRetries,
                 deadlineSet, deadlineBudget, deadlineOrigin, deadlineDue,
                 ackPublished, routeWritable, occurrenceConnection,
                 currentConnection, protectedReplacement>>

CloseReadyFlushWithoutTerminalFence ==
  /\ ExactFlushReady
  /\ phase' = "Parked"
  /\ outcome' = "Closed"
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount,
                 dispatchStarted, dispatchRetries, writerFlushObserved,
                 ackPublished, occurrenceConnection, currentConnection,
                 protectedReplacement>>

RetireReadyFlush ==
  /\ ExactFlushReady
  /\ occurrenceConnection = "OldConnection"
  /\ phase' = "Parked"
  /\ outcome' = "Closed"
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount,
                 dispatchStarted, dispatchRetries, writerFlushObserved,
                 ackPublished, occurrenceConnection, currentConnection,
                 protectedReplacement>>

EraseReadyFlushWitness ==
  /\ ExactFlushReady
  /\ occurrenceConnection = "OldConnection"
  /\ phase' = "Parked"
  /\ outcome' = "Closed"
  /\ deadlineSet' = FALSE
  /\ deadlineBudget' = 0
  /\ deadlineOrigin' = 0
  /\ deadlineDue' = FALSE
  /\ routeWritable' = FALSE
  /\ ackReady' = FALSE
  /\ ackTimeoutAttempt' = 0
  /\ writerFlushObserved' = FALSE
  /\ UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount,
                 dispatchStarted, dispatchRetries, ackPublished,
                 occurrenceConnection, currentConnection,
                 protectedReplacement>>

(***************************************************************************
Unlike the other single-defect mutants, erasing both the ready bit and its
history witness leaves the post-state invariant intact. The transition
property below rejects that instrumentation corruption directly.
***************************************************************************)

MutationFirstDispatch ==
  IF MutationMode = "UncappedAdaptiveDeadline"
  THEN UncappedAdaptiveDeadline
  ELSE FirstExactActorDispatch

MutationTopologyDispatch ==
  IF MutationMode = "TopologyAcquiresDeadline"
  THEN TopologyAcquiresDeadline
  ELSE FirstTopologyActorDispatch

MutationQueueRetry ==
  IF MutationMode = "ResetDeadlineOnQueueRetry"
  THEN ResetDeadlineOnQueueRetry
  ELSE RetryFullPeerWriterQueue

MutationPublish ==
  IF MutationMode = "PublishWrongTimeoutAttempt"
  THEN PublishWrongTimeoutAttempt
  ELSE PublishPeerWriterFlush

MutationExpire ==
  CASE MutationMode = "TimeoutAsFlushed" ->
         TimeoutAsFlushed
    [] MutationMode = "TerminateReplacementConnection" ->
         TerminateReplacementConnection
    [] MutationMode = "TimeoutBeatsReadyFlush" ->
         TimeoutBeatsReadyFlush
    [] OTHER ->
         ExpireExactDeadline

MutationClose ==
  CASE MutationMode = "IncrementAttemptOnClosed" ->
         IncrementAttemptOnClosed
    [] MutationMode = "CloseReadyFlushWithoutTerminalFence" ->
         CloseReadyFlushWithoutTerminalFence
    [] OTHER ->
         ClosePeerWriter

MutationReconnect ==
  IF MutationMode = "ResetAttemptOnReconnect"
  THEN ResetAttemptOnReconnect
  ELSE ReconnectExactReply

MutationRetire ==
  CASE MutationMode = "RetireReadyFlush" ->
         RetireReadyFlush
    [] MutationMode = "EraseReadyFlushWitness" ->
         EraseReadyFlushWitness
    [] OTHER ->
         RetireOldExactRoute

MutationInit ==
  /\ Init
  /\ MutationConfiguration

MutationNext ==
  \/ AdmitExactReply
  \/ AdmitTopologyOutput
  \/ MutationFirstDispatch
  \/ MutationTopologyDispatch
  \/ MutationQueueRetry
  \/ AdmitPeerWriter
  \/ MutationPublish
  \/ PollPeerWriterFlush
  \/ ReachExactDeadline
  \/ MutationExpire
  \/ MutationClose
  \/ MutationRetire
  \/ InstallReplacementBeforeTerminal
  \/ MutationReconnect
  \/ FinishTopologyOutput

MutationSpec ==
  /\ MutationInit
  /\ [][MutationNext]_replyWriterDeadlineVars

MutationWriterFlushObservationMonotonicAction ==
  /\ MutationNext
  /\ writerFlushObserved
  => writerFlushObserved'

MutationWriterFlushObservationMonotonicity ==
  [][MutationWriterFlushObservationMonotonicAction]_replyWriterDeadlineVars

=============================================================================
