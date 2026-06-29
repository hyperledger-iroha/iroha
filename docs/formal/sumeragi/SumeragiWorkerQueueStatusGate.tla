---- MODULE SumeragiWorkerQueueStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi worker-queue status accounting.

This slice pins `WorkerQueueKind` counter routing,
`record_worker_queue_enqueue(...)`, `record_worker_queue_drain(...)`,
`record_worker_queue_blocked(...)`, `record_worker_queue_drop(...)`,
`worker_queue_depth_snapshot()`, `worker_queue_diagnostics_snapshot()`,
worker stage/iteration markers, reset behavior, and the commit inflight
pause/resume queue-depth snapshots.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

VotesMapsCounter == 1
PayloadMapsCounter == 2
RbcMapsCounter == 3
BlocksMapsCounter == 4
ConsensusMapsCounter == 5
LaneMapsCounter == 6
BackgroundMapsCounter == 7
EnqueueIncrementsDepth == 8
DrainZeroNoop == 9
DrainOneDecrements == 10
DrainManyDecrementsDelta == 11
DrainOverDepthSaturates == 12
BlockedZeroNoop == 13
BlockedPositiveRecords == 14
BlockedMaxKeepsHigher == 15
BlockedMaxRaisesLower == 16
DropIncrementsTotal == 17
DepthSnapshotReadsAllQueues == 18
DiagnosticsSnapshotReadsAllFamilies == 19
WorkerStageSet == 20
WorkerIterationSet == 21
ResetClearsWorkerLoop == 22
CommitPauseCapturesDepths == 23
CommitResumeCapturesDepths == 24
CommitFinishWrongIdNoop == 25

Candidates == 1..25

MapVotesCounter == 1
MapPayloadCounter == 2
MapRbcCounter == 3
MapBlocksCounter == 4
MapConsensusCounter == 5
MapLaneCounter == 6
MapBackgroundCounter == 7
EnqueueDepthAdd == 8
EnqueueUsesKindCounter == 9
DrainZeroNoopAction == 10
DrainDrainedToU64 == 11
DrainSaturatingSub == 12
DrainManySubtractsDelta == 13
BlockedZeroNoopAction == 14
BlockedTotalAdd == 15
BlockedMsAdd == 16
BlockedMaxUpdate == 17
BlockedMaxKeepsHigherAction == 18
DropTotalAdd == 19
SnapshotReadsDepths == 20
SnapshotReadsAllQueues == 21
DiagnosticsReadsBlockedTotal == 22
DiagnosticsReadsBlockedMs == 23
DiagnosticsReadsBlockedMax == 24
DiagnosticsReadsDropped == 25
StageIdStored == 26
StageTimestampStored == 27
IterationStored == 28
ResetDepthCounters == 29
ResetDiagnosticsCounters == 30
ResetStageAndIteration == 31
PauseTotalIncrement == 32
PauseCapturesQueueDepths == 33
ResumeTotalIncrement == 34
ResumeCapturesQueueDepths == 35
FinishChecksActive == 36
FinishChecksId == 37
WrongCounterSelected == 38
DrainUnderflows == 39
CountZeroBlocked == 40
BlockedMaxDecreases == 41
SnapshotDropsQueue == 42
DropResetOnlyDepth == 43
PauseUsesDefaultDepths == 44
ResumeSkipsDepths == 45
FinishWrongIdMutates == 46

Actions == 1..46

QueueMappingActions ==
  {MapVotesCounter, MapPayloadCounter, MapRbcCounter, MapBlocksCounter,
   MapConsensusCounter, MapLaneCounter, MapBackgroundCounter}

