---- MODULE SumeragiV2BusyAliasRankMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded witness for the structurally aliased Proposal/Timeout WAL and signing
records.  A set value may occupy both logical owner lanes even though the
production intent guards make that state unreachable.

The retired IF rank records the aliased state as rank two and fails to fall
when the signing lane completes.  The weighted membership rank records the
same robustness state as three and falls to two.  The guarded production path
keeps the lanes disjoint and follows the reachable 2 -> 1 -> 0 descent.
***************************************************************************)

VARIABLES walOwner, signingOwner, intentDurable, phase

vars == <<walOwner, signingOwner, intentDurable, phase>>

OwnerBit(owner) == IF owner = TRUE THEN 1 ELSE 0

OldIfRank ==
  IF walOwner = TRUE THEN 2 ELSE IF signingOwner = TRUE THEN 1 ELSE 0

WeightedRank == 2 * OwnerBit(walOwner) + OwnerBit(signingOwner)

AliasInit ==
  /\ walOwner = TRUE
  /\ signingOwner = TRUE
  /\ intentDurable = TRUE
  /\ phase = "BeforeSign"

SignAliasedOwner ==
  /\ phase = "BeforeSign"
  /\ signingOwner = TRUE
  /\ intentDurable = TRUE
  /\ signingOwner' = FALSE
  /\ UNCHANGED <<walOwner, intentDurable>>
  /\ phase' = "AfterSign"

AliasSpec ==
  /\ AliasInit
  /\ [][SignAliasedOwner]_vars
  /\ WF_vars(SignAliasedOwner)

OldIfRankDropped == phase = "BeforeSign" \/ OldIfRank < 2

WeightedRankDropped == phase = "BeforeSign" \/ WeightedRank < 3

KernelInit ==
  /\ walOwner = TRUE
  /\ signingOwner = FALSE
  /\ intentDurable = FALSE
  /\ phase = "BeforePersist"

PersistWalOwner ==
  /\ phase = "BeforePersist"
  /\ walOwner = TRUE
  /\ intentDurable = FALSE
  /\ walOwner' = FALSE
  /\ signingOwner' = TRUE
  /\ intentDurable' = TRUE
  /\ phase' = "BeforeSign"

SignGuardedOwner ==
  /\ phase = "BeforeSign"
  /\ signingOwner = TRUE
  /\ intentDurable = TRUE
  /\ signingOwner' = FALSE
  /\ UNCHANGED <<walOwner, intentDurable>>
  /\ phase' = "Idle"

KernelNext == PersistWalOwner \/ SignGuardedOwner

KernelSpec ==
  /\ KernelInit
  /\ [][KernelNext]_vars
  /\ WF_vars(PersistWalOwner)
  /\ WF_vars(SignGuardedOwner)

KernelLaneGuards ==
  /\ (walOwner = TRUE => intentDurable = FALSE)
  /\ (signingOwner = TRUE => intentDurable = TRUE)

KernelRankExcludesAlias == WeightedRank \in 0..2

KernelEventuallyIdle == (phase # "Idle") ~> (phase = "Idle")

=============================================================================
