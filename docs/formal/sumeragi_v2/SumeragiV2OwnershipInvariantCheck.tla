---- MODULE SumeragiV2OwnershipInvariantCheck ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
Small exhaustive counterexample search for scheduler ownership.  The state
constraint holds the logical clock at its initial value while retaining every
non-clock AsyncNext branch, so TLC can enumerate all zero-clock ownership
transfers without conflating this finite check with deductive proof evidence.
***************************************************************************)

SingleValidatorRosters == <<<<0>>>>
SingleValidatorPowers == <<<<1>>>>

OwnershipBoundedSpec ==
  AsyncFiniteInit /\ [][AsyncNext]_AsyncAllVars

OwnershipInitialClock == asyncNow = 0

=============================================================================