SpecActions(candidate) ==
  CASE candidate = VotesMapsCounter ->
      {MapVotesCounter}
    [] candidate = PayloadMapsCounter ->
      {MapPayloadCounter}
    [] candidate = RbcMapsCounter ->
      {MapRbcCounter}
    [] candidate = BlocksMapsCounter ->
      {MapBlocksCounter}
    [] candidate = ConsensusMapsCounter ->
      {MapConsensusCounter}
    [] candidate = LaneMapsCounter ->
      {MapLaneCounter}
    [] candidate = BackgroundMapsCounter ->
      {MapBackgroundCounter}
    [] candidate = EnqueueIncrementsDepth ->
      {MapVotesCounter, EnqueueUsesKindCounter, EnqueueDepthAdd}
    [] candidate = DrainZeroNoop ->
      {MapVotesCounter, DrainZeroNoopAction}
    [] candidate = DrainOneDecrements ->
      {MapVotesCounter, DrainDrainedToU64, DrainSaturatingSub}
    [] candidate = DrainManyDecrementsDelta ->
      {MapVotesCounter, DrainDrainedToU64, DrainSaturatingSub,
       DrainManySubtractsDelta}
    [] candidate = DrainOverDepthSaturates ->
      {MapVotesCounter, DrainDrainedToU64, DrainSaturatingSub}
    [] candidate = BlockedZeroNoop ->
      {MapVotesCounter, BlockedZeroNoopAction}
    [] candidate = BlockedPositiveRecords ->
      {MapVotesCounter, BlockedTotalAdd, BlockedMsAdd, BlockedMaxUpdate}
    [] candidate = BlockedMaxKeepsHigher ->
      {MapVotesCounter, BlockedMaxKeepsHigherAction}
    [] candidate = BlockedMaxRaisesLower ->
      {MapVotesCounter, BlockedMaxUpdate}
    [] candidate = DropIncrementsTotal ->
      {MapBackgroundCounter, DropTotalAdd}
    [] candidate = DepthSnapshotReadsAllQueues ->
      QueueMappingActions \cup {SnapshotReadsDepths, SnapshotReadsAllQueues}
    [] candidate = DiagnosticsSnapshotReadsAllFamilies ->
      {DiagnosticsReadsBlockedTotal, DiagnosticsReadsBlockedMs,
       DiagnosticsReadsBlockedMax, DiagnosticsReadsDropped}
    [] candidate = WorkerStageSet ->
      {StageIdStored, StageTimestampStored}
    [] candidate = WorkerIterationSet ->
      {IterationStored}
    [] candidate = ResetClearsWorkerLoop ->
      {ResetDepthCounters, ResetDiagnosticsCounters, ResetStageAndIteration}
    [] candidate = CommitPauseCapturesDepths ->
      {PauseTotalIncrement, PauseCapturesQueueDepths, SnapshotReadsDepths,
       SnapshotReadsAllQueues}
    [] candidate = CommitResumeCapturesDepths ->
      {ResumeTotalIncrement, ResumeCapturesQueueDepths, SnapshotReadsDepths,
       SnapshotReadsAllQueues}
    [] candidate = CommitFinishWrongIdNoop ->
      {FinishChecksActive, FinishChecksId}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = VotesMapsCounter /\ Bug = "vote_maps_to_payload" ->
      (spec \ {MapVotesCounter}) \cup {MapPayloadCounter}
    [] candidate = PayloadMapsCounter /\ Bug = "payload_maps_to_votes" ->
      (spec \ {MapPayloadCounter}) \cup {MapVotesCounter}
    [] candidate = RbcMapsCounter /\ Bug = "rbc_maps_to_blocks" ->
      (spec \ {MapRbcCounter}) \cup {MapBlocksCounter}
    [] candidate = BlocksMapsCounter /\ Bug = "blocks_maps_to_rbc" ->
      (spec \ {MapBlocksCounter}) \cup {MapRbcCounter}
    [] candidate = ConsensusMapsCounter /\
          Bug = "consensus_maps_to_background" ->
      (spec \ {MapConsensusCounter}) \cup {MapBackgroundCounter}
    [] candidate = LaneMapsCounter /\ Bug = "lane_maps_to_consensus" ->
      (spec \ {MapLaneCounter}) \cup {MapConsensusCounter}
    [] candidate = BackgroundMapsCounter /\ Bug = "background_maps_to_lane" ->
      (spec \ {MapBackgroundCounter}) \cup {MapLaneCounter}
    [] candidate = EnqueueIncrementsDepth /\ Bug = "enqueue_skips_depth" ->
      spec \ {EnqueueDepthAdd}
    [] candidate = EnqueueIncrementsDepth /\ Bug = "enqueue_wrong_counter" ->
      (spec \ {MapVotesCounter, EnqueueUsesKindCounter}) \cup
        {WrongCounterSelected}
    [] candidate = DrainZeroNoop /\ Bug = "drain_zero_decrements" ->
      (spec \ {DrainZeroNoopAction}) \cup {DrainSaturatingSub}
    [] candidate = DrainOverDepthSaturates /\ Bug = "drain_not_saturating" ->
      (spec \ {DrainSaturatingSub}) \cup {DrainUnderflows}
    [] candidate = DrainManyDecrementsDelta /\
          Bug = "drain_many_subtracts_one" ->
      spec \ {DrainManySubtractsDelta}
    [] candidate = BlockedZeroNoop /\ Bug = "blocked_zero_counted" ->
      (spec \ {BlockedZeroNoopAction}) \cup
        {BlockedTotalAdd, BlockedMsAdd, CountZeroBlocked}
    [] candidate = BlockedPositiveRecords /\ Bug = "blocked_total_missing" ->
      spec \ {BlockedTotalAdd}
    [] candidate = BlockedPositiveRecords /\ Bug = "blocked_ms_missing" ->
      spec \ {BlockedMsAdd}
    [] candidate = BlockedMaxRaisesLower /\ Bug = "blocked_max_not_updated" ->
      spec \ {BlockedMaxUpdate}
    [] candidate = BlockedMaxKeepsHigher /\ Bug = "blocked_max_decreases" ->
      (spec \ {BlockedMaxKeepsHigherAction}) \cup {BlockedMaxDecreases}
    [] candidate = DropIncrementsTotal /\ Bug = "drop_missing" ->
      spec \ {DropTotalAdd}
    [] candidate = DepthSnapshotReadsAllQueues /\ Bug = "snapshot_drops_queue" ->
      (spec \ {SnapshotReadsAllQueues}) \cup {SnapshotDropsQueue}
    [] candidate = DiagnosticsSnapshotReadsAllFamilies /\
          Bug = "diagnostics_drops_blocked_ms" ->
      spec \ {DiagnosticsReadsBlockedMs}
    [] candidate = WorkerStageSet /\ Bug = "stage_not_recorded" ->
      spec \ {StageIdStored, StageTimestampStored}
    [] candidate = WorkerIterationSet /\ Bug = "iteration_not_recorded" ->
      spec \ {IterationStored}
    [] candidate = ResetClearsWorkerLoop /\ Bug = "reset_keeps_diagnostics" ->
      (spec \ {ResetDiagnosticsCounters}) \cup {DropResetOnlyDepth}
    [] candidate = CommitPauseCapturesDepths /\
          Bug = "pause_snapshot_uses_defaults" ->
      (spec \ {PauseCapturesQueueDepths}) \cup {PauseUsesDefaultDepths}
    [] candidate = CommitResumeCapturesDepths /\
          Bug = "resume_snapshot_not_recorded" ->
      (spec \ {ResumeCapturesQueueDepths}) \cup {ResumeSkipsDepths}
    [] candidate = CommitFinishWrongIdNoop /\
          Bug = "finish_wrong_id_mutates" ->
      (spec \ {FinishChecksId}) \cup {FinishWrongIdMutates}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "vote_maps_to_payload",
       "payload_maps_to_votes",
       "rbc_maps_to_blocks",
       "blocks_maps_to_rbc",
       "consensus_maps_to_background",
       "lane_maps_to_consensus",
       "background_maps_to_lane",
       "enqueue_skips_depth",
       "enqueue_wrong_counter",
       "drain_zero_decrements",
       "drain_not_saturating",
       "drain_many_subtracts_one",
       "blocked_zero_counted",
       "blocked_total_missing",
       "blocked_ms_missing",
       "blocked_max_not_updated",
       "blocked_max_decreases",
       "drop_missing",
       "snapshot_drops_queue",
       "diagnostics_drops_blocked_ms",
       "stage_not_recorded",
       "iteration_not_recorded",
       "reset_keeps_diagnostics",
       "pause_snapshot_uses_defaults",
       "resume_snapshot_not_recorded",
       "finish_wrong_id_mutates"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

