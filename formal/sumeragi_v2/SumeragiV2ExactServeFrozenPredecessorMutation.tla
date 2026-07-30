---- MODULE SumeragiV2ExactServeFrozenPredecessorMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation for an admitted exact request at full effective Serve
capacity.

One earlier physical Serve owner and the target's off-queue reservation fill
the effective capacity.  A finite ordinary Completion prefix is also frozen
ahead of the target, but it is not a second Serve slot.  Later Local, Causal,
Control, and Completion work must remain behind the admitted ticket.  After
the frozen ordinary prefix and earlier Serve owner acknowledge, the target
materializes into exactly the reserved slot.
***************************************************************************)

CONSTANT FenceLaterWork

ASSUME FenceLaterWork \in BOOLEAN

ServeCapacity == 2

VARIABLES
  phase,
  targetReserved,
  targetQueued,
  earlierServeOwners,
  ordinaryCompletionPrefix,
  laterLocalOwners,
  laterCausalOwners,
  laterControlOwners,
  laterCompletionOwners,
  frozenPredecessorDebt

vars ==
  <<phase, targetReserved, targetQueued, earlierServeOwners,
    ordinaryCompletionPrefix, laterLocalOwners, laterCausalOwners,
    laterControlOwners, laterCompletionOwners,
    frozenPredecessorDebt>>

LaterOwnerCount ==
  laterLocalOwners + laterCausalOwners
    + laterControlOwners + laterCompletionOwners

EffectiveServeOccupancy ==
  earlierServeOwners + LaterOwnerCount
    + (IF targetReserved \/ targetQueued THEN 1 ELSE 0)

ExactIngressRank ==
  <<frozenPredecessorDebt,
    IF targetQueued THEN 0 ELSE 1>>

TypeInvariant ==
  /\ phase \in
       {"Full", "OrdinaryAcked", "ServeAcked", "Ready", "Materialized"}
  /\ targetReserved \in BOOLEAN
  /\ targetQueued \in BOOLEAN
  /\ earlierServeOwners \in 0..1
  /\ ordinaryCompletionPrefix \in 0..1
  /\ laterLocalOwners \in 0..1
  /\ laterCausalOwners \in 0..1
  /\ laterControlOwners \in 0..1
  /\ laterCompletionOwners \in 0..1
  /\ frozenPredecessorDebt \in 0..2

ReservedServeCapacityCannotBeStolen ==
  EffectiveServeOccupancy <= ServeCapacity

LaterWorkCannotPrecedeAdmittedTarget ==
  targetReserved /\ ~targetQueued => LaterOwnerCount = 0

FrozenPredecessorDebtIsExact ==
  frozenPredecessorDebt =
    earlierServeOwners + ordinaryCompletionPrefix

MaterializationUsesReservedSlot ==
  phase = "Materialized"
    => /\ targetQueued
       /\ ~targetReserved
       /\ EffectiveServeOccupancy = 1
       /\ ExactIngressRank = <<0, 0>>

Init ==
  /\ phase = "Full"
  /\ targetReserved
  /\ ~targetQueued
  /\ earlierServeOwners = 1
  /\ ordinaryCompletionPrefix = 1
  /\ laterLocalOwners = 0
  /\ laterCausalOwners = 0
  /\ laterControlOwners = 0
  /\ laterCompletionOwners = 0
  /\ frozenPredecessorDebt = 2

AttemptLaterLocal ==
  /\ targetReserved
  /\ ~targetQueued
  /\ laterLocalOwners' =
       IF FenceLaterWork THEN laterLocalOwners ELSE 1
  /\ UNCHANGED
       <<phase, targetReserved, targetQueued, earlierServeOwners,
         ordinaryCompletionPrefix, laterCausalOwners, laterControlOwners,
         laterCompletionOwners, frozenPredecessorDebt>>

AttemptLaterCausal ==
  /\ targetReserved
  /\ ~targetQueued
  /\ laterCausalOwners' =
       IF FenceLaterWork THEN laterCausalOwners ELSE 1
  /\ UNCHANGED
       <<phase, targetReserved, targetQueued, earlierServeOwners,
         ordinaryCompletionPrefix, laterLocalOwners, laterControlOwners,
         laterCompletionOwners, frozenPredecessorDebt>>

AttemptLaterControl ==
  /\ targetReserved
  /\ ~targetQueued
  /\ laterControlOwners' =
       IF FenceLaterWork THEN laterControlOwners ELSE 1
  /\ UNCHANGED
       <<phase, targetReserved, targetQueued, earlierServeOwners,
         ordinaryCompletionPrefix, laterLocalOwners, laterCausalOwners,
         laterCompletionOwners, frozenPredecessorDebt>>

AttemptLaterCompletion ==
  /\ targetReserved
  /\ ~targetQueued
  /\ laterCompletionOwners' =
       IF FenceLaterWork THEN laterCompletionOwners ELSE 1
  /\ UNCHANGED
       <<phase, targetReserved, targetQueued, earlierServeOwners,
         ordinaryCompletionPrefix, laterLocalOwners, laterCausalOwners,
         laterControlOwners, frozenPredecessorDebt>>

AcknowledgeOrdinaryPrefix ==
  /\ ordinaryCompletionPrefix = 1
  /\ ordinaryCompletionPrefix' = 0
  /\ frozenPredecessorDebt' = frozenPredecessorDebt - 1
  /\ phase' =
       IF earlierServeOwners = 0 THEN "Ready" ELSE "OrdinaryAcked"
  /\ UNCHANGED
       <<targetReserved, targetQueued, earlierServeOwners,
         laterLocalOwners, laterCausalOwners, laterControlOwners,
         laterCompletionOwners>>

AcknowledgeEarlierServe ==
  /\ earlierServeOwners = 1
  /\ earlierServeOwners' = 0
  /\ frozenPredecessorDebt' = frozenPredecessorDebt - 1
  /\ phase' =
       IF ordinaryCompletionPrefix = 0 THEN "Ready" ELSE "ServeAcked"
  /\ UNCHANGED
       <<targetReserved, targetQueued, ordinaryCompletionPrefix,
         laterLocalOwners, laterCausalOwners, laterControlOwners,
         laterCompletionOwners>>

MaterializeExactTarget ==
  /\ targetReserved
  /\ ~targetQueued
  /\ frozenPredecessorDebt = 0
  /\ LaterOwnerCount = 0
  /\ phase' = "Materialized"
  /\ ~targetReserved'
  /\ targetQueued'
  /\ UNCHANGED
       <<earlierServeOwners, ordinaryCompletionPrefix,
         laterLocalOwners, laterCausalOwners, laterControlOwners,
         laterCompletionOwners, frozenPredecessorDebt>>

Next ==
  \/ AttemptLaterLocal
  \/ AttemptLaterCausal
  \/ AttemptLaterControl
  \/ AttemptLaterCompletion
  \/ AcknowledgeOrdinaryPrefix
  \/ AcknowledgeEarlierServe
  \/ MaterializeExactTarget

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AcknowledgeOrdinaryPrefix)
  /\ WF_vars(AcknowledgeEarlierServe)
  /\ WF_vars(MaterializeExactTarget)

TargetEventuallyMaterializes ==
  targetReserved ~> targetQueued

=============================================================================
