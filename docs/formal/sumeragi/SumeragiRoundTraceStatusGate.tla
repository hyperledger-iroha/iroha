---- MODULE SumeragiRoundTraceStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi round-trace status recording.

This slice pins the observable contracts around `RoundTraceState::transition`,
`RoundTraceState::snapshot_gaps`, `record_round_trace(...)`,
`reset_round_trace_for_tests()`, `request_commit_pipeline_for_round(...)`,
`request_commit_pipeline_for_pending(...)`, and `record_round_no_progress_wake()`.

Concrete `Instant`, queue-depth, and block-hash values are collapsed into
observable actions so the model stays small while preserving the contracts
used by operator status snapshots and commit-pipeline wakeups.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

TransitionNewKeyNoPrevious == 1
TransitionNewKeySetsKeyPhase == 2
TransitionNewKeyResetsGaps == 3
TransitionSamePhaseReturnsPrevious == 4
TransitionSamePhasePreservesState == 5
TransitionPhaseChangeReturnsPrevious == 6
TransitionPhaseChangeRecordsOldGap == 7
TransitionPhaseChangeSetsNext == 8
SnapshotNoKeyZero == 9
SnapshotActiveElapsed == 10
SnapshotKeepsCompleted == 11
RecordTraceAppendsLatest == 12
RecordTracePrunesOldest == 13
RecordTraceSnapshotProjects == 14
ResetClearsTrace == 15
RequestRoundExactMetadata == 16
RequestRoundWakesPipeline == 17
RequestPendingAbsentOnlyWakes == 18
RequestPendingPresentInfersPhase == 19
RequestPendingPreservesLatency == 20
NoProgressPresentRecordsFlag == 21
NoProgressAbsentNoTrace == 22
EntryCopiesQueueState == 23

Candidates == 1..23

SetKeyInput == 1
SetPhaseInput == 2
ResetGaps == 3
SetStartNow == 4
ReturnNone == 5
ReturnPreviousPhase == 6
PreserveState == 7
SetGapForPreviousPhase == 8
NoNextGapWrite == 9
SnapshotZeroGaps == 10
SnapshotActivePhaseElapsed == 11
PreserveCompletedGaps == 12
AppendEntry == 13
SetLatestEntry == 14
PruneOldest == 15
PreserveNewestEntries == 16
ProjectSnapshotEntries == 17
ProjectSnapshotGaps == 18
ResetTraceState == 19
WakePipeline == 20
NoTraceRecorded == 21
InferPhaseFromPending == 22
PreserveHeightView == 23
PreserveCause == 24
PreserveQueueLatency == 25
ClearNoProgressFlag == 26
SetNoProgressFlag == 27
CauseNoProgress == 28
CopyPendingCounts == 29
CopyBackpressure == 30
CopyQueueDepths == 31

Actions == 1..31