WorkerQueueStatusActionsMatchSpec ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

WorkerQueueStatusExactness ==
  /\ WorkerQueueStatusActionsMatchSpec

Safety ==
  WorkerQueueStatusExactness

WorkerQueueStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ WorkerQueueStatusExactness

BugVoteMapsToPayload ==
  ImplementationActions(VotesMapsCounter) = SpecActions(VotesMapsCounter)

BugPayloadMapsToVotes ==
  ImplementationActions(PayloadMapsCounter) = SpecActions(PayloadMapsCounter)

BugRbcMapsToBlocks ==
  ImplementationActions(RbcMapsCounter) = SpecActions(RbcMapsCounter)

BugBlocksMapsToRbc ==
  ImplementationActions(BlocksMapsCounter) = SpecActions(BlocksMapsCounter)

BugConsensusMapsToBackground ==
  ImplementationActions(ConsensusMapsCounter) =
    SpecActions(ConsensusMapsCounter)

BugLaneMapsToConsensus ==
  ImplementationActions(LaneMapsCounter) = SpecActions(LaneMapsCounter)

BugBackgroundMapsToLane ==
  ImplementationActions(BackgroundMapsCounter) =
    SpecActions(BackgroundMapsCounter)

BugEnqueueSkipsDepth ==
  ImplementationActions(EnqueueIncrementsDepth) =
    SpecActions(EnqueueIncrementsDepth)

