---- MODULE SumeragiCommitDrainSummaryGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for commit-drain summary aggregation.

`CommitDrainSummary::record(...)` is called after an accepted commit-worker
result is applied. It increments the result count with saturating arithmetic,
adds only the timing fields that are present, keeps timing accumulators
independent, and leaves the `progress` flag owned by the surrounding drain path.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

None == -1
U64Max == 20

Some(v) == v >= 0

Val(v) == IF Some(v) THEN v ELSE 0

SatAdd(a, b) == IF a + b > U64Max THEN U64Max ELSE a + b

\* @type: (Bool, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => <<Bool, Int, Int, Int, Int, Int, Int>>;
SpecRecord(progress, results, qcTotal, persistTotal, kuraTotal, applyTotal,
           commitTotal, qcMs, persistMs, kuraMs, applyMs, commitMs) ==
  <<progress,
    SatAdd(results, 1),
    IF Some(qcMs) THEN SatAdd(qcTotal, qcMs) ELSE qcTotal,
    IF Some(persistMs) THEN SatAdd(persistTotal, persistMs) ELSE persistTotal,
    IF Some(kuraMs) THEN SatAdd(kuraTotal, kuraMs) ELSE kuraTotal,
    IF Some(applyMs) THEN SatAdd(applyTotal, applyMs) ELSE applyTotal,
    IF Some(commitMs) THEN SatAdd(commitTotal, commitMs) ELSE commitTotal>>

\* @type: (Bool, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int) => <<Bool, Int, Int, Int, Int, Int, Int>>;
ActualRecord(progress, results, qcTotal, persistTotal, kuraTotal, applyTotal,
             commitTotal, qcMs, persistMs, kuraMs, applyMs, commitMs) ==
  LET nextResults ==
        IF Bug = 1 THEN results
        ELSE IF Bug = 9 THEN results + 1
        ELSE SatAdd(results, 1)
      nextProgress ==
        IF Bug = 2 THEN ~progress ELSE progress
      nextQc ==
        IF Bug = 3 /\ ~Some(qcMs) /\ ~Some(persistMs) /\ ~Some(kuraMs) /\
           ~Some(applyMs) /\ ~Some(commitMs)
        THEN SatAdd(qcTotal, 1)
        ELSE IF Bug = 4 /\ Some(qcMs)
        THEN qcTotal
        ELSE IF Bug = 5 /\ Some(persistMs)
        THEN SatAdd(qcTotal, persistMs)
        ELSE IF Some(qcMs)
        THEN SatAdd(qcTotal, qcMs)
        ELSE qcTotal
      nextPersist ==
        IF Bug = 5 /\ Some(persistMs)
        THEN persistTotal
        ELSE IF Some(persistMs)
        THEN SatAdd(persistTotal, persistMs)
        ELSE persistTotal
      nextKura ==
        IF Bug = 6 /\ Some(kuraMs)
        THEN kuraTotal
        ELSE IF Some(kuraMs)
        THEN SatAdd(kuraTotal, kuraMs)
        ELSE kuraTotal
      nextApply ==
        IF Bug = 7 /\ Some(applyMs)
        THEN applyTotal
        ELSE IF Some(applyMs)
        THEN SatAdd(applyTotal, applyMs)
        ELSE applyTotal
      nextCommit ==
        IF Bug = 8 /\ Some(commitMs)
        THEN commitTotal
        ELSE IF Bug = 10 /\ Some(commitMs)
        THEN commitTotal + commitMs
        ELSE IF Some(commitMs)
        THEN SatAdd(commitTotal, commitMs)
        ELSE commitTotal
  IN <<nextProgress, nextResults, nextQc, nextPersist, nextKura, nextApply, nextCommit>>

SpecNoTimings ==
  SpecRecord(FALSE, 0, 0, 0, 0, 0, 0, None, None, None, None, None)

ActualNoTimings ==
  ActualRecord(FALSE, 0, 0, 0, 0, 0, 0, None, None, None, None, None)

SpecAllTimings ==
  SpecRecord(FALSE, 2, 10, 11, 12, 13, 14, 1, 2, 3, 4, 5)

ActualAllTimings ==
  ActualRecord(FALSE, 2, 10, 11, 12, 13, 14, 1, 2, 3, 4, 5)

SpecProgressPreserved ==
  SpecRecord(TRUE, 1, 1, 1, 1, 1, 1, None, 2, None, None, None)

ActualProgressPreserved ==
  ActualRecord(TRUE, 1, 1, 1, 1, 1, 1, None, 2, None, None, None)

SpecSaturating ==
  SpecRecord(TRUE, U64Max, 19, 19, 19, 19, 19, 5, 5, 5, 5, 5)

ActualSaturating ==
  ActualRecord(TRUE, U64Max, 19, 19, 19, 19, 19, 5, 5, 5, 5, 5)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

\* @type: <<<<Bool, Int, Int, Int, Int, Int, Int>>, <<Bool, Int, Int, Int, Int, Int, Int>>, <<Bool, Int, Int, Int, Int, Int, Int>>, <<Bool, Int, Int, Int, Int, Int, Int>>>>;
SpecOutput ==
  <<SpecNoTimings, SpecAllTimings, SpecProgressPreserved, SpecSaturating>>

\* @type: <<<<Bool, Int, Int, Int, Int, Int, Int>>, <<Bool, Int, Int, Int, Int, Int, Int>>, <<Bool, Int, Int, Int, Int, Int, Int>>, <<Bool, Int, Int, Int, Int, Int, Int>>>>;
ActualOutput ==
  <<ActualNoTimings, ActualAllTimings, ActualProgressPreserved, ActualSaturating>>

SafetyFast ==
  ActualOutput = SpecOutput

BugResultsIncremented ==
  ActualNoTimings = SpecNoTimings

BugProgressPreserved ==
  ActualProgressPreserved = SpecProgressPreserved

BugNoneTimingsIgnored ==
  ActualNoTimings = SpecNoTimings

BugQcTimingRecorded ==
  ActualAllTimings = SpecAllTimings

BugPersistTimingIndependent ==
  ActualAllTimings = SpecAllTimings

BugKuraTimingRecorded ==
  ActualAllTimings = SpecAllTimings

BugStateApplyTimingRecorded ==
  ActualAllTimings = SpecAllTimings

BugStateCommitTimingRecorded ==
  ActualAllTimings = SpecAllTimings

BugResultSaturates ==
  ActualSaturating = SpecSaturating

BugStageSaturates ==
  ActualSaturating = SpecSaturating

====
