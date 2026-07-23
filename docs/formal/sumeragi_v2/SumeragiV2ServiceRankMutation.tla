---- MODULE SumeragiV2ServiceRankMutation ----
EXTENDS Naturals

(***************************************************************************
Small adversarial scheduler for the candidate-value replacement bug.

OldSpec permits an equal retransmission to enter behind the occurrence that
currently owns the protected candidate value. Dispatch removes the oldest
occurrence, leaving its equal replacement. The two-state fair lasso therefore
services every occurrence but never makes the value unowned or lowers its rank
from <<3, 5>>.

CoalescedSpec admits an equal value only while it is not already queued.
Every ownership interval must then cross the unowned state before a later
retransmission may start a new interval.
***************************************************************************)

VARIABLES queuedCopies, deferredOwned, pendingEqual

vars == <<queuedCopies, deferredOwned, pendingEqual>>

CandidateOwned == deferredOwned \/ queuedCopies > 0

CandidateValueRank ==
  IF deferredOwned
  THEN <<2, 1>>
  ELSE IF queuedCopies = 1 THEN <<3, 5>> ELSE <<3, 8>>

ServiceRankLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
        /\ left[2] < right[2]

OldInit ==
  /\ queuedCopies = 1
  /\ deferredOwned = FALSE
  /\ pendingEqual = FALSE

EnqueueEqualReplacement ==
  /\ queuedCopies = 1
  /\ queuedCopies' = 2
  /\ UNCHANGED <<deferredOwned, pendingEqual>>

DispatchOldestCopy ==
  /\ queuedCopies = 2
  /\ queuedCopies' = 1
  /\ UNCHANGED <<deferredOwned, pendingEqual>>

OldNext == EnqueueEqualReplacement \/ DispatchOldestCopy

OldSpec ==
  OldInit
    /\ [][OldNext]_vars
    /\ WF_vars(EnqueueEqualReplacement)
    /\ WF_vars(DispatchOldestCopy)

OldValueRankProgress ==
  (CandidateOwned /\ CandidateValueRank = <<3, 5>>)
    ~> (~CandidateOwned
         \/ ServiceRankLess(CandidateValueRank, <<3, 5>>))

CoalescedInit ==
  /\ queuedCopies = 1
  /\ deferredOwned = FALSE
  /\ pendingEqual = FALSE

DispatchOnlyCopy ==
  /\ queuedCopies = 1
  /\ queuedCopies' = 0
  /\ UNCHANGED <<deferredOwned, pendingEqual>>

AdmitAfterOwnershipEnds ==
  /\ queuedCopies = 0
  /\ queuedCopies' = 1
  /\ UNCHANGED <<deferredOwned, pendingEqual>>

CoalescedNext == DispatchOnlyCopy \/ AdmitAfterOwnershipEnds

CoalescedSpec ==
  CoalescedInit
    /\ [][CoalescedNext]_vars
    /\ WF_vars(DispatchOnlyCopy)
    /\ WF_vars(AdmitAfterOwnershipEnds)

CoalescedValueRankProgress ==
  (CandidateOwned /\ CandidateValueRank = <<3, 5>>)
    ~> (~CandidateOwned
         \/ ServiceRankLess(CandidateValueRank, <<3, 5>>))

(***************************************************************************
The second mutation isolates the deferred-owner refinement gap.  If ingress
checks only the live command queue, an equal value may enter behind a deferred
owner.  Servicing the deferred occurrence then exposes the replacement at the
higher runtime-queue stage, so ownership never exits or decreases from
<<2, 1>>.  Scheduler-wide coalescing consumes that pending occurrence without
creating a second owner.
***************************************************************************)

DeferredReplacementInit ==
  /\ queuedCopies = 0
  /\ deferredOwned = TRUE
  /\ pendingEqual = TRUE

AdmitEqualWhileDeferred ==
  /\ deferredOwned
  /\ pendingEqual
  /\ queuedCopies' = 1
  /\ pendingEqual' = FALSE
  /\ UNCHANGED deferredOwned

CoalesceEqualWhileDeferred ==
  /\ deferredOwned
  /\ pendingEqual
  /\ queuedCopies' = queuedCopies
  /\ pendingEqual' = FALSE
  /\ UNCHANGED deferredOwned

ServiceDeferredOwner ==
  /\ deferredOwned
  /\ deferredOwned' = FALSE
  /\ UNCHANGED <<queuedCopies, pendingEqual>>

TerminalTick ==
  /\ ~deferredOwned
  /\ pendingEqual' = ~pendingEqual
  /\ UNCHANGED <<queuedCopies, deferredOwned>>

DeferredReplacementOldNext ==
  AdmitEqualWhileDeferred \/ ServiceDeferredOwner \/ TerminalTick

DeferredReplacementOldSpec ==
  DeferredReplacementInit
    /\ [][DeferredReplacementOldNext]_vars
    /\ WF_vars(AdmitEqualWhileDeferred)
    /\ WF_vars(ServiceDeferredOwner)

DeferredReplacementCoalescedNext ==
  CoalesceEqualWhileDeferred \/ ServiceDeferredOwner \/ TerminalTick

DeferredReplacementCoalescedSpec ==
  DeferredReplacementInit
    /\ [][DeferredReplacementCoalescedNext]_vars
    /\ WF_vars(CoalesceEqualWhileDeferred)
    /\ WF_vars(ServiceDeferredOwner)

DeferredReplacementRankProgress ==
  (CandidateOwned /\ CandidateValueRank = <<2, 1>>)
    ~> (~CandidateOwned
         \/ ServiceRankLess(CandidateValueRank, <<2, 1>>))

=============================================================================
