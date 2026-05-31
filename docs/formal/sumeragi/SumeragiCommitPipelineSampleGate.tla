---- MODULE SumeragiCommitPipelineSampleGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for commit-pipeline timing samples.

`CommitPipelineTimings::finish(...)` overwrites the wall-clock total with the
elapsed pipeline duration. `commit_pipeline_sample_from_timings(...)` then
converts duration fields to millisecond samples with saturating u64 semantics
and copies already-aggregated drain stage millisecond counters without mixing
them with bookkeeping fields that are intentionally absent from the status
sample.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

U64Max == 20

DurationToMs(ms) == IF ms > U64Max THEN U64Max ELSE ms

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Bool, Int, Int, Int, Int, Int) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecSample(prevTotal, elapsed, validation, qcRebuild, gate, finalize,
           drainResults, drainQc, drainPersist, drainKura, drainApply,
           drainCommit, ran, drainCount, abortInflight, eventReschedule,
           blocksConsidered, blocksProcessed) ==
  <<DurationToMs(elapsed),
    DurationToMs(validation),
    DurationToMs(qcRebuild),
    DurationToMs(gate),
    DurationToMs(finalize),
    DurationToMs(drainResults),
    drainQc,
    drainPersist,
    drainKura,
    drainApply,
    drainCommit>>

\* @type: (Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Bool, Int, Int, Int, Int, Int) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualSample(prevTotal, elapsed, validation, qcRebuild, gate, finalize,
             drainResults, drainQc, drainPersist, drainKura, drainApply,
             drainCommit, ran, drainCount, abortInflight, eventReschedule,
             blocksConsidered, blocksProcessed) ==
  LET totalMs ==
        IF Bug = 1 THEN DurationToMs(prevTotal)
        ELSE IF Bug = 2 THEN elapsed
        ELSE IF Bug = 10 THEN DurationToMs(elapsed + abortInflight)
        ELSE DurationToMs(elapsed)
      validationMs ==
        IF Bug = 3 THEN DurationToMs(elapsed)
        ELSE IF Bug = 2 THEN validation
        ELSE DurationToMs(validation)
      qcRebuildMs ==
        IF Bug = 4 THEN DurationToMs(gate)
        ELSE IF Bug = 2 THEN qcRebuild
        ELSE DurationToMs(qcRebuild)
      gateMs ==
        IF Bug = 5 THEN 0
        ELSE IF Bug = 4 THEN DurationToMs(qcRebuild)
        ELSE IF Bug = 2 THEN gate
        ELSE DurationToMs(gate)
      finalizeMs ==
        IF Bug = 6 THEN DurationToMs(eventReschedule)
        ELSE IF Bug = 2 THEN finalize
        ELSE DurationToMs(finalize)
      drainResultsMs ==
        IF Bug = 7 THEN DurationToMs(drainCount)
        ELSE IF Bug = 2 THEN drainResults
        ELSE DurationToMs(drainResults)
      drainQcMs == IF Bug = 8 THEN drainPersist ELSE drainQc
      drainPersistMs == IF Bug = 8 THEN drainQc ELSE drainPersist
      drainKuraMs == IF Bug = 11 THEN blocksProcessed ELSE drainKura
      drainApplyMs == IF Bug = 9 THEN drainCommit ELSE drainApply
      drainCommitMs == IF Bug = 9 THEN drainApply ELSE drainCommit
  IN <<totalMs,
      validationMs,
      qcRebuildMs,
      gateMs,
      finalizeMs,
      drainResultsMs,
      drainQcMs,
      drainPersistMs,
      drainKuraMs,
      drainApplyMs,
      drainCommitMs>>

SpecBase ==
  SpecSample(2, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
             TRUE, 11, 12, 13, 14, 15)

ActualBase ==
  ActualSample(2, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
               TRUE, 11, 12, 13, 14, 15)

SpecFinishIgnoresPrevious ==
  SpecSample(19, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
             TRUE, 14, 15, 16, 17, 18)

ActualFinishIgnoresPrevious ==
  ActualSample(19, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
               TRUE, 14, 15, 16, 17, 18)

SpecSaturatingDurations ==
  SpecSample(1, 22, 23, 24, 25, 26, 27, 2, 4, 6, 8, 10,
             FALSE, 12, 14, 16, 18, 20)

ActualSaturatingDurations ==
  ActualSample(1, 22, 23, 24, 25, 26, 27, 2, 4, 6, 8, 10,
               FALSE, 12, 14, 16, 18, 20)

SpecBookkeepingIgnored ==
  SpecSample(0, 4, 5, 6, 7, 8, 9, 10, 11, 3, 12, 13,
             TRUE, 19, 16, 18, 20, 17)

ActualBookkeepingIgnored ==
  ActualSample(0, 4, 5, 6, 7, 8, 9, 10, 11, 3, 12, 13,
               TRUE, 19, 16, 18, 20, 17)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

\* @type: <<<<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>>>;
SpecOutput ==
  <<SpecBase, SpecFinishIgnoresPrevious, SpecSaturatingDurations,
    SpecBookkeepingIgnored>>

\* @type: <<<<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>, <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>>>;
ActualOutput ==
  <<ActualBase, ActualFinishIgnoresPrevious, ActualSaturatingDurations,
    ActualBookkeepingIgnored>>

SafetyFast ==
  ActualOutput = SpecOutput

BugFinishSetsTotal ==
  ActualFinishIgnoresPrevious = SpecFinishIgnoresPrevious

BugDurationSaturates ==
  ActualSaturatingDurations = SpecSaturatingDurations

BugValidationMapped ==
  ActualBase = SpecBase

BugQcRebuildMapped ==
  ActualBase = SpecBase

BugGateMapped ==
  ActualBase = SpecBase

BugFinalizeMapped ==
  ActualBase = SpecBase

BugDrainResultsMapped ==
  ActualBase = SpecBase

BugDrainQcPersistIndependent ==
  ActualBase = SpecBase

BugDrainStateApplyCommitIndependent ==
  ActualBase = SpecBase

BugTotalNotPhaseSum ==
  ActualBase = SpecBase

BugBookkeepingIgnored ==
  ActualBookkeepingIgnored = SpecBookkeepingIgnored

====
