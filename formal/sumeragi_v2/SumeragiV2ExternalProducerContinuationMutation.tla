---- MODULE SumeragiV2ExternalProducerContinuationMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation kernel for external producer continuations.

A conditional-transport or volatile-body candidate can leave its final
scheduler carrier without reaching Decision or a lower semantic rank.  The
repair atomically installs one exact continuation at the departing
lifecycle's immutable address.  Class-specific fair service may materialize
that record only from an exact retained/returned carrier; the following turn
terminalizes it.  An exact retry coalesces with the terminal identity instead
of recreating the old stage.  The toggles below independently remove each
part of that contract.
***************************************************************************)

CONSTANTS
  ReserveConditionalDeparture,
  ReserveVolatileDeparture,
  EnforceExactCarrierWitness,
  CoalesceTerminalRetry,
  FairConditionalService,
  FairVolatileService

ASSUME
  /\ ReserveConditionalDeparture \in BOOLEAN
  /\ ReserveVolatileDeparture \in BOOLEAN
  /\ EnforceExactCarrierWitness \in BOOLEAN
  /\ CoalesceTerminalRetry \in BOOLEAN
  /\ FairConditionalService \in BOOLEAN
  /\ FairVolatileService \in BOOLEAN

LifecycleOrdinal == 7
LifecycleSlot == 2

ConditionalIdentity == "DeliverQC/context-0/view-3/subject-A"
VolatileIdentity == "FetchBody/context-0/view-3/subject-A"

NoRecord ==
  [identity |-> "None", sourceClass |-> "None", ordinal |-> 0,
   slot |-> 0, status |-> "None"]

Continuation(identity, sourceClass, status) ==
  [identity |-> identity, sourceClass |-> sourceClass,
   ordinal |-> LifecycleOrdinal, slot |-> LifecycleSlot,
   status |-> status]

ContinuationCarrier ==
  {NoRecord}
    \cup
  {Continuation(identity, sourceClass, status):
     identity \in {ConditionalIdentity, VolatileIdentity},
     sourceClass \in {"ConditionalTransport", "VolatileBody"},
     status \in {"Reserved", "Materialized", "Terminal"}}

VARIABLES
  phase,
  continuation,
  exactCarrier,
  materializedFromExactCarrier,
  conditionalResurrected,
  volatileResurrected

vars ==
  <<phase, continuation, exactCarrier, materializedFromExactCarrier,
    conditionalResurrected, volatileResurrected>>

TypeInvariant ==
  /\ phase \in
       {"ConditionalSource", "ConditionalReserved",
        "ConditionalMaterialized", "VolatileReserved",
        "VolatileMaterialized", "Done"}
  /\ continuation \in ContinuationCarrier
  /\ exactCarrier \in BOOLEAN
  /\ materializedFromExactCarrier \in BOOLEAN
  /\ conditionalResurrected \in BOOLEAN
  /\ volatileResurrected \in BOOLEAN

ConditionalDepartureInstallsExactContinuation ==
  phase = "ConditionalReserved"
    => continuation =
         Continuation(
           ConditionalIdentity, "ConditionalTransport", "Reserved")

VolatileDepartureInstallsExactContinuation ==
  phase = "VolatileReserved"
    => continuation =
         Continuation(VolatileIdentity, "VolatileBody", "Reserved")

ContinuationReusesLifecycleAddress ==
  continuation # NoRecord
    => /\ continuation.ordinal = LifecycleOrdinal
       /\ continuation.slot = LifecycleSlot

ExternalMaterializationRequiresExactCarrier ==
  continuation.status = "Materialized"
    => materializedFromExactCarrier

TerminalIdentityCannotResurrect ==
  /\ ~conditionalResurrected
  /\ ~volatileResurrected

Init ==
  /\ phase = "ConditionalSource"
  /\ continuation = NoRecord
  /\ exactCarrier = TRUE
  /\ materializedFromExactCarrier = FALSE
  /\ conditionalResurrected = FALSE
  /\ volatileResurrected = FALSE

DepartConditional ==
  /\ phase = "ConditionalSource"
  /\ phase' = "ConditionalReserved"
  /\ continuation' =
       IF ReserveConditionalDeparture
       THEN Continuation(
              ConditionalIdentity, "ConditionalTransport", "Reserved")
       ELSE NoRecord
  /\ exactCarrier' = TRUE
  /\ materializedFromExactCarrier' = FALSE
  /\ UNCHANGED <<conditionalResurrected, volatileResurrected>>

ServiceConditional ==
  /\ phase \in {"ConditionalReserved", "ConditionalMaterialized"}
  /\ continuation.identity = ConditionalIdentity
  /\ IF phase = "ConditionalReserved"
     THEN /\ phase' = "ConditionalMaterialized"
          /\ continuation' =
               Continuation(
                 ConditionalIdentity,
                 "ConditionalTransport", "Materialized")
          /\ materializedFromExactCarrier' =
               IF EnforceExactCarrierWitness THEN exactCarrier ELSE FALSE
          /\ UNCHANGED <<exactCarrier, conditionalResurrected>>
     ELSE /\ phase' = "VolatileReserved"
          /\ conditionalResurrected' = ~CoalesceTerminalRetry
          /\ continuation' =
               IF ReserveVolatileDeparture
               THEN Continuation(
                      VolatileIdentity, "VolatileBody", "Reserved")
               ELSE NoRecord
          /\ exactCarrier' = TRUE
          /\ materializedFromExactCarrier' = FALSE
  /\ UNCHANGED volatileResurrected

ServiceVolatile ==
  /\ phase \in {"VolatileReserved", "VolatileMaterialized"}
  /\ continuation.identity = VolatileIdentity
  /\ IF phase = "VolatileReserved"
     THEN /\ phase' = "VolatileMaterialized"
          /\ continuation' =
               Continuation(
                 VolatileIdentity, "VolatileBody", "Materialized")
          /\ materializedFromExactCarrier' =
               IF EnforceExactCarrierWitness THEN exactCarrier ELSE FALSE
          /\ UNCHANGED volatileResurrected
     ELSE /\ phase' = "Done"
          /\ continuation' =
               Continuation(
                 VolatileIdentity, "VolatileBody", "Terminal")
          /\ materializedFromExactCarrier' =
               materializedFromExactCarrier
          /\ volatileResurrected' = ~CoalesceTerminalRetry
  /\ UNCHANGED <<exactCarrier, conditionalResurrected>>

Next ==
  \/ DepartConditional
  \/ ServiceConditional
  \/ ServiceVolatile

ConditionalFairness ==
  IF FairConditionalService THEN WF_vars(ServiceConditional) ELSE TRUE

VolatileFairness ==
  IF FairVolatileService THEN WF_vars(ServiceVolatile) ELSE TRUE

Spec ==
  Init /\ [][Next]_vars /\ WF_vars(DepartConditional)
    /\ ConditionalFairness /\ VolatileFairness

ExternalContinuationsReachTerminal == <>(phase = "Done")

=============================================================================
