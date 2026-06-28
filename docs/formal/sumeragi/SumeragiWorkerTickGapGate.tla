---- MODULE SumeragiWorkerTickGapGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for worker tick-gap helpers in `sumeragi/mod.rs`.

This slice pins `should_run_tick(...)` and `idle_wait_duration(...)`. Both
helpers use `Instant::saturating_duration_since(...)`: future `last_tick`
values produce zero elapsed time, the run gate is inclusive at the gap
boundary, and idle waiting returns `None` exactly when the run gate is open.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoWait == -1

Cases == {
  "zero_gap",
  "before_gap",
  "at_gap",
  "after_gap",
  "future_last",
  "future_last_zero_gap",
  "large_gap"
}

PendingCases == {"before_gap", "future_last", "large_gap"}
ReadyCases == {"zero_gap", "at_gap", "after_gap", "future_last_zero_gap"}

BugSet == {
  "none",
  "run_strict_boundary",
  "run_future_underflows",
  "run_ignores_gap",
  "wait_boundary_some",
  "wait_after_some",
  "wait_before_none",
  "wait_before_uses_gap",
  "wait_future_underflows",
  "wait_zero_gap_some"
}

Now(c) ==
  CASE c = "zero_gap" -> 5
    [] c = "before_gap" -> 7
    [] c = "at_gap" -> 10
    [] c = "after_gap" -> 12
    [] c = "future_last" -> 4
    [] c = "future_last_zero_gap" -> 4
    [] c = "large_gap" -> 8
    [] OTHER -> 0

LastTick(c) ==
  CASE c = "zero_gap" -> 5
    [] c = "before_gap" -> 5
    [] c = "at_gap" -> 5
    [] c = "after_gap" -> 5
    [] c = "future_last" -> 9
    [] c = "future_last_zero_gap" -> 9
    [] c = "large_gap" -> 1
    [] OTHER -> 0

MinGap(c) ==
  CASE c = "zero_gap" -> 0
    [] c = "before_gap" -> 5
    [] c = "at_gap" -> 5
    [] c = "after_gap" -> 5
    [] c = "future_last" -> 4
    [] c = "future_last_zero_gap" -> 0
    [] c = "large_gap" -> 10
    [] OTHER -> 0

SaturatingElapsed(c) ==
  IF Now(c) >= LastTick(c) THEN Now(c) - LastTick(c) ELSE 0

SpecShouldRun(c) ==
  SaturatingElapsed(c) >= MinGap(c)

SpecIdleWait(c) ==
  IF SpecShouldRun(c) THEN NoWait ELSE MinGap(c) - SaturatingElapsed(c)

ActualShouldRun(c) ==
  CASE Bug = "run_strict_boundary"
       /\ c = "at_gap" -> SaturatingElapsed(c) > MinGap(c)
    [] Bug = "run_future_underflows"
       /\ c = "future_last" -> LastTick(c) - Now(c) >= MinGap(c)
    [] Bug = "run_ignores_gap"
       /\ c = "before_gap" -> TRUE
    [] OTHER -> SpecShouldRun(c)

ActualIdleWait(c) ==
  CASE Bug = "wait_boundary_some"
       /\ c = "at_gap" -> 0
    [] Bug = "wait_after_some"
       /\ c = "after_gap" -> 1
    [] Bug = "wait_before_none"
       /\ c = "before_gap" -> NoWait
    [] Bug = "wait_before_uses_gap"
       /\ c = "before_gap" -> MinGap(c)
    [] Bug = "wait_future_underflows"
       /\ c = "future_last" -> LastTick(c) - Now(c)
    [] Bug = "wait_zero_gap_some"
       /\ c = "zero_gap" -> 0
    [] OTHER -> SpecIdleWait(c)

Init == checked = 0
Next == UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked \in 0..1

CasePartitionExact ==
  /\ PendingCases \cup ReadyCases = Cases
  /\ PendingCases \cap ReadyCases = {}

SaturatingElapsedBounds ==
  \A c \in Cases:
    /\ SaturatingElapsed(c) >= 0
    /\ SaturatingElapsed(c) <= IF Now(c) >= LastTick(c) THEN Now(c) - LastTick(c) ELSE 0

RunReadinessPartition ==
  /\ \A c \in ReadyCases: ActualShouldRun(c)
  /\ \A c \in PendingCases: ~ActualShouldRun(c)

WaitReadinessPartition ==
  /\ \A c \in ReadyCases: ActualIdleWait(c) = NoWait
  /\ \A c \in PendingCases:
      /\ ActualIdleWait(c) = MinGap(c) - SaturatingElapsed(c)
      /\ ActualIdleWait(c) > 0

BoundaryAndFutureAnchors ==
  /\ ActualShouldRun("at_gap")
  /\ ~ActualShouldRun("before_gap")
  /\ ~ActualShouldRun("future_last")
  /\ ActualShouldRun("future_last_zero_gap")
  /\ ActualIdleWait("at_gap") = NoWait
  /\ ActualIdleWait("future_last") = MinGap("future_last")

WorkerTickGapExactness ==
  /\ \A c \in Cases: ActualShouldRun(c) = SpecShouldRun(c)
  /\ \A c \in Cases: ActualIdleWait(c) = SpecIdleWait(c)
  /\ \A c \in Cases: (ActualIdleWait(c) = NoWait) = ActualShouldRun(c)
  /\ \A c \in PendingCases: ActualIdleWait(c) > 0
  /\ \A c \in ReadyCases: ActualIdleWait(c) = NoWait
  /\ CasePartitionExact
  /\ SaturatingElapsedBounds
  /\ RunReadinessPartition
  /\ WaitReadinessPartition
  /\ BoundaryAndFutureAnchors

WorkerTickGapCoreSafety == WorkerTickGapExactness

WorkerTickGapCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ WorkerTickGapExactness

NoBugInvariant == WorkerTickGapExactness

Safety ==
  WorkerTickGapExactness

SafetyFast ==
  WorkerTickGapExactness

BugRunStrictBoundary ==
  ActualShouldRun("at_gap") = SpecShouldRun("at_gap")

BugRunFutureUnderflows ==
  ActualShouldRun("future_last") = SpecShouldRun("future_last")

BugRunIgnoresGap ==
  ActualShouldRun("before_gap") = SpecShouldRun("before_gap")

BugWaitBoundarySome ==
  ActualIdleWait("at_gap") = SpecIdleWait("at_gap")

BugWaitAfterSome ==
  ActualIdleWait("after_gap") = SpecIdleWait("after_gap")

BugWaitBeforeNone ==
  ActualIdleWait("before_gap") = SpecIdleWait("before_gap")

BugWaitBeforeUsesGap ==
  ActualIdleWait("before_gap") = SpecIdleWait("before_gap")

BugWaitFutureUnderflows ==
  ActualIdleWait("future_last") = SpecIdleWait("future_last")

BugWaitZeroGapSome ==
  ActualIdleWait("zero_gap") = SpecIdleWait("zero_gap")
====
