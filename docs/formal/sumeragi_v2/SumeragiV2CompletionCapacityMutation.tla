---- MODULE SumeragiV2CompletionCapacityMutation ----
EXTENDS Naturals

(***************************************************************************
Small adversarial mutation for completion-capacity conflation.

`runtimeOwned` is a stale completion already accepted by the serialized
runtime and moved behind the reducer's Busy fence.  `requiredCausal` is the
exact persistence completion which clears Busy.  The retired guard counted
both runtime and outstanding ownership against the executor work capacity;
with capacity one, required admission was permanently disabled.  The repaired
guard counts only outstanding producer work.  Weak fairness must therefore
admit the required completion even while the stale runtime owner remains.
***************************************************************************)

VARIABLES runtimeOwned, requiredCausal, requiredOutstanding, tickParity

vars ==
  <<runtimeOwned, requiredCausal, requiredOutstanding, tickParity>>

WorkCapacity == 1

OutstandingWorkCount == IF requiredOutstanding THEN 1 ELSE 0

TotalCompletionLoad ==
  OutstandingWorkCount + (IF runtimeOwned THEN 1 ELSE 0)

Init ==
  /\ runtimeOwned = TRUE
  /\ requiredCausal = TRUE
  /\ requiredOutstanding = FALSE
  /\ tickParity = FALSE

AdmitWithConflatedCapacity ==
  /\ requiredCausal
  /\ TotalCompletionLoad < WorkCapacity
  /\ requiredCausal' = FALSE
  /\ requiredOutstanding' = TRUE
  /\ UNCHANGED <<runtimeOwned, tickParity>>

AdmitWithSeparatedCapacity ==
  /\ requiredCausal
  /\ OutstandingWorkCount < WorkCapacity
  /\ requiredCausal' = FALSE
  /\ requiredOutstanding' = TRUE
  /\ UNCHANGED <<runtimeOwned, tickParity>>

Tick ==
  /\ tickParity' = ~tickParity
  /\ UNCHANGED <<runtimeOwned, requiredCausal, requiredOutstanding>>

ConflatedNext == AdmitWithConflatedCapacity \/ Tick

SeparatedNext == AdmitWithSeparatedCapacity \/ Tick

ConflatedSpec ==
  /\ Init
  /\ [][ConflatedNext]_vars
  /\ WF_vars(AdmitWithConflatedCapacity)
  /\ WF_vars(Tick)

SeparatedSpec ==
  /\ Init
  /\ [][SeparatedNext]_vars
  /\ WF_vars(AdmitWithSeparatedCapacity)
  /\ WF_vars(Tick)

RequiredCompletionEventuallyOwnsWork ==
  requiredCausal ~> requiredOutstanding

=============================================================================
