---- MODULE SumeragiValidationWorkerConfigGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for pending-block validation worker configuration.

`spawn_validation_workers(...)` derives concrete worker/channel sizes from
configuration knobs before allocating validation channels. This slice pins the
deterministic part of that mapping: auto worker counts clamp observed
parallelism into the validation worker range `2..=8`, explicit thread counts
are preserved, zero work queue caps become `threads * 4` with a floor of four,
and zero result queue caps become `threads * 8` with a floor of eight.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BugSet == 0..11

Max(a, b) == IF a >= b THEN a ELSE b

Min(a, b) == IF a <= b THEN a ELSE b

ClampAutoThreads(available) ==
  Max(2, Min(available, 8))

\* @type: (Int, Int, Int, Int) => <<Int, Int, Int>>;
SpecConfig(workerThreads, workCap, resultCap, available) ==
  LET threads ==
        IF workerThreads = 0 THEN ClampAutoThreads(available) ELSE Max(workerThreads, 1)
      resolvedWorkCap ==
        IF workCap = 0 THEN Max(threads * 4, 4) ELSE workCap
      resolvedResultCap ==
        IF resultCap = 0 THEN Max(threads * 8, 8) ELSE resultCap
  IN <<threads, resolvedWorkCap, resolvedResultCap>>

\* @type: (Int, Int, Int, Int) => <<Int, Int, Int>>;
ActualConfig(workerThreads, workCap, resultCap, available) ==
  LET threads ==
        IF Bug = 1 /\ workerThreads = 0 /\ available < 2
        THEN available
        ELSE IF Bug = 2 /\ workerThreads = 0 /\ available > 8
        THEN available
        ELSE IF Bug = 3 /\ workerThreads = 0 /\ available >= 2 /\ available <= 8
        THEN 2
        ELSE IF Bug = 4 /\ workerThreads # 0 /\ workerThreads < 2
        THEN 2
        ELSE IF Bug = 5 /\ workerThreads # 0 /\ workerThreads > 8
        THEN 8
        ELSE IF workerThreads = 0
        THEN ClampAutoThreads(available)
        ELSE Max(workerThreads, 1)
      resolvedWorkCap ==
        IF Bug = 6 /\ workCap = 0
        THEN 0
        ELSE IF Bug = 7 /\ workCap = 0
        THEN workCap
        ELSE IF Bug = 8 /\ workCap # 0
        THEN Max(threads * 4, 4)
        ELSE IF workCap = 0
        THEN Max(threads * 4, 4)
        ELSE workCap
      resolvedResultCap ==
        IF Bug = 9 /\ resultCap = 0
        THEN 0
        ELSE IF Bug = 10 /\ resultCap = 0
        THEN resultCap
        ELSE IF Bug = 11 /\ resultCap # 0
        THEN Max(threads * 8, 8)
        ELSE IF resultCap = 0
        THEN Max(threads * 8, 8)
        ELSE resultCap
  IN <<threads, resolvedWorkCap, resolvedResultCap>>

SpecAutoLow ==
  SpecConfig(0, 0, 0, 1)

ActualAutoLow ==
  ActualConfig(0, 0, 0, 1)

SpecAutoMid ==
  SpecConfig(0, 0, 0, 4)

ActualAutoMid ==
  ActualConfig(0, 0, 0, 4)

SpecAutoHigh ==
  SpecConfig(0, 0, 0, 12)

ActualAutoHigh ==
  ActualConfig(0, 0, 0, 12)

SpecExplicitLow ==
  SpecConfig(1, 0, 0, 12)

ActualExplicitLow ==
  ActualConfig(1, 0, 0, 12)

SpecExplicitHigh ==
  SpecConfig(12, 0, 0, 1)

ActualExplicitHigh ==
  ActualConfig(12, 0, 0, 1)

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

\* @type: <<<<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>>>;
SpecOutput ==
  <<SpecAutoLow, SpecAutoMid, SpecAutoHigh, SpecExplicitLow,
    SpecExplicitHigh, SpecExplicitCaps, SpecMixedWorkExplicit,
    SpecMixedResultExplicit>>

