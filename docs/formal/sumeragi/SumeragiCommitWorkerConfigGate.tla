---- MODULE SumeragiCommitWorkerConfigGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for commit-worker channel capacity configuration.

`spawn_commit_worker(...)` normalizes the configured work and result queue
capacities before allocating synchronous channels. This slice pins the
deterministic mapping: zero capacities are floored to one, and non-zero
explicit capacities are preserved.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BugSet == 0..6

Max(a, b) == IF a >= b THEN a ELSE b

\* @type: (Int, Int) => <<Int, Int>>;
SpecConfig(workCap, resultCap) ==
  <<Max(workCap, 1), Max(resultCap, 1)>>

\* @type: (Int, Int) => <<Int, Int>>;
ActualConfig(workCap, resultCap) ==
  LET resolvedWorkCap ==
        IF Bug = 1 /\ workCap = 0
        THEN workCap
        ELSE IF Bug = 2 /\ workCap = 0
        THEN 2
        ELSE IF Bug = 3 /\ workCap # 0
        THEN 1
        ELSE Max(workCap, 1)
      resolvedResultCap ==
        IF Bug = 4 /\ resultCap = 0
        THEN resultCap
        ELSE IF Bug = 5 /\ resultCap = 0
        THEN 2
        ELSE IF Bug = 6 /\ resultCap # 0
        THEN 1
        ELSE Max(resultCap, 1)
  IN <<resolvedWorkCap, resolvedResultCap>>

SpecZero ==
  SpecConfig(0, 0)

ActualZero ==
  ActualConfig(0, 0)

SpecExplicit ==
  SpecConfig(5, 7)

ActualExplicit ==
  ActualConfig(5, 7)

SpecMixedWorkZero ==
  SpecConfig(0, 7)

ActualMixedWorkZero ==
  ActualConfig(0, 7)

SpecMixedResultZero ==
  SpecConfig(5, 0)

ActualMixedResultZero ==
  ActualConfig(5, 0)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0

\* @type: <<<<Int, Int>>, <<Int, Int>>, <<Int, Int>>, <<Int, Int>>>>;
SpecOutput ==
  <<SpecZero, SpecExplicit, SpecMixedWorkZero, SpecMixedResultZero>>

\* @type: <<<<Int, Int>>, <<Int, Int>>, <<Int, Int>>, <<Int, Int>>>>;
ActualOutput ==
  <<ActualZero, ActualExplicit, ActualMixedWorkZero, ActualMixedResultZero>>

ActualOutputSet ==
  {ActualZero, ActualExplicit, ActualMixedWorkZero, ActualMixedResultZero}

WorkCapFloorAnchors ==
  /\ ActualZero[1] = 1
  /\ ActualMixedWorkZero[1] = 1

ResultCapFloorAnchors ==
  /\ ActualZero[2] = 1
  /\ ActualMixedResultZero[2] = 1

ExplicitCapPreservation ==
  /\ ActualExplicit[1] = 5
  /\ ActualExplicit[2] = 7
  /\ ActualMixedWorkZero[2] = 7
  /\ ActualMixedResultZero[1] = 5

QueueCapsPositive ==
  \A output \in ActualOutputSet:
    /\ output[1] >= 1
    /\ output[2] >= 1

CommitWorkerConfigOutputExact ==
  ActualOutput = SpecOutput

CommitWorkerConfigFloorExact ==
  /\ WorkCapFloorAnchors
  /\ ResultCapFloorAnchors

CommitWorkerConfigExplicitExact ==
  /\ ExplicitCapPreservation

CommitWorkerConfigPositiveExact ==
  /\ QueueCapsPositive

CommitWorkerConfigExactness ==
  /\ CommitWorkerConfigOutputExact
  /\ CommitWorkerConfigFloorExact
  /\ CommitWorkerConfigExplicitExact
  /\ CommitWorkerConfigPositiveExact

SafetyFast ==
  CommitWorkerConfigExactness

BugWorkZeroFloored ==
  ActualMixedWorkZero = SpecMixedWorkZero

BugWorkFloorExactlyOne ==
  ActualZero = SpecZero

BugExplicitWorkPreserved ==
  ActualExplicit = SpecExplicit

BugResultZeroFloored ==
  ActualMixedResultZero = SpecMixedResultZero

BugResultFloorExactlyOne ==
  ActualZero = SpecZero

BugExplicitResultPreserved ==
  ActualExplicit = SpecExplicit

====
