---- MODULE SumeragiV2FixedCorridorDeadlineMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite-state regression for the immutable fixed-corridor stage reservation.

Admission reserves the producer stage at clock 2 and its successor transport
stage at clock 4, strictly before the corridor deadline 5.  One same-rank
physical owner replacement is forced when the producer becomes due.  The
repaired transition retains the immutable admission reservation, so the
replacement cannot buy another clock episode and Decision occurs at 4.

The mutation restarts the producer deadline from replacement time.  That
non-progress event moves publication to 4 and delivery to 6, crossing the
frozen corridor boundary.  No packet deadline is clamped and no Tick action
is removed: the pair isolates exactly the replenishment defect repaired by
the admission ordinal/reservation/tombstone lifecycle.

This bounded pair checks the repaired transition boundary.  It is regression
evidence, not deductive proof of the full AsyncNetwork temporal theorem.
***************************************************************************)

CONSTANT DeadlineMode

DeadlineModes == {"RetainFrozenReservation", "RestartOnReplacement"}

VARIABLES stage, now, producerDeadline, corridorDeadline,
          packetDeadline, replacementPending, decided, lastTransition

vars ==
  <<stage, now, producerDeadline, corridorDeadline,
    packetDeadline, replacementPending, decided, lastTransition>>

Init ==
  /\ DeadlineMode \in DeadlineModes
  /\ stage = "Producer"
  /\ now = 0
  /\ producerDeadline = 2
  /\ corridorDeadline = 5
  /\ packetDeadline = 0
  /\ replacementPending
  /\ ~decided
  /\ lastTransition = "Init"

Tick ==
  /\ ~decided
  /\ \/ /\ stage = "Producer"
          /\ now < producerDeadline
     \/ /\ stage = "Packet"
          /\ now < packetDeadline
  /\ now' = now + 1
  /\ lastTransition' = "Tick"
  /\ UNCHANGED
       <<stage, producerDeadline, corridorDeadline,
         packetDeadline, replacementPending, decided>>

ReplaceProducerOwner ==
  /\ stage = "Producer"
  /\ replacementPending
  /\ now >= producerDeadline
  /\ replacementPending' = FALSE
  /\ producerDeadline' =
       IF DeadlineMode = "RetainFrozenReservation"
       THEN producerDeadline
       ELSE now + 2
  /\ lastTransition' = "ReplaceProducerOwner"
  /\ UNCHANGED
       <<stage, now, corridorDeadline, packetDeadline, decided>>

PublishSuccessor ==
  /\ stage = "Producer"
  /\ ~replacementPending
  /\ now >= producerDeadline
  /\ stage' = "Packet"
  /\ packetDeadline' = now + 2
  /\ lastTransition' = "PublishSuccessor"
  /\ UNCHANGED
       <<now, producerDeadline, corridorDeadline,
         replacementPending, decided>>

DeliverSuccessor ==
  /\ stage = "Packet"
  /\ now >= packetDeadline
  /\ stage' = "Decision"
  /\ decided' = TRUE
  /\ lastTransition' = "DeliverSuccessor"
  /\ UNCHANGED
       <<now, producerDeadline, corridorDeadline,
         packetDeadline, replacementPending>>

Next ==
  \/ Tick
  \/ ReplaceProducerOwner
  \/ PublishSuccessor
  \/ DeliverSuccessor

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Tick)
  /\ WF_vars(ReplaceProducerOwner)
  /\ WF_vars(PublishSuccessor)
  /\ WF_vars(DeliverSuccessor)

TypeInvariant ==
  /\ DeadlineMode \in DeadlineModes
  /\ stage \in {"Producer", "Packet", "Decision"}
  /\ now \in Nat
  /\ producerDeadline \in Nat
  /\ corridorDeadline \in Nat \ {0}
  /\ packetDeadline \in Nat
  /\ replacementPending \in BOOLEAN
  /\ decided \in BOOLEAN
  /\ lastTransition
       \in {"Init", "Tick", "ReplaceProducerOwner",
            "PublishSuccessor", "DeliverSuccessor"}

CorridorSource == stage = "Producer"

DecisionBeforeFrozenDeadline ==
  /\ decided
  /\ now < corridorDeadline

FixedReservationNeverMoves ==
  DeadlineMode = "RetainFrozenReservation"
    => producerDeadline = 2

FixedCorridorEventuallyDecidesBeforeDeadline ==
  CorridorSource ~> DecisionBeforeFrozenDeadline

====
