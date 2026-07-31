---- MODULE SumeragiV2AsyncInstallRunnerContinuationProofs ----
EXTENDS SumeragiV2AsyncInstallRunnerProofs

(***************************************************************************
The asynchronous theorem is deliberately one-height: every reachable state
keeps the caller-supplied context and height fixed.  Per-node rollover and
historical service are represented by `NodeHasApplication` and
`RunHistoricalServer`, not by the reconfiguration harness's global barrier.
***************************************************************************)

AsyncFrozenContextAt(initialContext) ==
  /\ context = initialContext
  /\ height = initialContext.height

THEOREM AsyncInitEstablishesFrozenContext ==
  \A initialContext:
    AsyncInitAt(initialContext) => AsyncFrozenContextAt(initialContext)
BY SMT DEF AsyncInitAt, AsyncBaseInitAt, InitAt,
           AsyncFrozenContextAt

THEOREM AsyncNextPreservesFrozenContext ==
  \A initialContext:
    AsyncFrozenContextAt(initialContext)
      /\ [AsyncNext]_AsyncAllVars
      => AsyncFrozenContextAt(initialContext)'
BY Isa DEF AsyncFrozenContextAt, AsyncNext, AsyncAllVars, vars,
           AsyncSchedulerVars

THEOREM AsyncSpecAlwaysKeepsFrozenContext ==
  \A initialContext:
    AsyncSpecAt(initialContext) => []AsyncFrozenContextAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []AsyncFrozenContextAt(initialContext)
    <2>1. AsyncInitAt(initialContext)
            => AsyncFrozenContextAt(initialContext)
      BY AsyncInitEstablishesFrozenContext
    <2>2. AsyncFrozenContextAt(initialContext)
            /\ [AsyncNext]_AsyncAllVars
            => AsyncFrozenContextAt(initialContext)'
      BY AsyncNextPreservesFrozenContext
    <2> QED BY <2>1, <2>2, PTL DEF AsyncSpecAt
  <1> QED BY <1>1

=============================================================================
