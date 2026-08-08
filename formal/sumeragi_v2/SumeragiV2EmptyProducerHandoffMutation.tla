---- MODULE SumeragiV2EmptyProducerHandoffMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation kernel for the empty-producer semantic handoff.

The last physical carrier may depart without immediately exposing a causal
successor.  The repair installs one dormant producer reservation at the same
atomic boundary, inheriting the lifecycle address and ordinal.  Same-height
restart retains that exact token.  It may become Materialized only when a
concrete downstream owner acknowledges it.  If the exact frozen handoff is
instead retired with no successor, the selected Reserved record terminalizes
directly and remains the process-local tombstone for that drained identity.
The mutation drops the atomic reservation and exposes the formerly unowned
producer residual before either resolution is possible.
***************************************************************************)

CONSTANT InstallDormantReservation

ASSUME InstallDormantReservation \in BOOLEAN

LifecycleOrdinal == 17
LifecycleSlot == 4
ProducerIdentity == "context-0/leader-1/view-5/subject-A/DeliverQC"

NoReservation ==
  [identity |-> "None", sourceClass |-> "None", ordinal |-> 0,
   slot |-> 0, status |-> "None"]

Reservation(status) ==
  [identity |-> ProducerIdentity,
   sourceClass |-> "ConditionalTransport",
   ordinal |-> LifecycleOrdinal,
   slot |-> LifecycleSlot,
   status |-> status]

ReservationSet ==
  {NoReservation, Reservation("Reserved"),
   Reservation("Materialized"), Reservation("Terminal")}

VARIABLES phase, physicalOwner, downstreamOwner, reservation
VARIABLE handoffRetired

vars ==
  <<phase, physicalOwner, downstreamOwner, reservation, handoffRetired>>

TypeInvariant ==
  /\ phase \in {"Physical", "Dormant", "Restarted",
                  "Acknowledged", "Done"}
  /\ physicalOwner \in BOOLEAN
  /\ downstreamOwner \in BOOLEAN
  /\ reservation \in ReservationSet
  /\ handoffRetired \in BOOLEAN

EmptyProducerDepartureNeverBecomesUnowned ==
  phase \in {"Dormant", "Restarted"}
    => \/ physicalOwner
       \/ downstreamOwner
       \/ reservation = Reservation("Reserved")

DormantReservationUsesInheritedLifecycle ==
  reservation # NoReservation
    => /\ reservation.identity = ProducerIdentity
       /\ reservation.ordinal = LifecycleOrdinal
       /\ reservation.slot = LifecycleSlot

MaterializationRequiresDownstreamOwner ==
  reservation.status = "Materialized" => downstreamOwner

TerminalReservationRequiresAcknowledgementOrRetirement ==
  reservation.status = "Terminal"
    => \/ downstreamOwner
       \/ handoffRetired

RestartPreservesExactDormantReservation ==
  phase = "Restarted" => reservation = Reservation("Reserved")

Init ==
  /\ phase = "Physical"
  /\ physicalOwner = TRUE
  /\ downstreamOwner = FALSE
  /\ reservation = NoReservation
  /\ handoffRetired = FALSE

DepartLastPhysicalOwner ==
  /\ phase = "Physical"
  /\ phase' = "Dormant"
  /\ physicalOwner' = FALSE
  /\ downstreamOwner' = FALSE
  /\ handoffRetired' = FALSE
  /\ reservation' =
       IF InstallDormantReservation
       THEN Reservation("Reserved")
       ELSE NoReservation

RestartDormantOwner ==
  /\ phase = "Dormant"
  /\ reservation = Reservation("Reserved")
  /\ phase' = "Restarted"
  /\ reservation' = Reservation("Reserved")
  /\ UNCHANGED <<physicalOwner, downstreamOwner, handoffRetired>>

AcknowledgeDownstreamOwner ==
  /\ phase \in {"Dormant", "Restarted"}
  /\ reservation = Reservation("Reserved")
  /\ phase' = "Acknowledged"
  /\ physicalOwner' = FALSE
  /\ downstreamOwner' = TRUE
  /\ handoffRetired' = FALSE
  /\ reservation' = Reservation("Materialized")

RetireExactEmptyHandoff ==
  /\ phase \in {"Dormant", "Restarted"}
  /\ reservation = Reservation("Reserved")
  /\ ~downstreamOwner
  /\ phase' = "Done"
  /\ physicalOwner' = FALSE
  /\ downstreamOwner' = FALSE
  /\ handoffRetired' = TRUE
  /\ reservation' = Reservation("Terminal")

RetireAcknowledgedReservation ==
  /\ phase = "Acknowledged"
  /\ downstreamOwner
  /\ phase' = "Done"
  /\ reservation' = Reservation("Terminal")
  /\ UNCHANGED <<physicalOwner, downstreamOwner, handoffRetired>>

Next ==
  \/ DepartLastPhysicalOwner
  \/ RestartDormantOwner
  \/ AcknowledgeDownstreamOwner
  \/ RetireExactEmptyHandoff
  \/ RetireAcknowledgedReservation

=============================================================================
