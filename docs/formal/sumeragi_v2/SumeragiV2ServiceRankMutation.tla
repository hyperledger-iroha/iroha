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

VARIABLE queuedCopies

vars == <<queuedCopies>>

CandidateOwned == queuedCopies > 0

CandidateValueRank ==
  IF queuedCopies = 1 THEN <<3, 5>> ELSE <<3, 8>>

ServiceRankLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
        /\ left[2] < right[2]

OldInit == queuedCopies = 1

EnqueueEqualReplacement ==
  /\ queuedCopies = 1
  /\ queuedCopies' = 2

DispatchOldestCopy ==
  /\ queuedCopies = 2
  /\ queuedCopies' = 1

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

CoalescedInit == queuedCopies = 1

DispatchOnlyCopy ==
  /\ queuedCopies = 1
  /\ queuedCopies' = 0

AdmitAfterOwnershipEnds ==
  /\ queuedCopies = 0
  /\ queuedCopies' = 1

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

=============================================================================