SpecActions(candidate) ==
  CASE candidate = TransitionNewKeyNoPrevious ->
      {ReturnNone}
    [] candidate = TransitionNewKeySetsKeyPhase ->
      {SetKeyInput, SetPhaseInput}
    [] candidate = TransitionNewKeyResetsGaps ->
      {ResetGaps, SetStartNow}
    [] candidate = TransitionSamePhaseReturnsPrevious ->
      {ReturnPreviousPhase}
    [] candidate = TransitionSamePhasePreservesState ->
      {PreserveState}
    [] candidate = TransitionPhaseChangeReturnsPrevious ->
      {ReturnPreviousPhase}
    [] candidate = TransitionPhaseChangeRecordsOldGap ->
      {SetGapForPreviousPhase, NoNextGapWrite}
    [] candidate = TransitionPhaseChangeSetsNext ->
      {SetPhaseInput, SetStartNow}
    [] candidate = SnapshotNoKeyZero ->
      {SnapshotZeroGaps}
    [] candidate = SnapshotActiveElapsed ->
      {SnapshotActivePhaseElapsed}
    [] candidate = SnapshotKeepsCompleted ->
      {PreserveCompletedGaps, SnapshotActivePhaseElapsed}
    [] candidate = RecordTraceAppendsLatest ->
      {AppendEntry, SetLatestEntry}
    [] candidate = RecordTracePrunesOldest ->
      {PruneOldest, PreserveNewestEntries}
    [] candidate = RecordTraceSnapshotProjects ->
      {ProjectSnapshotEntries, ProjectSnapshotGaps}
    [] candidate = ResetClearsTrace ->
      {ResetTraceState}
    [] candidate = RequestRoundExactMetadata ->
      {AppendEntry, SetLatestEntry, SetKeyInput, SetPhaseInput,
       PreserveCause, PreserveQueueLatency, ClearNoProgressFlag}
    [] candidate = RequestRoundWakesPipeline ->
      {WakePipeline}
    [] candidate = RequestPendingAbsentOnlyWakes ->
      {NoTraceRecorded, WakePipeline}
    [] candidate = RequestPendingPresentInfersPhase ->
      {AppendEntry, SetLatestEntry, InferPhaseFromPending,
       PreserveHeightView, PreserveCause}
    [] candidate = RequestPendingPreservesLatency ->
      {PreserveQueueLatency}
    [] candidate = NoProgressPresentRecordsFlag ->
      {AppendEntry, SetLatestEntry, InferPhaseFromPending,
       CauseNoProgress, SetNoProgressFlag}
    [] candidate = NoProgressAbsentNoTrace ->
      {NoTraceRecorded}
    [] candidate = EntryCopiesQueueState ->
      {CopyPendingCounts, CopyBackpressure, CopyQueueDepths}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = TransitionNewKeyNoPrevious /\
          Bug = "transition_new_key_returns_previous" ->
      (spec \ {ReturnNone}) \cup {ReturnPreviousPhase}
    [] candidate = TransitionNewKeySetsKeyPhase /\
          Bug = "transition_new_key_keeps_old_phase" ->
      spec \ {SetPhaseInput}
    [] candidate = TransitionNewKeySetsKeyPhase /\
          Bug = "transition_new_key_no_key" ->
      spec \ {SetKeyInput}
    [] candidate = TransitionNewKeyResetsGaps /\
          Bug = "transition_new_key_keeps_old_gaps" ->
      spec \ {ResetGaps}
    [] candidate = TransitionSamePhaseReturnsPrevious /\
          Bug = "transition_same_phase_returns_none" ->
      (spec \ {ReturnPreviousPhase}) \cup {ReturnNone}
    [] candidate = TransitionSamePhasePreservesState /\
          Bug = "transition_same_phase_resets_state" ->
      (spec \ {PreserveState}) \cup {SetPhaseInput, SetStartNow}
    [] candidate = TransitionPhaseChangeReturnsPrevious /\
          Bug = "transition_phase_change_returns_next" ->
      (spec \ {ReturnPreviousPhase}) \cup {SetPhaseInput}
    [] candidate = TransitionPhaseChangeRecordsOldGap /\
          Bug = "transition_phase_change_skips_gap" ->
      spec \ {SetGapForPreviousPhase}
    [] candidate = TransitionPhaseChangeRecordsOldGap /\
          Bug = "transition_phase_change_writes_next_gap" ->
      spec \ {NoNextGapWrite}
    [] candidate = TransitionPhaseChangeSetsNext /\
          Bug = "transition_phase_change_keeps_start" ->
      spec \ {SetStartNow}
    [] candidate = SnapshotNoKeyZero /\
          Bug = "snapshot_no_key_has_active_gap" ->
      (spec \ {SnapshotZeroGaps}) \cup {SnapshotActivePhaseElapsed}
    [] candidate = SnapshotActiveElapsed /\
          Bug = "snapshot_active_phase_missing" ->
      spec \ {SnapshotActivePhaseElapsed}
    [] candidate = SnapshotKeepsCompleted /\
          Bug = "snapshot_drops_completed_gaps" ->
      spec \ {PreserveCompletedGaps}
    [] candidate = RecordTraceAppendsLatest /\
          Bug = "record_trace_drops_entry" ->
      spec \ {AppendEntry}
    [] candidate = RecordTraceAppendsLatest /\
          Bug = "record_trace_latest_stale" ->
      spec \ {SetLatestEntry}
    [] candidate = RecordTracePrunesOldest /\
          Bug = "record_trace_keeps_over_cap" ->
      spec \ {PruneOldest}
    [] candidate = RecordTracePrunesOldest /\
          Bug = "record_trace_prunes_newest" ->
      spec \ {PreserveNewestEntries}
    [] candidate = RecordTraceSnapshotProjects /\
          Bug = "record_trace_snapshot_drops_entries" ->
      spec \ {ProjectSnapshotEntries}
    [] candidate = ResetClearsTrace /\
          Bug = "reset_keeps_latest" ->
      spec \ {ResetTraceState}
    [] candidate = RequestRoundExactMetadata /\
          Bug = "request_round_wrong_key" ->
      spec \ {SetKeyInput}
    [] candidate = RequestRoundExactMetadata /\
          Bug = "request_round_drops_latency" ->
      spec \ {PreserveQueueLatency}
    [] candidate = RequestRoundWakesPipeline /\
          Bug = "request_round_no_wake" ->
      spec \ {WakePipeline}
    [] candidate = RequestPendingAbsentOnlyWakes /\
          Bug = "request_pending_absent_records_trace" ->
      (spec \ {NoTraceRecorded}) \cup {AppendEntry}
    [] candidate = RequestPendingPresentInfersPhase /\
          Bug = "request_pending_present_not_inferred" ->
      spec \ {InferPhaseFromPending}
    [] candidate = RequestPendingPreservesLatency /\
          Bug = "request_pending_drops_latency" ->
      spec \ {PreserveQueueLatency}
    [] candidate = NoProgressAbsentNoTrace /\
          Bug = "no_progress_absent_records_trace" ->
      (spec \ {NoTraceRecorded}) \cup {AppendEntry}
    [] candidate = NoProgressPresentRecordsFlag /\
          Bug = "no_progress_present_drops_flag" ->
      spec \ {SetNoProgressFlag}
    [] candidate = EntryCopiesQueueState /\
          Bug = "entry_queue_state_not_copied" ->
      spec \ {CopyPendingCounts, CopyBackpressure, CopyQueueDepths}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "transition_new_key_returns_previous",
       "transition_new_key_keeps_old_phase",
       "transition_new_key_no_key",
       "transition_new_key_keeps_old_gaps",
       "transition_same_phase_returns_none",
       "transition_same_phase_resets_state",
       "transition_phase_change_returns_next",
       "transition_phase_change_skips_gap",
       "transition_phase_change_writes_next_gap",
       "transition_phase_change_keeps_start",
       "snapshot_no_key_has_active_gap",
       "snapshot_active_phase_missing",
       "snapshot_drops_completed_gaps",
       "record_trace_drops_entry",
       "record_trace_latest_stale",
       "record_trace_keeps_over_cap",
       "record_trace_prunes_newest",
       "record_trace_snapshot_drops_entries",
       "reset_keeps_latest",
       "request_round_wrong_key",
       "request_round_drops_latency",
       "request_round_no_wake",
       "request_pending_absent_records_trace",
       "request_pending_present_not_inferred",
       "request_pending_drops_latency",
       "no_progress_absent_records_trace",
       "no_progress_present_drops_flag",
       "entry_queue_state_not_copied"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

