---- MODULE SumeragiV2ProductiveDeadlockMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded mutation for productive deadlock freedom.  In the old scenario a
bare scheduler tick remains enabled forever while no evidence, deadline debt,
service rank, or decision changes.  The repaired scenario exposes a finite
chain of concrete productive witnesses and then decides.
***************************************************************************)

CONSTANT ProductiveRepair

VARIABLES tick, evidence, deadlineDebt, serviceRank, decided

vars == <<tick, evidence, deadlineDebt, serviceRank, decided>>

TypeInvariant ==
  /\ tick \in BOOLEAN
  /\ evidence \in 0..1
  /\ deadlineDebt \in 0..1
  /\ serviceRank \in 0..1
  /\ decided \in BOOLEAN

Init ==
  /\ tick = FALSE
  /\ evidence = 0
  /\ deadlineDebt = IF ProductiveRepair THEN 1 ELSE 0
  /\ serviceRank = IF ProductiveRepair THEN 1 ELSE 0
  /\ decided = FALSE

BareSchedulerStep ==
  /\ ~ProductiveRepair
  /\ tick' = ~tick
  /\ UNCHANGED <<evidence, deadlineDebt, serviceRank, decided>>

DeadlineStep ==
  /\ ProductiveRepair
  /\ ~decided
  /\ deadlineDebt > 0
  /\ deadlineDebt' = deadlineDebt - 1
  /\ tick' = ~tick
  /\ UNCHANGED <<evidence, serviceRank, decided>>

EvidenceStep ==
  /\ ProductiveRepair
  /\ ~decided
  /\ deadlineDebt = 0
  /\ evidence = 0
  /\ evidence' = 1
  /\ tick' = ~tick
  /\ UNCHANGED <<deadlineDebt, serviceRank, decided>>

RankStep ==
  /\ ProductiveRepair
  /\ ~decided
  /\ evidence = 1
  /\ serviceRank > 0
  /\ serviceRank' = serviceRank - 1
  /\ tick' = ~tick
  /\ UNCHANGED <<evidence, deadlineDebt, decided>>

DecisionStep ==
  /\ ProductiveRepair
  /\ ~decided
  /\ evidence = 1
  /\ serviceRank = 0
  /\ decided' = TRUE
  /\ tick' = ~tick
  /\ UNCHANGED <<evidence, deadlineDebt, serviceRank>>

Next ==
  \/ BareSchedulerStep
  \/ DeadlineStep
  \/ EvidenceStep
  \/ RankStep
  \/ DecisionStep

ProductiveStep ==
  /\ Next
  /\ \/ evidence' > evidence
     \/ deadlineDebt' < deadlineDebt
     \/ serviceRank' < serviceRank
     \/ /\ ~decided
           /\ decided'

SchedulerActionEnabled == ENABLED Next

ProductiveActionEnabled == ENABLED ProductiveStep

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Next)

SchedulerOnlyDeadlockClaim == [](~decided => SchedulerActionEnabled)

ProductiveDeadlockClaim == [](~decided => ProductiveActionEnabled)

=============================================================================
