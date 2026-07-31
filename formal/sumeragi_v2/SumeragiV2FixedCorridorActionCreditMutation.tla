---- MODULE SumeragiV2FixedCorridorActionCreditMutation ----
EXTENDS Integers, Naturals

(***************************************************************************
Bounded mutation for cumulative action credit across exact causal handoffs.

The parent is already at its final reducer-dispatch action.  Its debt is
therefore one plus the exact successor-tail credit.  The repaired transition
transfers only that reserved tail to the exact child batch.  The mutation
instead assigns a fresh per-child window after dispatch, reproducing the
cross-child recharge which a lexicographic per-candidate rank cannot exclude.

BeginTimeout covers the longest single-child chain.  ValidateBody covers the
three-child Normal/Completion/Completion batch.  The complete production
CommandSuccessors recurrence is checked separately by the aggregate source
checker; this finite pair is mutation evidence, not deductive proof.
***************************************************************************)

CONSTANT MutationMode

VARIABLES stage, actionDebt

BeginTimeoutParentDebt == 69
BeginTimeoutExactChildDebt == 68
ValidateBodyParentDebt == 27
ValidateBodyExactChildBatchDebt == 26

vars == <<stage, actionDebt>>

TypeInvariant ==
  /\ MutationMode \in {"Cumulative", "PerChildRecharge"}
  /\ stage
       \in {"BeginTimeoutParent", "PersistTimeoutChild",
            "ValidateBodyParent", "ValidateBodyChildren"}
  /\ actionDebt \in Nat

Init ==
  \/ /\ stage = "BeginTimeoutParent"
     /\ actionDebt = BeginTimeoutParentDebt
  \/ /\ stage = "ValidateBodyParent"
     /\ actionDebt = ValidateBodyParentDebt

DispatchBeginTimeout ==
  /\ stage = "BeginTimeoutParent"
  /\ stage' = "PersistTimeoutChild"
  /\ actionDebt' =
       IF MutationMode = "Cumulative"
       THEN BeginTimeoutExactChildDebt
       ELSE BeginTimeoutParentDebt

DispatchValidateBody ==
  /\ stage = "ValidateBodyParent"
  /\ stage' = "ValidateBodyChildren"
  /\ actionDebt' =
       IF MutationMode = "Cumulative"
       THEN ValidateBodyExactChildBatchDebt
       ELSE ValidateBodyParentDebt

RemainAtChildren ==
  /\ stage \in {"PersistTimeoutChild", "ValidateBodyChildren"}
  /\ UNCHANGED vars

Next ==
  DispatchBeginTimeout
    \/ DispatchValidateBody
    \/ RemainAtChildren

Spec ==
  Init /\ [][Next]_vars

ExactSuccessorHandoffStrictlyConsumesCumulativeActionDebt ==
  /\ (stage = "PersistTimeoutChild"
        => actionDebt < BeginTimeoutParentDebt)
  /\ (stage = "ValidateBodyChildren"
        => actionDebt < ValidateBodyParentDebt)

=============================================================================