\* @type: <<<<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>, <<Int, Int, Int>>>>;
ActualOutput ==
  <<ActualAutoLow, ActualAutoMid, ActualAutoHigh, ActualExplicitLow,
    ActualExplicitHigh, ActualExplicitCaps, ActualMixedWorkExplicit,
    ActualMixedResultExplicit>>

ActualOutputSet ==
  {ActualAutoLow, ActualAutoMid, ActualAutoHigh, ActualExplicitLow,
   ActualExplicitHigh, ActualExplicitCaps, ActualMixedWorkExplicit,
   ActualMixedResultExplicit}

AutoThreadClampAnchors ==
  /\ ActualAutoLow[1] = 2
  /\ ActualAutoMid[1] = 4
  /\ ActualAutoHigh[1] = 8

ExplicitThreadPreservation ==
  /\ ActualExplicitLow[1] = 1
  /\ ActualExplicitHigh[1] = 12
  /\ ActualExplicitCaps[1] = 2
  /\ ActualMixedWorkExplicit[1] = 3
  /\ ActualMixedResultExplicit[1] = 3

ZeroWorkCapDerivation ==
  /\ ActualAutoLow[2] = Max(ActualAutoLow[1] * 4, 4)
  /\ ActualAutoMid[2] = Max(ActualAutoMid[1] * 4, 4)
  /\ ActualAutoHigh[2] = Max(ActualAutoHigh[1] * 4, 4)
  /\ ActualExplicitLow[2] = Max(ActualExplicitLow[1] * 4, 4)
  /\ ActualExplicitHigh[2] = Max(ActualExplicitHigh[1] * 4, 4)
  /\ ActualMixedResultExplicit[2] =
     Max(ActualMixedResultExplicit[1] * 4, 4)

ZeroResultCapDerivation ==
  /\ ActualAutoLow[3] = Max(ActualAutoLow[1] * 8, 8)
  /\ ActualAutoMid[3] = Max(ActualAutoMid[1] * 8, 8)
  /\ ActualAutoHigh[3] = Max(ActualAutoHigh[1] * 8, 8)
  /\ ActualExplicitLow[3] = Max(ActualExplicitLow[1] * 8, 8)
  /\ ActualExplicitHigh[3] = Max(ActualExplicitHigh[1] * 8, 8)
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

ValidationWorkerConfigCoreSafety ==
  ActualOutput = SpecOutput
  /\ AutoThreadClampAnchors
  /\ ExplicitThreadPreservation
  /\ ZeroWorkCapDerivation
  /\ ZeroResultCapDerivation
  /\ ExplicitCapPreservation
  /\ QueueCapsPositive

ValidationWorkerConfigExactness ==
  ActualOutput = SpecOutput
  /\ AutoThreadClampAnchors
  /\ ExplicitThreadPreservation
  /\ ZeroWorkCapDerivation
  /\ ZeroResultCapDerivation
  /\ ExplicitCapPreservation
  /\ QueueCapsPositive
ValidationWorkerConfigCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ValidationWorkerConfigExactness

SafetyFast ==
  ValidationWorkerConfigExactness

BugAutoMinFloor ==
  ActualAutoLow = SpecAutoLow

BugAutoMaxCap ==
  ActualAutoHigh = SpecAutoHigh

BugAutoMidPreserved ==
  ActualAutoMid = SpecAutoMid

BugExplicitLowPreserved ==
  ActualExplicitLow = SpecExplicitLow

BugExplicitHighPreserved ==
  ActualExplicitHigh = SpecExplicitHigh

BugWorkCapFloor ==
  ActualExplicitLow = SpecExplicitLow

BugZeroWorkCapDerived ==
  ActualAutoMid = SpecAutoMid

BugExplicitWorkCapPreserved ==
  ActualExplicitCaps = SpecExplicitCaps

BugResultCapFloor ==
  ActualExplicitLow = SpecExplicitLow

BugZeroResultCapDerived ==
  ActualAutoMid = SpecAutoMid

BugExplicitResultCapPreserved ==
  ActualExplicitCaps = SpecExplicitCaps

====
