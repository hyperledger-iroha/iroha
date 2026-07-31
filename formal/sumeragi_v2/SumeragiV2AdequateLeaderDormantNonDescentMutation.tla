---- MODULE SumeragiV2AdequateLeaderDormantNonDescentMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Bounded mutation witness for Dormant adequate-leader predecessors.

The older lifecycle has immutable scheduler ordinal 1 and the already
selected target has ordinal 2.  A packetless Dormant lifecycle owns no fair
action and therefore cannot block the target.  When a retained producer
exists, it first emits the exact retry packet without claiming rank descent;
atomic admission then moves the same identity from the source-frozen Dormant
potential into the active predecessor set.

The episode rank is lexicographic:

  <<source-frozen Dormant potential not yet discovered,
    active predecessor debt>>

Consequently retry emission preserves the rank, atomic reactivation consumes
the outer coordinate even though active predecessor debt rises, and service
of the reactivated owner consumes the inner coordinate.  Omitting the
Dormant potential from the source cut makes reactivation an increase and is
the dedicated failing mutation.
***************************************************************************)

CONSTANTS
  RetainedRetryProducer,
  TreatPacketlessDormantAsActive,
  FreezeDormantPotential

ASSUME
  /\ RetainedRetryProducer \in BOOLEAN
  /\ TreatPacketlessDormantAsActive \in BOOLEAN
  /\ FreezeDormantPotential \in BOOLEAN

OlderOwner == "older-ordinal-1"
TargetOwner == "target-ordinal-2"
Owners == {OlderOwner, TargetOwner}

VARIABLES
  phase,
  packetPresent,
  dormantOwners,
  activeOwners,
  frozenPotential,
  knownPotential,
  targetServiced

vars ==
  <<phase, packetPresent, dormantOwners, activeOwners,
    frozenPotential, knownPotential, targetServiced>>

TypeInvariant ==
  /\ phase
       \in {"Dormant", "RetryReady", "Reactivated",
            "OlderServiced", "TargetServiced"}
  /\ packetPresent \in BOOLEAN
  /\ dormantOwners \subseteq {OlderOwner}
  /\ activeOwners \subseteq Owners
  /\ frozenPotential \subseteq {OlderOwner}
  /\ knownPotential \subseteq frozenPotential
  /\ targetServiced \in BOOLEAN

DormantOwnerHasActiveTurn ==
  \/ OlderOwner \in activeOwners
  \/ /\ TreatPacketlessDormantAsActive
     /\ OlderOwner \in dormantOwners
     /\ ~packetPresent

PacketlessDormantOwnsNoActiveTurn ==
  /\ phase = "Dormant"
  /\ ~RetainedRetryProducer
  => ~DormantOwnerHasActiveTurn

DormantPotentialDebt ==
  Cardinality(frozenPotential \ knownPotential)

ActivePredecessorDebt ==
  IF OlderOwner \in activeOwners THEN 1 ELSE 0

EpisodeRank ==
  <<DormantPotentialDebt, ActivePredecessorDebt>>

SourceEpisodeRank ==
  <<IF FreezeDormantPotential THEN 1 ELSE 0, 0>>

LexLess(left, right) ==
  \/ left[1] < right[1]
  \/ /\ left[1] = right[1]
     /\ left[2] < right[2]

ProducerEpisodeDoesNotClaimDescent ==
  phase = "RetryReady" => EpisodeRank = SourceEpisodeRank

ReactivationConsumesFrozenPotentialBudget ==
  (/\ RetainedRetryProducer
   /\ phase \in {"Reactivated", "OlderServiced", "TargetServiced"})
    => LexLess(EpisodeRank, SourceEpisodeRank)

KnownPotentialIsExactDiscovery ==
  knownPotential =
    frozenPotential \cap ({OlderOwner} \ dormantOwners)

Init ==
  /\ phase = "Dormant"
  /\ ~packetPresent
  /\ dormantOwners = {OlderOwner}
  /\ activeOwners = {TargetOwner}
  /\ frozenPotential =
       IF FreezeDormantPotential THEN {OlderOwner} ELSE {}
  /\ knownPotential = {}
  /\ ~targetServiced

EmitExactRetry ==
  /\ phase = "Dormant"
  /\ RetainedRetryProducer
  /\ ~packetPresent
  /\ phase' = "RetryReady"
  /\ packetPresent'
  /\ UNCHANGED
       <<dormantOwners, activeOwners, frozenPotential,
         knownPotential, targetServiced>>

ReactivateDormant ==
  /\ phase = "RetryReady"
  /\ packetPresent
  /\ OlderOwner \in dormantOwners
  /\ phase' = "Reactivated"
  /\ ~packetPresent'
  /\ dormantOwners' = dormantOwners \ {OlderOwner}
  /\ activeOwners' = activeOwners \cup {OlderOwner}
  /\ knownPotential' =
       knownPotential \cup ({OlderOwner} \cap frozenPotential)
  /\ UNCHANGED <<frozenPotential, targetServiced>>

ServiceOlder ==
  /\ phase = "Reactivated"
  /\ OlderOwner \in activeOwners
  /\ phase' = "OlderServiced"
  /\ activeOwners' = activeOwners \ {OlderOwner}
  /\ UNCHANGED
       <<packetPresent, dormantOwners, frozenPotential,
         knownPotential, targetServiced>>

ServiceTargetAfterOlder ==
  /\ phase = "OlderServiced"
  /\ TargetOwner \in activeOwners
  /\ phase' = "TargetServiced"
  /\ activeOwners' = activeOwners \ {TargetOwner}
  /\ targetServiced'
  /\ UNCHANGED
       <<packetPresent, dormantOwners, frozenPotential, knownPotential>>

ServiceTargetWithoutRetryOwner ==
  /\ phase = "Dormant"
  /\ ~RetainedRetryProducer
  /\ ~DormantOwnerHasActiveTurn
  /\ TargetOwner \in activeOwners
  /\ phase' = "TargetServiced"
  /\ activeOwners' = activeOwners \ {TargetOwner}
  /\ targetServiced'
  /\ UNCHANGED
       <<packetPresent, dormantOwners, frozenPotential, knownPotential>>

Next ==
  \/ EmitExactRetry
  \/ ReactivateDormant
  \/ ServiceOlder
  \/ ServiceTargetAfterOlder
  \/ ServiceTargetWithoutRetryOwner

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(EmitExactRetry)
  /\ WF_vars(ReactivateDormant)
  /\ WF_vars(ServiceOlder)
  /\ WF_vars(ServiceTargetAfterOlder)
  /\ WF_vars(ServiceTargetWithoutRetryOwner)

TargetEventuallyServiced ==
  <>targetServiced

=============================================================================
