---- MODULE SumeragiV2IngressCapacityMutation ----
EXTENDS FiniteSets, Naturals, Sequences

(***************************************************************************
Adversarial witness for the lane-length clause in the ingress capacity
invariant.

IngressDepth deliberately counts at most Capacity entries from each lane, as
the production model does by ranging pair indices over 1..Capacity.  Without
an explicit per-lane bound, an unreachable overlong lane can therefore satisfy
the old aggregate invariant.  Removing its only progress item exposes a
protected progress slot without reducing the counted depth and violates that
invariant.  The repaired initial state respects the lane bound; the same
removal then decreases counted depth and preserves the complete invariant.
***************************************************************************)

VARIABLE lane

vars == <<lane>>

Capacity == 2
Progress == "Progress"
Auxiliary == "Auxiliary"

LaneHasProgress == Progress \in {lane[index]: index \in 1..Len(lane)}

IngressDepth ==
  Cardinality({index \in 1..Capacity: index <= Len(lane)})

ProtectedSlotCount ==
  (IF Len(lane) = 0 \/ ~LaneHasProgress THEN 1 ELSE 0)
    + (IF Len(lane) = 0
         \/ /\ Len(lane) = 1
               /\ LaneHasProgress
       THEN 1
       ELSE 0)

OldCapacityInvariant ==
  /\ IngressDepth <= Capacity
  /\ IngressDepth + ProtectedSlotCount <= Capacity

NewCapacityInvariant ==
  /\ Len(lane) <= Capacity
  /\ OldCapacityInvariant

RemoveProgressHead ==
  /\ lane # <<>>
  /\ Head(lane) = Progress
  /\ lane' = Tail(lane)

OldInit == lane = <<Progress, Auxiliary, Auxiliary>>

OldSpec ==
  /\ OldInit
  /\ [][RemoveProgressHead]_vars

BoundedInit == lane = <<Progress, Auxiliary>>

BoundedSpec ==
  /\ BoundedInit
  /\ [][RemoveProgressHead]_vars

=============================================================================
