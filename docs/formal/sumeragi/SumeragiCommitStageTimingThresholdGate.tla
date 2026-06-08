---- MODULE SumeragiCommitStageTimingThresholdGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for slow commit-stage timing detection.

`commit_stage_timings_exceed_threshold(...)` reports slow commit work only when
the threshold is nonzero and either the blocking QC+persist total reaches the
threshold or any observed stage/validation substage reaches it. The surrounding
logging path first checks `CommitStageTimings::has_recorded_stages(...)`; a
prevalidated-only artifact therefore counts as recorded, but it does not report
slow work without a measured stage.
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

Max(a, b) == IF a >= b THEN a ELSE b

SatAdd(a, b) == IF a + b > U64Max THEN U64Max ELSE a + b

MaxOpt(a, b) ==
  IF ~Some(a) THEN b
  ELSE IF ~Some(b) THEN a
  ELSE Max(a, b)

AnyStage(qc, persist, kura, stateApply, stateCommit,
         valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize) ==
  Some(qc) \/ Some(persist) \/ Some(kura) \/ Some(stateApply) \/
  Some(stateCommit) \/ Some(valTotal) \/ Some(valStateless) \/
  Some(valExec) \/ Some(valTx) \/ Some(valTxApply) \/ Some(valTxFinalize)

BlockingTotal(qc, persist) ==
  IF Some(qc) \/ Some(persist)
  THEN SatAdd(Val(qc), Val(persist))
  ELSE None

MaxObserved(qc, persist, kura, stateApply, stateCommit,
            valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize) ==
  MaxOpt(
    MaxOpt(
      MaxOpt(
        MaxOpt(
          MaxOpt(MaxOpt(qc, persist), MaxOpt(kura, stateApply)),
          MaxOpt(stateCommit, valTotal)),
        MaxOpt(valStateless, valExec)),
      MaxOpt(valTx, valTxApply)),
    valTxFinalize)

MaxNonValidation(qc, persist, kura, stateApply, stateCommit) ==
  MaxOpt(MaxOpt(qc, persist), MaxOpt(MaxOpt(kura, stateApply), stateCommit))

ValidationTotalOnly(valTotal) ==
  valTotal

SumAll(qc, persist, kura, stateApply, stateCommit,
       valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize) ==
  SatAdd(
    SatAdd(
      SatAdd(
        SatAdd(
          SatAdd(SatAdd(Val(qc), Val(persist)), SatAdd(Val(kura), Val(stateApply))),
          SatAdd(Val(stateCommit), Val(valTotal))),
        SatAdd(Val(valStateless), Val(valExec))),
      SatAdd(Val(valTx), Val(valTxApply))),
    Val(valTxFinalize))

SpecHasRecorded(qc, persist, kura, stateApply, stateCommit,
                valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize,
                prevalidated) ==
  prevalidated \/
  AnyStage(qc, persist, kura, stateApply, stateCommit,
           valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize)

SpecExceeds(qc, persist, kura, stateApply, stateCommit,
            valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize,
            threshold) ==
  LET blocking == BlockingTotal(qc, persist)
      maxStage == MaxObserved(qc, persist, kura, stateApply, stateCommit,
                              valTotal, valStateless, valExec, valTx,
                              valTxApply, valTxFinalize)
  IN threshold # 0 /\
     ((Some(blocking) /\ blocking >= threshold) \/
      (Some(maxStage) /\ maxStage >= threshold))

ActualExceeds(qc, persist, kura, stateApply, stateCommit,
              valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize,
              threshold) ==
  LET blocking == BlockingTotal(qc, persist)
      maxStage == MaxObserved(qc, persist, kura, stateApply, stateCommit,
                              valTotal, valStateless, valExec, valTx,
                              valTxApply, valTxFinalize)
      nonValidation == MaxNonValidation(qc, persist, kura, stateApply, stateCommit)
      allTotal == SumAll(qc, persist, kura, stateApply, stateCommit,
                         valTotal, valStateless, valExec, valTx,
                         valTxApply, valTxFinalize)
  IN IF Bug = 1 /\ threshold = 0
     THEN Some(blocking) \/ Some(maxStage)
     ELSE IF Bug = 2
     THEN threshold # 0 /\ Some(maxStage) /\ maxStage >= threshold
     ELSE IF Bug = 3
     THEN threshold # 0 /\
          ((Some(blocking) /\ blocking > threshold) \/
           (Some(maxStage) /\ maxStage >= threshold))
     ELSE IF Bug = 4
     THEN threshold # 0 /\
          (((Some(blocking) /\ blocking >= threshold) \/
            (Some(maxStage) /\ maxStage >= threshold)) \/
           allTotal >= threshold)
     ELSE IF Bug = 5
     THEN threshold # 0 /\ Some(blocking) /\ blocking >= threshold
     ELSE IF Bug = 6
     THEN threshold # 0 /\
          ((Some(blocking) /\ blocking >= threshold) \/
           (Some(maxStage) /\ maxStage > threshold))
     ELSE IF Bug = 7
     THEN threshold # 0 /\
          ((Some(blocking) /\ blocking >= threshold) \/
           (Some(nonValidation) /\ nonValidation >= threshold))
     ELSE IF Bug = 8
     THEN threshold # 0 /\
          ((Some(blocking) /\ blocking >= threshold) \/
           (Some(nonValidation) /\ nonValidation >= threshold) \/
           (Some(ValidationTotalOnly(valTotal)) /\ ValidationTotalOnly(valTotal) >= threshold))
     ELSE SpecExceeds(qc, persist, kura, stateApply, stateCommit,
                      valTotal, valStateless, valExec, valTx,
                      valTxApply, valTxFinalize, threshold)

