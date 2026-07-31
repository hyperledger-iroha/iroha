---- MODULE SumeragiV2ExactIngressTicketPriorityMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation for the provisional exact-ticket versus Runtime ordering.

The ticket is already installed at the atomic transport/Serve-reservation cut
while the runner still names Runtime.  The repaired action jumps directly to
Ingress and leaves both the target capacity component and the frozen
pre-ticket producer count unchanged.  The mutation runs Runtime first; its
new Completion/Control producer occupies Serve capacity, making the outer
capacity component of the exact rank ascend before the target selector.
***************************************************************************)

CONSTANT PrioritizeProvisionalTarget

ASSUME PrioritizeProvisionalTarget \in BOOLEAN

VARIABLES
  phase,
  ticketOwned,
  targetCapacityDebt,
  runnerReach,
  frozenPreTicketOwners,
  laterRuntimeOwners,
  targetDrained

vars ==
  <<phase, ticketOwned, targetCapacityDebt, runnerReach,
    frozenPreTicketOwners, laterRuntimeOwners, targetDrained>>

ExactIngressRank == <<targetCapacityDebt, runnerReach>>

TypeInvariant ==
  /\ phase \in {"TicketAtRuntime", "AfterFirstTurn", "TargetDrained"}
  /\ ticketOwned \in BOOLEAN
  /\ targetCapacityDebt \in 0..1
  /\ runnerReach \in 0..2
  /\ frozenPreTicketOwners \in 0..1
  /\ laterRuntimeOwners \in 0..1
  /\ targetDrained \in BOOLEAN

ProvisionalTargetPrecedesRuntimeWork ==
  phase = "AfterFirstTurn"
    => /\ laterRuntimeOwners = 0
       /\ targetCapacityDebt = 0

FrozenPreTicketEpisodeDoesNotReplenish ==
  ticketOwned => frozenPreTicketOwners <= 1

TargetOnlyTurnStrictlyLowersExactRank ==
  phase = "AfterFirstTurn"
    => ExactIngressRank = <<0, 0>>

Init ==
  /\ phase = "TicketAtRuntime"
  /\ ticketOwned = TRUE
  /\ targetCapacityDebt = 0
  /\ runnerReach = 2
  /\ frozenPreTicketOwners = 1
  /\ laterRuntimeOwners = 0
  /\ targetDrained = FALSE

FirstRunnerTurn ==
  /\ phase = "TicketAtRuntime"
  /\ phase' = "AfterFirstTurn"
  /\ IF PrioritizeProvisionalTarget
     THEN /\ targetCapacityDebt' = 0
          /\ runnerReach' = 0
          /\ laterRuntimeOwners' = 0
     ELSE /\ targetCapacityDebt' = 1
          /\ runnerReach' = 1
          /\ laterRuntimeOwners' = 1
  /\ UNCHANGED <<ticketOwned, frozenPreTicketOwners, targetDrained>>

DrainExactTarget ==
  /\ phase = "AfterFirstTurn"
  /\ targetCapacityDebt = 0
  /\ runnerReach = 0
  /\ phase' = "TargetDrained"
  /\ ticketOwned' = FALSE
  /\ targetDrained' = TRUE
  /\ UNCHANGED <<targetCapacityDebt, runnerReach,
                 frozenPreTicketOwners, laterRuntimeOwners>>

Next ==
  \/ FirstRunnerTurn
  \/ DrainExactTarget

=============================================================================
