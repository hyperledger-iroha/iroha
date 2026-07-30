---- MODULE SumeragiV2AdequateLeaderPreAdmissionRouteMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Bounded mutation witness for the pre-admission subject-replacement route.

The retained PrepareQC owns one immutable route-neutral identity before the
receiver allocates a scheduler ordinal.  Retransmission may create a fresh
packet occurrence, but it must copy that identity exactly.  Atomic admission
then allocates ordinal 3, strictly after the already-admitted target ordinal 2,
so the frozen route is resolved without entering the predecessor cut.

The mutation changes only the retransmitted packet identity.  It therefore
cannot resolve the frozen route even though transport and admission both run.
No packet count is used as a rank, and no Dormant/no-packet action is made fair.
***************************************************************************)

CONSTANT PreserveExactRouteIdentity

ASSUME PreserveExactRouteIdentity \in BOOLEAN

ExactRoute == "prepare-qc-route"
DriftedRoute == "mutated-route"
NoRoute == "no-route"
TargetOrdinal == 2
SchedulerCeiling == 3

VARIABLES
  phase,
  retainedRoute,
  packetRoute,
  admittedRoute,
  admittedOrdinal

vars ==
  <<phase, retainedRoute, packetRoute, admittedRoute, admittedOrdinal>>

TypeInvariant ==
  /\ phase \in {"Retained", "Emitted", "Admitted"}
  /\ retainedRoute = ExactRoute
  /\ packetRoute \in {NoRoute, ExactRoute, DriftedRoute}
  /\ admittedRoute \in {NoRoute, ExactRoute, DriftedRoute}
  /\ admittedOrdinal \in 0..SchedulerCeiling

RetriedTransportRetainsFrozenIdentity ==
  phase \in {"Emitted", "Admitted"}
    => packetRoute = retainedRoute

ExactRouteResolved ==
  /\ admittedRoute = ExactRoute
  /\ TargetOrdinal < admittedOrdinal

FreshAdmissionFollowsFrozenSchedulerCeiling ==
  admittedRoute = ExactRoute
    => /\ admittedOrdinal = SchedulerCeiling
       /\ TargetOrdinal < admittedOrdinal

Init ==
  /\ phase = "Retained"
  /\ retainedRoute = ExactRoute
  /\ packetRoute = NoRoute
  /\ admittedRoute = NoRoute
  /\ admittedOrdinal = 0

EmitRetainedExactRetry ==
  /\ phase = "Retained"
  /\ phase' = "Emitted"
  /\ packetRoute' =
       IF PreserveExactRouteIdentity THEN retainedRoute ELSE DriftedRoute
  /\ UNCHANGED <<retainedRoute, admittedRoute, admittedOrdinal>>

AdmitEmittedPacket ==
  /\ phase = "Emitted"
  /\ phase' = "Admitted"
  /\ admittedRoute' = packetRoute
  /\ admittedOrdinal' =
       IF packetRoute = ExactRoute THEN SchedulerCeiling ELSE 0
  /\ UNCHANGED <<retainedRoute, packetRoute>>

Next ==
  \/ EmitRetainedExactRetry
  \/ AdmitEmittedPacket

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(EmitRetainedExactRetry)
  /\ WF_vars(AdmitEmittedPacket)

ExactRetainedRouteEventuallyResolves ==
  <>ExactRouteResolved

=============================================================================
