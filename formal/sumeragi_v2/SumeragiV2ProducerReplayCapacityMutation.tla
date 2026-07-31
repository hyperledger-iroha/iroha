---- MODULE SumeragiV2ProducerReplayCapacityMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite regression for the restart-dormant Local producer reservation.

The serialized runtime has three physical command slots in this quotient.
Two ordinary commands are already present when one exact Local continuation
is restored as a latent reservation.  The repaired admission rule charges the
latent owner before admitting later Causal, Control, or Completion churn.
The exact retry then atomically replaces that latent charge with one queued
carrier, and service installs a stable tombstone.  Repeated post-drain retries
only toggle a diagnostic bit; they cannot recreate the old stage.

The first mutant ignores the latent charge and fills the final physical slot.
The exact replay is then disabled, so rotating producer churn can form a fair
stuttering lasso.  The second mutant appends the exact carrier without
consuming its latent charge.  Both defects violate the same physical-plus-
latent capacity invariant for their intended reason.
***************************************************************************)

Capacity == 3
TargetStatuses == {"Dormant", "Queued", "Tombstone"}
ChurnClasses == {"Causal", "Control", "Completion"}
TransitionNames ==
  {"Initial", "RotateChurn", "RetryCoalesced", "OrdinaryEnqueue",
   "CapacityBlindEnqueue", "ExactReplay", "NonAtomicReplay",
   "ServiceTarget"}

VARIABLES
  queueDepth,
  targetStatus,
  nextChurnClass,
  retryParity,
  lastTransition

MutationVars ==
  <<queueDepth, targetStatus, nextChurnClass, retryParity, lastTransition>>

TargetReservationCharge ==
  IF targetStatus = "Dormant" THEN 1 ELSE 0

ReplayRank ==
  CASE targetStatus = "Dormant" -> 2
    [] targetStatus = "Queued" -> 1
    [] OTHER -> 0

TypeInvariant ==
  /\ queueDepth \in 0..Capacity
  /\ targetStatus \in TargetStatuses
  /\ nextChurnClass \in ChurnClasses
  /\ retryParity \in BOOLEAN
  /\ lastTransition \in TransitionNames

PhysicalAndLatentChargesFitConfiguredCapacity ==
  queueDepth + TargetReservationCharge <= Capacity

QueuedTargetOwnsOnePhysicalCarrier ==
  targetStatus = "Queued" => queueDepth > 0

TombstoneCannotResurrect ==
  targetStatus = "Tombstone"
    => lastTransition \notin {"ExactReplay", "NonAtomicReplay"}

Init ==
  /\ queueDepth = Capacity - 1
  /\ targetStatus = "Dormant"
  /\ nextChurnClass \in ChurnClasses
  /\ retryParity = FALSE
  /\ lastTransition = "Initial"

RotateChurnClass ==
  /\ nextChurnClass' =
       CASE nextChurnClass = "Causal" -> "Control"
         [] nextChurnClass = "Control" -> "Completion"
         [] OTHER -> "Causal"
  /\ lastTransition' = "RotateChurn"
  /\ UNCHANGED <<queueDepth, targetStatus, retryParity>>

RetryCoalesces ==
  /\ targetStatus \in {"Dormant", "Tombstone"}
  /\ retryParity' = ~retryParity
  /\ lastTransition' = "RetryCoalesced"
  /\ UNCHANGED <<queueDepth, targetStatus, nextChurnClass>>

OrdinaryEnqueuePreservesReplayReservation ==
  /\ queueDepth + 1 + TargetReservationCharge <= Capacity
  /\ queueDepth' = queueDepth + 1
  /\ lastTransition' = "OrdinaryEnqueue"
  /\ UNCHANGED <<targetStatus, nextChurnClass, retryParity>>

CapacityBlindOrdinaryEnqueue ==
  /\ queueDepth < Capacity
  /\ queueDepth' = queueDepth + 1
  /\ lastTransition' = "CapacityBlindEnqueue"
  /\ UNCHANGED <<targetStatus, nextChurnClass, retryParity>>

ExactReplayAtomicallyConsumesReservation ==
  /\ targetStatus = "Dormant"
  /\ queueDepth < Capacity
  /\ queueDepth' = queueDepth + 1
  /\ targetStatus' = "Queued"
  /\ lastTransition' = "ExactReplay"
  /\ UNCHANGED <<nextChurnClass, retryParity>>

ReplayWithoutConsumingReservation ==
  /\ targetStatus = "Dormant"
  /\ queueDepth < Capacity
  /\ queueDepth' = queueDepth + 1
  /\ targetStatus' = "Dormant"
  /\ lastTransition' = "NonAtomicReplay"
  /\ UNCHANGED <<nextChurnClass, retryParity>>

ServiceExactTarget ==
  /\ targetStatus = "Queued"
  /\ queueDepth > 0
  /\ queueDepth' = queueDepth - 1
  /\ targetStatus' = "Tombstone"
  /\ lastTransition' = "ServiceTarget"
  /\ UNCHANGED <<nextChurnClass, retryParity>>

FixedRunner ==
  \/ ExactReplayAtomicallyConsumesReservation
  \/ ServiceExactTarget

FixedNext ==
  \/ RotateChurnClass
  \/ RetryCoalesces
  \/ OrdinaryEnqueuePreservesReplayReservation
  \/ FixedRunner

CapacityBlindNext ==
  \/ RotateChurnClass
  \/ RetryCoalesces
  \/ CapacityBlindOrdinaryEnqueue
  \/ FixedRunner

NonAtomicReplayNext ==
  \/ RotateChurnClass
  \/ RetryCoalesces
  \/ OrdinaryEnqueuePreservesReplayReservation
  \/ ReplayWithoutConsumingReservation

FixedSpec ==
  /\ Init
  /\ [][FixedNext]_MutationVars
  /\ WF_MutationVars(FixedRunner)

CapacityBlindSpec ==
  /\ Init
  /\ [][CapacityBlindNext]_MutationVars
  /\ WF_MutationVars(FixedRunner)

NonAtomicReplaySpec ==
  /\ Init
  /\ [][NonAtomicReplayNext]_MutationVars

EventuallyExactTargetIsTombstoned ==
  <>(targetStatus = "Tombstone")

=============================================================================
