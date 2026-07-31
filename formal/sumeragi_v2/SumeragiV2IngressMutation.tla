---- MODULE SumeragiV2IngressMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Small adversarial ingress model for same-source head-of-line blocking.

OldSpec inspects only the lane head.  An auxiliary item whose downstream
resource is unavailable therefore hides an admissible consensus-progress item
behind it forever, even though weak fairness is attached to the old drain
action.  IndexedSpec scans from the oldest entry and removes the first
admissible item, exactly as FairV2Ingress::try_recv_if does.  The blocked item
remains queued and the progress occurrence is serviced.
***************************************************************************)

VARIABLES lane, progressServed

vars == <<lane, progressServed>>

BlockedAux == "BlockedAux"
Progress == "Progress"

ProgressPresent == Progress \in {lane[index]: index \in 1..Len(lane)}

OldInit ==
  /\ lane = <<BlockedAux, Progress>>
  /\ progressServed = FALSE

OldHeadDrain ==
  /\ lane # <<>>
  /\ Head(lane) = Progress
  /\ lane' = Tail(lane)
  /\ progressServed' = TRUE

OldNext == OldHeadDrain

OldSpec ==
  /\ OldInit
  /\ [][OldNext]_vars
  /\ WF_vars(OldHeadDrain)

FirstProgressIndex ==
  CHOOSE index \in 1..Len(lane):
    /\ lane[index] = Progress
    /\ \A earlier \in 1..(index - 1): lane[earlier] # Progress

SequenceWithoutIndex(sequence, index) ==
  SubSeq(sequence, 1, index - 1)
    \o SubSeq(sequence, index + 1, Len(sequence))

IndexedInit == OldInit

IndexedDrainProgress ==
  /\ ProgressPresent
  /\ lane' = SequenceWithoutIndex(lane, FirstProgressIndex)
  /\ progressServed' = TRUE

IndexedNext == IndexedDrainProgress

IndexedSpec ==
  /\ IndexedInit
  /\ [][IndexedNext]_vars
  /\ WF_vars(IndexedDrainProgress)

ProgressOccurrenceService == ProgressPresent ~> progressServed

=============================================================================