SpecSlowLog(qc, persist, kura, stateApply, stateCommit,
            valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize,
            prevalidated, threshold) ==
  SpecHasRecorded(qc, persist, kura, stateApply, stateCommit,
                  valTotal, valStateless, valExec, valTx, valTxApply,
                  valTxFinalize, prevalidated) /\
  SpecExceeds(qc, persist, kura, stateApply, stateCommit,
              valTotal, valStateless, valExec, valTx, valTxApply,
              valTxFinalize, threshold)

ActualSlowLog(qc, persist, kura, stateApply, stateCommit,
              valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize,
              prevalidated, threshold) ==
  LET hasRecorded ==
        SpecHasRecorded(qc, persist, kura, stateApply, stateCommit,
                        valTotal, valStateless, valExec, valTx, valTxApply,
                        valTxFinalize, prevalidated)
      anyStage ==
        AnyStage(qc, persist, kura, stateApply, stateCommit,
                 valTotal, valStateless, valExec, valTx, valTxApply, valTxFinalize)
  IN IF Bug = 9 /\ prevalidated /\ ~anyStage
     THEN TRUE
     ELSE IF Bug = 10 /\ ~hasRecorded
     THEN TRUE
     ELSE hasRecorded /\
          ActualExceeds(qc, persist, kura, stateApply, stateCommit,
                        valTotal, valStateless, valExec, valTx, valTxApply,
                        valTxFinalize, threshold)

SpecZeroThreshold ==
  SpecSlowLog(None, 6, None, None, None, None, None, None, None, None, None,
              FALSE, 0)

ActualZeroThreshold ==
  ActualSlowLog(None, 6, None, None, None, None, None, None, None, None, None,
                FALSE, 0)

SpecBlockingSum ==
  SpecSlowLog(3, 3, None, None, None, None, None, None, None, None, None,
              FALSE, 5)

ActualBlockingSum ==
  ActualSlowLog(3, 3, None, None, None, None, None, None, None, None, None,
                FALSE, 5)

SpecBlockingBoundary ==
  SpecSlowLog(2, 3, None, None, None, None, None, None, None, None, None,
              FALSE, 5)

ActualBlockingBoundary ==
  ActualSlowLog(2, 3, None, None, None, None, None, None, None, None, None,
                FALSE, 5)

SpecNonBlockingSum ==
  SpecSlowLog(None, None, None, 3, 3, None, None, None, None, None, None,
              FALSE, 5)

ActualNonBlockingSum ==
  ActualSlowLog(None, None, None, 3, 3, None, None, None, None, None, None,
                FALSE, 5)

SpecSlowStage ==
  SpecSlowLog(None, None, None, 6, None, None, None, None, None, None, None,
              FALSE, 5)

ActualSlowStage ==
  ActualSlowLog(None, None, None, 6, None, None, None, None, None, None, None,
                FALSE, 5)

SpecStageBoundary ==
  SpecSlowLog(None, None, None, 5, None, None, None, None, None, None, None,
              FALSE, 5)

ActualStageBoundary ==
  ActualSlowLog(None, None, None, 5, None, None, None, None, None, None, None,
                FALSE, 5)

SpecValidationSubstage ==
  SpecSlowLog(None, None, None, None, None, 1, None, 6, None, None, None,
              FALSE, 5)

ActualValidationSubstage ==
  ActualSlowLog(None, None, None, None, None, 1, None, 6, None, None, None,
                FALSE, 5)