RoundTraceStatusActionsMatchSpec ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

RoundTraceStatusExactness ==
  /\ RoundTraceStatusActionsMatchSpec

Safety ==
  RoundTraceStatusExactness

RoundTraceStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RoundTraceStatusExactness

BugTransitionNewKeyReturnsPrevious ==
  ImplementationActions(TransitionNewKeyNoPrevious) =
    SpecActions(TransitionNewKeyNoPrevious)

BugTransitionNewKeyKeepsOldPhase ==
  ImplementationActions(TransitionNewKeySetsKeyPhase) =
    SpecActions(TransitionNewKeySetsKeyPhase)

BugTransitionNewKeyNoKey ==
  ImplementationActions(TransitionNewKeySetsKeyPhase) =
    SpecActions(TransitionNewKeySetsKeyPhase)

BugTransitionNewKeyKeepsOldGaps ==
  ImplementationActions(TransitionNewKeyResetsGaps) =
    SpecActions(TransitionNewKeyResetsGaps)

BugTransitionSamePhaseReturnsNone ==
  ImplementationActions(TransitionSamePhaseReturnsPrevious) =
    SpecActions(TransitionSamePhaseReturnsPrevious)

BugTransitionSamePhaseResetsState ==
  ImplementationActions(TransitionSamePhasePreservesState) =
    SpecActions(TransitionSamePhasePreservesState)

