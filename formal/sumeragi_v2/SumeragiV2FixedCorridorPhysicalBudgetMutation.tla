---- MODULE SumeragiV2FixedCorridorPhysicalBudgetMutation ----
EXTENDS Integers, Naturals

(***************************************************************************
Finite arithmetic mutation for the fixed-corridor physical-service budget.

Completion and Normal own independent deferred queues even though both use
`DeferredNormalCapacity`; Progress owns a third queue.  A complete physical
window must therefore charge both normal-capacity lanes before applying the
three-way deferred-cursor and I/O-selector reset multipliers.

The mutation holds the independently derived producer-episode charge fixed,
then restores the former deferred/cursor accounting: it charges only one
normal-capacity lane and includes only one copy of each drain budget.  After
the deterministic worst-case load, that configured budget is smaller than the
concrete carrier debt.  This bounded pair is regression evidence for the
arithmetic seam only; it is not a temporal provider for the production
fixed-clock theorem.
***************************************************************************)

CONSTANTS
  MutationMode,
  ProducerEpisodeBudget,
  RuntimeCycleBudget,
  DeferredNormalCapacity,
  DeferredProgressCapacity,
  CompletionReserve,
  IoAuxCapacity,
  IoWorkCapacity

ASSUME
  /\ MutationMode \in {"Fixed", "OmitSecondNormalAndCursorCopies"}
  /\ ProducerEpisodeBudget \in Nat \ {0}
  /\ RuntimeCycleBudget \in Nat \ {0}
  /\ DeferredNormalCapacity \in Nat \ {0}
  /\ DeferredProgressCapacity \in Nat \ {0}
  /\ CompletionReserve \in Nat \ {0}
  /\ IoAuxCapacity \in Nat \ {0}
  /\ IoWorkCapacity \in Nat \ {0}

VARIABLES stage, physicalDebt

DeferredDrainBudget ==
  2 * DeferredNormalCapacity
    + DeferredProgressCapacity
    + CompletionReserve

HistoricalDeferredDrainBudget ==
  DeferredNormalCapacity
    + DeferredProgressCapacity
    + CompletionReserve

IoDrainBudget ==
  IoAuxCapacity + IoWorkCapacity + 1

CompletePhysicalWindowBudget ==
  ProducerEpisodeBudget
    + RuntimeCycleBudget
    + 4 * DeferredDrainBudget
    + 6 * IoDrainBudget

ConfiguredPhysicalWindowBudget ==
  IF MutationMode = "Fixed"
  THEN CompletePhysicalWindowBudget
  ELSE ProducerEpisodeBudget
         + RuntimeCycleBudget
         + HistoricalDeferredDrainBudget
         + IoDrainBudget

vars == <<stage, physicalDebt>>

TypeInvariant ==
  /\ stage \in {"Empty", "Loaded"}
  /\ physicalDebt \in Nat

Init ==
  /\ stage = "Empty"
  /\ physicalDebt = 0

LoadWorstCasePhysicalCarriers ==
  /\ stage = "Empty"
  /\ stage' = "Loaded"
  /\ physicalDebt' = CompletePhysicalWindowBudget

RemainLoaded ==
  /\ stage = "Loaded"
  /\ UNCHANGED vars

Next ==
  LoadWorstCasePhysicalCarriers
    \/ RemainLoaded

Spec ==
  Init /\ [][Next]_vars

PhysicalWindowBudgetCoversIndependentLanesAndCursorResets ==
  physicalDebt <= ConfiguredPhysicalWindowBudget

=============================================================================
