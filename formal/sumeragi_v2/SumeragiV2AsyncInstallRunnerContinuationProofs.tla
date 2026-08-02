---- MODULE SumeragiV2AsyncInstallRunnerContinuationProofs ----
EXTENDS SumeragiV2AsyncInstallRunnerProofs

(***************************************************************************
The runner theorem composes only concrete phase actions.  In particular, the
ordinary and exact-Serve-predecessor serialized leaves follow the same exact
command and transport updates, while the target-only leaf is a concrete
phase stutter.  None is promoted to an abstract progress or fairness action at
this boundary.
***************************************************************************)
THEOREM AsyncRunnerStepPreservesSchedulerType ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncControlServiceStateTypeInvariant
  /\ AsyncControlServiceSlotTransition
  /\ AsyncRunnerStep
  => AsyncSchedulerTypeInvariant'
PROOF
  <1>1. ASSUME StrongInductiveInvariant,
              AsyncTypeInvariant,
              AsyncControlServiceStateTypeInvariant,
              AsyncControlServiceSlotTransition,
              AsyncRunnerStep
         PROVE AsyncSchedulerTypeInvariant'
    <2>1. CASE \E node \in AsyncCurrentResponsiveVoters:
                    RunNode(node)
      <3>1. PICK node \in AsyncCurrentResponsiveVoters:
               RunNode(node)
        BY <2>1
      <3>2. node \in ValidatorIds
        BY <1>1, <3>1, AsyncCurrentResponsiveVotersAreValidators
           DEF AsyncTypeInvariant
      <3>3. RunNodeWork(node)
        BY <3>1 DEF RunNode
      <3> QED BY <1>1, <3>2, <3>3,
                   RunNodeWorkPreservesSchedulerType
    <2>2. CASE \E node \in asyncHistoricalRecoveryTargets:
                    RunHistoricalRecoveryNode(node)
      <3>1. PICK node \in asyncHistoricalRecoveryTargets:
               RunHistoricalRecoveryNode(node)
        BY <2>2
      <3>2. node \in ValidatorIds
        BY <1>1, <3>1, ModelResponsiveValidators, SMT
           DEF AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
               AsyncHistoricalRecoveryTypeInvariant,
               RunHistoricalRecoveryNode, HistoricalRecoveryTarget,
               TypeInvariant
      <3>3. RunNodeWork(node)
        BY <3>1 DEF RunHistoricalRecoveryNode
      <3> QED BY <1>1, <3>2, <3>3,
                   RunNodeWorkPreservesSchedulerType
    <2>3. CASE \E node \in AsyncResponsiveAppliedArchiveServers:
                    RunHistoricalServer(node)
      <3>1. PICK node \in AsyncResponsiveAppliedArchiveServers:
               RunHistoricalServer(node)
        BY <2>3
      <3> QED BY <1>1, <3>1,
                   RunHistoricalServerPreservesSchedulerType
    <2> QED BY <1>1, <2>1, <2>2, <2>3 DEF AsyncRunnerStep
  <1> QED BY <1>1

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
