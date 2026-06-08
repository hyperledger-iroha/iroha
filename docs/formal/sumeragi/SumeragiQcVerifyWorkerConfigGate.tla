---- MODULE SumeragiQcVerifyWorkerConfigGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for QC aggregate-verification worker configuration.

`resolve_worker_config(...)` turns zero-valued configuration knobs into
runtime-derived defaults before `spawn_qc_verify_workers(...)` allocates worker
channels. This slice pins the deterministic parts of that mapping: explicit
thread and queue-cap values are preserved, auto thread count uses the observed
parallelism fallback, zero work queue caps become `threads * 4` with a floor of
four, and zero result queue caps become `threads * 8` with a floor of eight.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BugSet == 0..8

Max(a, b) == IF a >= b THEN a ELSE b

\* @type: (Int, Int, Int, Int) => <<Int, Int, Int>>;
SpecConfig(workerThreads, workCap, resultCap, available) ==
  LET threads ==
        IF workerThreads = 0 THEN available ELSE workerThreads
      resolvedWorkCap ==
        IF workCap = 0 THEN Max(threads * 4, 4) ELSE workCap
      resolvedResultCap ==
        IF resultCap = 0 THEN Max(threads * 8, 8) ELSE resultCap
  IN <<threads, resolvedWorkCap, resolvedResultCap>>

\* @type: (Int, Int, Int, Int) => <<Int, Int, Int>>;
ActualConfig(workerThreads, workCap, resultCap, available) ==
  LET threads ==
        IF Bug = 1 /\ workerThreads = 0
        THEN 1
        ELSE IF Bug = 2 /\ workerThreads # 0
        THEN available
        ELSE IF workerThreads = 0
        THEN available
        ELSE workerThreads
      resolvedWorkCap ==
        IF Bug = 3 /\ workCap = 0
        THEN 0
        ELSE IF Bug = 4 /\ workCap = 0
        THEN workCap
        ELSE IF Bug = 5 /\ workCap # 0
        THEN Max(threads * 4, 4)
        ELSE IF workCap = 0
        THEN Max(threads * 4, 4)
        ELSE workCap
      resolvedResultCap ==
        IF Bug = 6 /\ resultCap = 0
        THEN 0
        ELSE IF Bug = 7 /\ resultCap = 0
        THEN resultCap
        ELSE IF Bug = 8 /\ resultCap # 0
        THEN Max(threads * 8, 8)
        ELSE IF resultCap = 0
        THEN Max(threads * 8, 8)
        ELSE resultCap
  IN <<threads, resolvedWorkCap, resolvedResultCap>>

SpecAutoLow ==
  SpecConfig(0, 0, 0, 1)

ActualAutoLow ==
  ActualConfig(0, 0, 0, 1)

SpecAutoHigh ==
  SpecConfig(0, 0, 0, 3)

ActualAutoHigh ==
  ActualConfig(0, 0, 0, 3)

SpecExplicitThreads ==
  SpecConfig(2, 0, 0, 4)

ActualExplicitThreads ==
  ActualConfig(2, 0, 0, 4)

SpecExplicitCaps ==
  SpecConfig(2, 5, 7, 4)

ActualExplicitCaps ==
  ActualConfig(2, 5, 7, 4)

SpecMixedWorkExplicit ==
  SpecConfig(3, 9, 0, 4)

ActualMixedWorkExplicit ==
  ActualConfig(3, 9, 0, 4)

SpecMixedResultExplicit ==
  SpecConfig(3, 0, 11, 4)

ActualMixedResultExplicit ==
  ActualConfig(3, 0, 11, 4)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0

\* @type: <<<<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>>>;
SpecOutput ==
  <<SpecAutoLow, SpecAutoHigh, SpecExplicitThreads, SpecExplicitCaps,
    SpecMixedWorkExplicit, SpecMixedResultExplicit>>