BugEnqueueWrongCounter ==
  ImplementationActions(EnqueueIncrementsDepth) =
    SpecActions(EnqueueIncrementsDepth)

BugDrainZeroDecrements ==
  ImplementationActions(DrainZeroNoop) = SpecActions(DrainZeroNoop)

BugDrainNotSaturating ==
  ImplementationActions(DrainOverDepthSaturates) =
    SpecActions(DrainOverDepthSaturates)

BugDrainManySubtractsOne ==
  ImplementationActions(DrainManyDecrementsDelta) =
    SpecActions(DrainManyDecrementsDelta)

BugBlockedZeroCounted ==
  ImplementationActions(BlockedZeroNoop) = SpecActions(BlockedZeroNoop)

BugBlockedTotalMissing ==
  ImplementationActions(BlockedPositiveRecords) =
    SpecActions(BlockedPositiveRecords)

BugBlockedMsMissing ==
  ImplementationActions(BlockedPositiveRecords) =
    SpecActions(BlockedPositiveRecords)

BugBlockedMaxNotUpdated ==
  ImplementationActions(BlockedMaxRaisesLower) =
    SpecActions(BlockedMaxRaisesLower)

BugBlockedMaxDecreases ==
  ImplementationActions(BlockedMaxKeepsHigher) =
    SpecActions(BlockedMaxKeepsHigher)

BugDropMissing ==
  ImplementationActions(DropIncrementsTotal) = SpecActions(DropIncrementsTotal)

BugSnapshotDropsQueue ==
  ImplementationActions(DepthSnapshotReadsAllQueues) =
    SpecActions(DepthSnapshotReadsAllQueues)

BugDiagnosticsDropsBlockedMs ==
  ImplementationActions(DiagnosticsSnapshotReadsAllFamilies) =
    SpecActions(DiagnosticsSnapshotReadsAllFamilies)

BugStageNotRecorded ==
  ImplementationActions(WorkerStageSet) = SpecActions(WorkerStageSet)

BugIterationNotRecorded ==
  ImplementationActions(WorkerIterationSet) = SpecActions(WorkerIterationSet)

BugResetKeepsDiagnostics ==
  ImplementationActions(ResetClearsWorkerLoop) =
    SpecActions(ResetClearsWorkerLoop)

BugPauseSnapshotUsesDefaults ==
  ImplementationActions(CommitPauseCapturesDepths) =
    SpecActions(CommitPauseCapturesDepths)

BugResumeSnapshotNotRecorded ==
  ImplementationActions(CommitResumeCapturesDepths) =
    SpecActions(CommitResumeCapturesDepths)

BugFinishWrongIdMutates ==
  ImplementationActions(CommitFinishWrongIdNoop) =
    SpecActions(CommitFinishWrongIdNoop)

=============================================================================
====
