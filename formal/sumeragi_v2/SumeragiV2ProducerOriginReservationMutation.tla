---- MODULE SumeragiV2ProducerOriginReservationMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation kernel for the bounded producer-origin reservation.

The scheduled corridor-entry root and every causal successor project one
existing lifecycle admission.  They do not allocate a parallel producer
record or mint another ordinal.  If the last scheduled stage drains, its
stage-exact continuation takes over the same slot and ordinal atomically; an
exact retry coalesces with that owner.  The three toggles remove those
contracts independently.
***************************************************************************)

CONSTANTS
  ProjectAdmissionAtOrigin,
  ReuseAdmissionOrdinalAtDeparture,
  CoalesceExactOriginRetry

ASSUME
  /\ ProjectAdmissionAtOrigin \in BOOLEAN
  /\ ReuseAdmissionOrdinalAtDeparture \in BOOLEAN
  /\ CoalesceExactOriginRetry \in BOOLEAN

Origin == "leader-1/context-0/view-4/subject-A"
RootIdentity == "AssembleBody/leader-1/context-0/view-4/subject-A"
SuccessorIdentity == "BeginProposal/leader-1/context-0/view-4/subject-A"
AdmissionOrdinal == 11
LifecycleSlot == 3

NoReservation ==
  [origin |-> "None", ordinal |-> 0, slot |-> 0]

Reservation ==
  [origin |-> Origin, ordinal |-> AdmissionOrdinal,
   slot |-> LifecycleSlot]

NoContinuation ==
  [identity |-> "None", origin |-> "None", ordinal |-> 0,
   slot |-> 0, status |-> "None"]

Continuation(ordinal) ==
  [identity |-> SuccessorIdentity, origin |-> Origin,
   ordinal |-> ordinal, slot |-> LifecycleSlot,
   status |-> "Reserved"]

VARIABLES
  phase,
  scheduledIdentity,
  lifecycleReservation,
  continuation,
  continuationCount,
  nextOrdinal

vars ==
  <<phase, scheduledIdentity, lifecycleReservation,
    continuation, continuationCount, nextOrdinal>>

TypeInvariant ==
  /\ phase \in {"Empty", "Admitted", "Successor", "Continuation", "Retried"}
  /\ scheduledIdentity \in {"None", RootIdentity, SuccessorIdentity}
  /\ lifecycleReservation \in {NoReservation, Reservation}
  /\ continuation \in
       {NoContinuation, Continuation(AdmissionOrdinal),
        Continuation(AdmissionOrdinal + 1)}
  /\ continuationCount \in 0..2
  /\ nextOrdinal \in {AdmissionOrdinal, AdmissionOrdinal + 1}

ScheduledOriginHasBoundedReservation ==
  phase \in {"Admitted", "Successor"}
    => /\ lifecycleReservation = Reservation
       /\ lifecycleReservation.origin = Origin
       /\ lifecycleReservation.ordinal = AdmissionOrdinal
       /\ lifecycleReservation.slot = LifecycleSlot

CausalSuccessorReusesOriginReservation ==
  phase = "Successor"
    => /\ scheduledIdentity = SuccessorIdentity
       /\ lifecycleReservation = Reservation
       /\ nextOrdinal = AdmissionOrdinal + 1

DepartureContinuationReusesAdmissionOrdinal ==
  phase \in {"Continuation", "Retried"}
    => /\ continuation.origin = Origin
       /\ continuation.ordinal = AdmissionOrdinal
       /\ continuation.slot = LifecycleSlot

ExactOriginRetryCoalesces ==
  phase = "Retried" => continuationCount = 1

Init ==
  /\ phase = "Empty"
  /\ scheduledIdentity = "None"
  /\ lifecycleReservation = NoReservation
  /\ continuation = NoContinuation
  /\ continuationCount = 0
  /\ nextOrdinal = AdmissionOrdinal

AdmitCorridorRoot ==
  /\ phase = "Empty"
  /\ phase' = "Admitted"
  /\ scheduledIdentity' = RootIdentity
  /\ lifecycleReservation' =
       IF ProjectAdmissionAtOrigin THEN Reservation ELSE NoReservation
  /\ continuation' = NoContinuation
  /\ continuationCount' = 0
  /\ nextOrdinal' = AdmissionOrdinal + 1

TransferToCausalSuccessor ==
  /\ phase = "Admitted"
  /\ phase' = "Successor"
  /\ scheduledIdentity' = SuccessorIdentity
  /\ UNCHANGED
       <<lifecycleReservation, continuation, continuationCount, nextOrdinal>>

DrainToStageContinuation ==
  /\ phase = "Successor"
  /\ phase' = "Continuation"
  /\ scheduledIdentity' = "None"
  /\ continuation' =
       Continuation(
         IF ReuseAdmissionOrdinalAtDeparture
         THEN AdmissionOrdinal
         ELSE nextOrdinal)
  /\ continuationCount' = 1
  /\ UNCHANGED <<lifecycleReservation, nextOrdinal>>

RetryExactOrigin ==
  /\ phase = "Continuation"
  /\ phase' = "Retried"
  /\ continuationCount' =
       IF CoalesceExactOriginRetry THEN 1 ELSE 2
  /\ UNCHANGED
       <<scheduledIdentity, lifecycleReservation, continuation, nextOrdinal>>

Next ==
  \/ AdmitCorridorRoot
  \/ TransferToCausalSuccessor
  \/ DrainToStageContinuation
  \/ RetryExactOrigin

=============================================================================