SpecPrevalidatedOnly ==
  SpecSlowLog(None, None, None, None, None, None, None, None, None, None, None,
              TRUE, 5)

ActualPrevalidatedOnly ==
  ActualSlowLog(None, None, None, None, None, None, None, None, None, None, None,
                TRUE, 5)

SpecEmpty ==
  SpecSlowLog(None, None, None, None, None, None, None, None, None, None, None,
              FALSE, 5)

ActualEmpty ==
  ActualSlowLog(None, None, None, None, None, None, None, None, None, None, None,
                FALSE, 5)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

\* @type: <<Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
SpecOutput ==
  <<SpecZeroThreshold, SpecBlockingSum, SpecBlockingBoundary,
    SpecNonBlockingSum, SpecSlowStage, SpecStageBoundary,
    SpecValidationSubstage, SpecPrevalidatedOnly, SpecEmpty>>

\* @type: <<Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
ActualOutput ==
  <<ActualZeroThreshold, ActualBlockingSum, ActualBlockingBoundary,
    ActualNonBlockingSum, ActualSlowStage, ActualStageBoundary,
    ActualValidationSubstage, ActualPrevalidatedOnly, ActualEmpty>>

CommitStageTimingOutputExact ==
  ActualOutput = SpecOutput

ZeroThresholdSuppressionAnchors ==
  /\ SpecZeroThreshold = FALSE
  /\ ActualZeroThreshold = FALSE

BlockingTotalThresholdAnchors ==
  /\ SpecBlockingSum = TRUE
  /\ ActualBlockingSum = TRUE
  /\ SpecBlockingBoundary = TRUE
  /\ ActualBlockingBoundary = TRUE

NonBlockingStageIsolationAnchors ==
  /\ SpecNonBlockingSum = FALSE
  /\ ActualNonBlockingSum = FALSE

StageMaximumThresholdAnchors ==
  /\ SpecSlowStage = TRUE
  /\ ActualSlowStage = TRUE
  /\ SpecStageBoundary = TRUE
  /\ ActualStageBoundary = TRUE

ValidationSubstageThresholdAnchors ==
  /\ SpecValidationSubstage = TRUE
  /\ ActualValidationSubstage = TRUE

RecordedTimingGateAnchors ==
  /\ SpecPrevalidatedOnly = FALSE
  /\ ActualPrevalidatedOnly = FALSE
  /\ SpecEmpty = FALSE
  /\ ActualEmpty = FALSE

CommitStageTimingZeroThresholdExact ==
  ZeroThresholdSuppressionAnchors

CommitStageTimingBlockingExact ==
  BlockingTotalThresholdAnchors

CommitStageTimingStageExact ==
  /\ NonBlockingStageIsolationAnchors
  /\ StageMaximumThresholdAnchors
  /\ ValidationSubstageThresholdAnchors

CommitStageTimingRecordedGateExact ==
  RecordedTimingGateAnchors

CommitStageTimingThresholdExactness ==
  /\ CommitStageTimingOutputExact
  /\ CommitStageTimingZeroThresholdExact
  /\ CommitStageTimingBlockingExact
  /\ CommitStageTimingStageExact
  /\ CommitStageTimingRecordedGateExact

SafetyFast ==
  CommitStageTimingThresholdExactness

SafetyBreakdown ==
  /\ ZeroThresholdSuppressionAnchors
  /\ BlockingTotalThresholdAnchors
  /\ NonBlockingStageIsolationAnchors
  /\ StageMaximumThresholdAnchors
  /\ ValidationSubstageThresholdAnchors
  /\ RecordedTimingGateAnchors

BugZeroThresholdSuppressed ==
  ActualZeroThreshold = SpecZeroThreshold

BugBlockingTotalIncluded ==
  ActualBlockingSum = SpecBlockingSum

BugBlockingBoundaryInclusive ==
  ActualBlockingBoundary = SpecBlockingBoundary

BugNonBlockingStagesNotSummed ==
  ActualNonBlockingSum = SpecNonBlockingSum

BugMaxStageIncluded ==
  ActualSlowStage = SpecSlowStage

BugMaxStageBoundaryInclusive ==
  ActualStageBoundary = SpecStageBoundary

BugValidationSubstageIncluded ==
  ActualValidationSubstage = SpecValidationSubstage

BugValidationSubstageNotTotalOnly ==
  ActualValidationSubstage = SpecValidationSubstage

BugPrevalidatedOnlyNoReport ==
  ActualPrevalidatedOnly = SpecPrevalidatedOnly

BugEmptyNoReport ==
  ActualEmpty = SpecEmpty

====
