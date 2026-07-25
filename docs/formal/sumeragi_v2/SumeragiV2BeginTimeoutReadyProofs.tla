---- MODULE SumeragiV2BeginTimeoutReadyProofs ----
EXTENDS SumeragiV2AsyncNetwork, TLAPS

(***************************************************************************
The serialized scheduler uses BeginTimeoutEnabled while the reducer executes
BeginTimeout.  This module proves that the shared pure readiness kernel is
exactly the enabledness set of the fully specified Core action; it is kept
separate from the large liveness module so downstream strict proofs never
have to normalize an embedded ENABLED expression during import.
***************************************************************************)

THEOREM BeginTimeoutReadyExactlyCharacterizesEnabledAction ==
  \A node \in ValidatorIds:
    BeginTimeoutReady(node) <=> ENABLED BeginTimeout(node)
BY ExpandENABLED, Isa
   DEF BeginTimeoutReady, BeginTimeout, TimeoutRequestFor, vars

THEOREM SchedulerTimeoutGuardExactlyMatchesCoreReadiness ==
  \A node \in ValidatorIds:
    BeginTimeoutEnabled(node) <=> BeginTimeoutReady(node)
BY DEF BeginTimeoutEnabled

=============================================================================