\* @type: <<<<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>>>;
ActualOutput ==
  <<ActualAutoLow, ActualAutoHigh, ActualExplicitThreads, ActualExplicitCaps,
    ActualMixedWorkExplicit, ActualMixedResultExplicit>>

ActualOutputSet ==
  {ActualAutoLow, ActualAutoHigh, ActualExplicitThreads, ActualExplicitCaps,
   ActualMixedWorkExplicit, ActualMixedResultExplicit}

AutoThreadsUseAvailable ==
  /\ ActualAutoLow[1] = 1
  /\ ActualAutoHigh[1] = 3

ExplicitThreadsPreserved ==
  /\ ActualExplicitThreads[1] = 2
  /\ ActualExplicitCaps[1] = 2
  /\ ActualMixedWorkExplicit[1] = 3
  /\ ActualMixedResultExplicit[1] = 3

ZeroWorkCapDerivation ==
  /\ ActualAutoLow[2] = Max(ActualAutoLow[1] * 4, 4)
  /\ ActualAutoHigh[2] = Max(ActualAutoHigh[1] * 4, 4)
  /\ ActualExplicitThreads[2] = Max(ActualExplicitThreads[1] * 4, 4)
  /\ ActualMixedResultExplicit[2] =
     Max(ActualMixedResultExplicit[1] * 4, 4)

ZeroResultCapDerivation ==
  /\ ActualAutoLow[3] = Max(ActualAutoLow[1] * 8, 8)
  /\ ActualAutoHigh[3] = Max(ActualAutoHigh[1] * 8, 8)
  /\ ActualExplicitThreads[3] = Max(ActualExplicitThreads[1] * 8, 8)
  /\ ActualMixedWorkExplicit[3] = Max(ActualMixedWorkExplicit[1] * 8, 8)

ExplicitCapPreservation ==
  /\ ActualExplicitCaps[2] = 5
  /\ ActualExplicitCaps[3] = 7
  /\ ActualMixedWorkExplicit[2] = 9
  /\ ActualMixedResultExplicit[3] = 11

QueueCapsPositive ==
  \A output \in ActualOutputSet:
    /\ output[1] >= 1
    /\ output[2] >= 1
    /\ output[3] >= 1

QcVerifyWorkerThreadsExact ==
  /\ AutoThreadsUseAvailable
  /\ ExplicitThreadsPreserved

QcVerifyWorkerQueueCapsExact ==
  /\ ZeroWorkCapDerivation
  /\ ZeroResultCapDerivation
  /\ ExplicitCapPreservation
  /\ QueueCapsPositive

QcVerifyWorkerConfigExactness ==
  /\ ActualOutput = SpecOutput
  /\ QcVerifyWorkerThreadsExact
  /\ QcVerifyWorkerQueueCapsExact

SafetyFast ==
  ActualOutput = SpecOutput
  /\ AutoThreadsUseAvailable
  /\ ExplicitThreadsPreserved
  /\ ZeroWorkCapDerivation
  /\ ZeroResultCapDerivation
  /\ ExplicitCapPreservation
  /\ QueueCapsPositive
  /\ QcVerifyWorkerConfigExactness

BugAutoThreadsUseAvailable ==
  ActualAutoHigh = SpecAutoHigh

BugExplicitThreadsPreserved ==
  ActualExplicitThreads = SpecExplicitThreads

BugWorkCapFloor ==
  ActualAutoLow = SpecAutoLow

BugZeroWorkCapDerived ==
  ActualAutoHigh = SpecAutoHigh

BugExplicitWorkCapPreserved ==
  ActualExplicitCaps = SpecExplicitCaps

BugResultCapFloor ==
  ActualAutoLow = SpecAutoLow

BugZeroResultCapDerived ==
  ActualAutoHigh = SpecAutoHigh

BugExplicitResultCapPreserved ==
  ActualExplicitCaps = SpecExplicitCaps

====