BugTransitionPhaseChangeReturnsNext ==
  ImplementationActions(TransitionPhaseChangeReturnsPrevious) =
    SpecActions(TransitionPhaseChangeReturnsPrevious)

BugTransitionPhaseChangeSkipsGap ==
  ImplementationActions(TransitionPhaseChangeRecordsOldGap) =
    SpecActions(TransitionPhaseChangeRecordsOldGap)

BugTransitionPhaseChangeWritesNextGap ==
  ImplementationActions(TransitionPhaseChangeRecordsOldGap) =
    SpecActions(TransitionPhaseChangeRecordsOldGap)

BugTransitionPhaseChangeKeepsStart ==
  ImplementationActions(TransitionPhaseChangeSetsNext) =
    SpecActions(TransitionPhaseChangeSetsNext)

BugSnapshotNoKeyHasActiveGap ==
  ImplementationActions(SnapshotNoKeyZero) = SpecActions(SnapshotNoKeyZero)

BugSnapshotActivePhaseMissing ==
  ImplementationActions(SnapshotActiveElapsed) =
    SpecActions(SnapshotActiveElapsed)

BugSnapshotDropsCompletedGaps ==
  ImplementationActions(SnapshotKeepsCompleted) =
    SpecActions(SnapshotKeepsCompleted)

BugRecordTraceDropsEntry ==
  ImplementationActions(RecordTraceAppendsLatest) =
    SpecActions(RecordTraceAppendsLatest)

BugRecordTraceLatestStale ==
  ImplementationActions(RecordTraceAppendsLatest) =
    SpecActions(RecordTraceAppendsLatest)

BugRecordTraceKeepsOverCap ==
  ImplementationActions(RecordTracePrunesOldest) =
    SpecActions(RecordTracePrunesOldest)

BugRecordTracePrunesNewest ==
  ImplementationActions(RecordTracePrunesOldest) =
    SpecActions(RecordTracePrunesOldest)

BugRecordTraceSnapshotDropsEntries ==
  ImplementationActions(RecordTraceSnapshotProjects) =
    SpecActions(RecordTraceSnapshotProjects)

BugResetKeepsLatest ==
  ImplementationActions(ResetClearsTrace) = SpecActions(ResetClearsTrace)

BugRequestRoundWrongKey ==
  ImplementationActions(RequestRoundExactMetadata) =
    SpecActions(RequestRoundExactMetadata)

BugRequestRoundDropsLatency ==
  ImplementationActions(RequestRoundExactMetadata) =
    SpecActions(RequestRoundExactMetadata)

BugRequestRoundNoWake ==
  ImplementationActions(RequestRoundWakesPipeline) =
    SpecActions(RequestRoundWakesPipeline)

BugRequestPendingAbsentRecordsTrace ==
  ImplementationActions(RequestPendingAbsentOnlyWakes) =
    SpecActions(RequestPendingAbsentOnlyWakes)

BugRequestPendingPresentNotInferred ==
  ImplementationActions(RequestPendingPresentInfersPhase) =
    SpecActions(RequestPendingPresentInfersPhase)

BugRequestPendingDropsLatency ==
  ImplementationActions(RequestPendingPreservesLatency) =
    SpecActions(RequestPendingPreservesLatency)

BugNoProgressAbsentRecordsTrace ==
  ImplementationActions(NoProgressAbsentNoTrace) =
    SpecActions(NoProgressAbsentNoTrace)

BugNoProgressPresentDropsFlag ==
  ImplementationActions(NoProgressPresentRecordsFlag) =
    SpecActions(NoProgressPresentRecordsFlag)

BugEntryQueueStateNotCopied ==
  ImplementationActions(EntryCopiesQueueState) =
    SpecActions(EntryCopiesQueueState)

====
