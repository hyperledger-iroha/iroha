---- MODULE SumeragiTelemetryStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi telemetry/status projection helpers.

This slice pins the status-facing helper surface in `status.rs` for
`record_availability_vote(...)`, `availability_snapshot()`,
`record_qc_latency(...)`, `qc_latency_snapshot()`,
`set_rbc_backlog_snapshot(...)`, direct RBC lane/dataspace backlog projection,
and the block-pipeline activity fields projected through `snapshot()`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

AvailabilityResetClears == 1
AvailabilityFirstVoteRecords == 2
AvailabilityRepeatedVoteAccumulates == 3
AvailabilityCollectorIndexRefreshes == 4
AvailabilityDistinctPeersIndependent == 5
AvailabilitySnapshotSortedByCollector == 6
AvailabilityTotalSaturates == 7
QcLatencyResetClears == 8
QcLatencyRecordsKind == 9
QcLatencyOverwriteSameKind == 10
QcLatencyKindsIndependent == 11
QcLatencySnapshotSortedByKind == 12
RbcBacklogResetClears == 13
RbcBacklogSetProjects == 14
RbcLaneBacklogProjects == 15
RbcDataspaceBacklogProjects == 16
StatusSnapshotProjectsBacklogs == 17
PipelineExecutionProjects == 18
PipelineConflictRateProjects == 19
LaneActivityProjects == 20
DataspaceActivityProjects == 21
AccessSetSourceProjects == 22
ResetClearsPipelineAndConflict == 23
ResetClearsActivityVectors == 24
ResetClearsAccessAndRbcVectors == 25

Candidates == 1..25

ResetAvailabilityTotal == 1
ResetAvailabilityPeers == 2
AvailabilityTotalIncrement == 3
AvailabilityPeerInsert == 4
AvailabilityPeerVoteIncrement == 5
AvailabilityPeerVoteAccumulate == 6
AvailabilityPeerIndexStored == 7
AvailabilityPeerIndexRefresh == 8
AvailabilityDistinctPeers == 9
AvailabilitySnapshotTotal == 10
AvailabilitySnapshotPeerSet == 11
AvailabilitySnapshotPeerVotes == 12
AvailabilitySnapshotCollectorIndex == 13
AvailabilitySortedByCollector == 14
AvailabilityTotalSaturatesAction == 15
ResetQcLatency == 16
QcLatencySnapshotEmpty == 17
QcLatencyInsert == 18
QcLatencyOverwrite == 19
QcLatencyKindsIndependentAction == 20
QcLatencySnapshotProjects == 21
QcLatencySnapshotSortedByKindAction == 22
ResetRbcBacklogSummary == 23
ResetRbcLaneBacklog == 24
ResetRbcDataspaceBacklog == 25
ResetPipelineExecution == 26
ResetPipelineConflict == 27
ResetLaneActivity == 28
ResetDataspaceActivity == 29
ResetAccessSetSource == 30
SetRbcBacklogSummary == 31
RbcBacklogSnapshotProjects == 32
SetRbcLaneBacklog == 33
StatusSnapshotRbcLaneProjects == 34
SetRbcDataspaceBacklog == 35
StatusSnapshotRbcDataspaceProjects == 36
SetPipelineExecution == 37
StatusSnapshotPipelineExecution == 38
SetPipelineConflict == 39
StatusSnapshotPipelineConflict == 40
SetLaneActivity == 41
StatusSnapshotLaneActivity == 42
SetDataspaceActivity == 43
StatusSnapshotDataspaceActivity == 44
SetAccessSetSource == 45
StatusSnapshotAccessSetSource == 46

Actions == 1..46

RbcResetActions ==
  {ResetRbcBacklogSummary, ResetRbcLaneBacklog, ResetRbcDataspaceBacklog,
   ResetPipelineExecution, ResetPipelineConflict, ResetLaneActivity,
   ResetDataspaceActivity, ResetAccessSetSource}

SpecActions(candidate) ==
  CASE candidate = AvailabilityResetClears ->
      {ResetAvailabilityTotal, ResetAvailabilityPeers,
       AvailabilitySnapshotTotal, AvailabilitySnapshotPeerSet}
    [] candidate = AvailabilityFirstVoteRecords ->
      {AvailabilityTotalIncrement, AvailabilityPeerInsert,
       AvailabilityPeerVoteIncrement, AvailabilityPeerIndexStored,
       AvailabilitySnapshotTotal, AvailabilitySnapshotPeerVotes,
       AvailabilitySnapshotCollectorIndex}
    [] candidate = AvailabilityRepeatedVoteAccumulates ->
      {AvailabilityTotalIncrement, AvailabilityPeerVoteAccumulate,
       AvailabilitySnapshotTotal, AvailabilitySnapshotPeerVotes}
    [] candidate = AvailabilityCollectorIndexRefreshes ->
      {AvailabilityPeerIndexRefresh, AvailabilitySnapshotCollectorIndex}
    [] candidate = AvailabilityDistinctPeersIndependent ->
      {AvailabilityDistinctPeers, AvailabilitySnapshotPeerSet}
    [] candidate = AvailabilitySnapshotSortedByCollector ->
      {AvailabilitySortedByCollector}
    [] candidate = AvailabilityTotalSaturates ->
      {AvailabilityTotalSaturatesAction, AvailabilitySnapshotTotal}
    [] candidate = QcLatencyResetClears ->
      {ResetQcLatency, QcLatencySnapshotEmpty}
    [] candidate = QcLatencyRecordsKind ->
      {QcLatencyInsert, QcLatencySnapshotProjects}
    [] candidate = QcLatencyOverwriteSameKind ->
      {QcLatencyOverwrite, QcLatencySnapshotProjects}
    [] candidate = QcLatencyKindsIndependent ->
      {QcLatencyKindsIndependentAction, QcLatencySnapshotProjects}
    [] candidate = QcLatencySnapshotSortedByKind ->
      {QcLatencySnapshotSortedByKindAction}
    [] candidate = RbcBacklogResetClears ->
      RbcResetActions
    [] candidate = RbcBacklogSetProjects ->
      {SetRbcBacklogSummary, RbcBacklogSnapshotProjects}
    [] candidate = RbcLaneBacklogProjects ->
      {SetRbcLaneBacklog, StatusSnapshotRbcLaneProjects}
    [] candidate = RbcDataspaceBacklogProjects ->
      {SetRbcDataspaceBacklog, StatusSnapshotRbcDataspaceProjects}
    [] candidate = StatusSnapshotProjectsBacklogs ->
      {StatusSnapshotRbcLaneProjects, StatusSnapshotRbcDataspaceProjects}
    [] candidate = PipelineExecutionProjects ->
      {SetPipelineExecution, StatusSnapshotPipelineExecution}
    [] candidate = PipelineConflictRateProjects ->
      {SetPipelineConflict, StatusSnapshotPipelineConflict}
    [] candidate = LaneActivityProjects ->
      {SetLaneActivity, StatusSnapshotLaneActivity}
    [] candidate = DataspaceActivityProjects ->
      {SetDataspaceActivity, StatusSnapshotDataspaceActivity}
    [] candidate = AccessSetSourceProjects ->
      {SetAccessSetSource, StatusSnapshotAccessSetSource}
    [] candidate = ResetClearsPipelineAndConflict ->
      {ResetPipelineExecution, ResetPipelineConflict,
       StatusSnapshotPipelineExecution, StatusSnapshotPipelineConflict}
    [] candidate = ResetClearsActivityVectors ->
      {ResetLaneActivity, ResetDataspaceActivity,
       StatusSnapshotLaneActivity, StatusSnapshotDataspaceActivity}
    [] candidate = ResetClearsAccessAndRbcVectors ->
      {ResetAccessSetSource, ResetRbcLaneBacklog, ResetRbcDataspaceBacklog,
       StatusSnapshotAccessSetSource, StatusSnapshotRbcLaneProjects,
       StatusSnapshotRbcDataspaceProjects}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = AvailabilityResetClears /\
          Bug = "reset_availability_keeps_total" ->
      spec \ {ResetAvailabilityTotal, AvailabilitySnapshotTotal}
    [] candidate = AvailabilityResetClears /\
          Bug = "reset_availability_keeps_peers" ->
      spec \ {ResetAvailabilityPeers, AvailabilitySnapshotPeerSet}
    [] candidate = AvailabilityFirstVoteRecords /\
          Bug = "availability_total_not_counted" ->
      spec \ {AvailabilityTotalIncrement, AvailabilitySnapshotTotal}
    [] candidate = AvailabilityFirstVoteRecords /\
          Bug = "availability_peer_vote_not_counted" ->
      spec \ {AvailabilityPeerVoteIncrement, AvailabilitySnapshotPeerVotes}
    [] candidate = AvailabilityRepeatedVoteAccumulates /\
          Bug = "availability_peer_votes_overwrite" ->
      spec \ {AvailabilityPeerVoteAccumulate, AvailabilitySnapshotPeerVotes}
    [] candidate = AvailabilityCollectorIndexRefreshes /\
          Bug = "availability_idx_not_updated" ->
      spec \ {AvailabilityPeerIndexRefresh, AvailabilitySnapshotCollectorIndex}
    [] candidate = AvailabilityDistinctPeersIndependent /\
          Bug = "availability_distinct_peers_collide" ->
      spec \ {AvailabilityDistinctPeers, AvailabilitySnapshotPeerSet}
    [] candidate = AvailabilitySnapshotSortedByCollector /\
          Bug = "availability_snapshot_not_sorted" ->
      spec \ {AvailabilitySortedByCollector}
    [] candidate = AvailabilityTotalSaturates /\
          Bug = "availability_total_overflows" ->
      spec \ {AvailabilityTotalSaturatesAction}
    [] candidate = QcLatencyResetClears /\
          Bug = "reset_qc_latency_keeps_entries" ->
      spec \ {ResetQcLatency, QcLatencySnapshotEmpty}
    [] candidate = QcLatencyRecordsKind /\
          Bug = "qc_latency_not_recorded" ->
      spec \ {QcLatencyInsert, QcLatencySnapshotProjects}
    [] candidate = QcLatencyOverwriteSameKind /\
          Bug = "qc_latency_overwrite_ignored" ->
      spec \ {QcLatencyOverwrite, QcLatencySnapshotProjects}
    [] candidate = QcLatencyKindsIndependent /\
          Bug = "qc_latency_kinds_collide" ->
      spec \ {QcLatencyKindsIndependentAction, QcLatencySnapshotProjects}
    [] candidate = QcLatencySnapshotSortedByKind /\
          Bug = "qc_latency_snapshot_not_sorted" ->
      spec \ {QcLatencySnapshotSortedByKindAction}
    [] candidate = RbcBacklogResetClears /\
          Bug = "reset_backlog_keeps_summary" ->
      spec \ {ResetRbcBacklogSummary}
    [] candidate = RbcBacklogSetProjects /\
          Bug = "rbc_backlog_summary_mismatch" ->
      spec \ {RbcBacklogSnapshotProjects}
    [] candidate = RbcLaneBacklogProjects /\
          Bug = "rbc_lane_backlog_dropped" ->
      spec \ {StatusSnapshotRbcLaneProjects}
    [] candidate = RbcDataspaceBacklogProjects /\
          Bug = "rbc_dataspace_backlog_dropped" ->
      spec \ {StatusSnapshotRbcDataspaceProjects}
    [] candidate = StatusSnapshotProjectsBacklogs /\
          Bug = "status_snapshot_drops_backlogs" ->
      spec \ {StatusSnapshotRbcLaneProjects,
              StatusSnapshotRbcDataspaceProjects}
    [] candidate = PipelineExecutionProjects /\
          Bug = "pipeline_execution_dropped" ->
      spec \ {StatusSnapshotPipelineExecution}
    [] candidate = PipelineConflictRateProjects /\
          Bug = "conflict_rate_dropped" ->
      spec \ {StatusSnapshotPipelineConflict}
    [] candidate = LaneActivityProjects /\
          Bug = "lane_activity_dropped" ->
      spec \ {StatusSnapshotLaneActivity}
    [] candidate = DataspaceActivityProjects /\
          Bug = "dataspace_activity_dropped" ->
      spec \ {StatusSnapshotDataspaceActivity}
    [] candidate = AccessSetSourceProjects /\
          Bug = "access_set_summary_dropped" ->
      spec \ {StatusSnapshotAccessSetSource}
    [] candidate = ResetClearsPipelineAndConflict /\
          Bug = "reset_keeps_pipeline_execution" ->
      spec \ {ResetPipelineExecution, StatusSnapshotPipelineExecution}
    [] candidate = ResetClearsActivityVectors /\
          Bug = "reset_keeps_activity" ->
      spec \ {ResetLaneActivity, ResetDataspaceActivity,
              StatusSnapshotLaneActivity, StatusSnapshotDataspaceActivity}
    [] candidate = ResetClearsAccessAndRbcVectors /\
          Bug = "reset_keeps_access_set_and_rbc_vectors" ->
      spec \ {ResetAccessSetSource, ResetRbcLaneBacklog,
              ResetRbcDataspaceBacklog, StatusSnapshotAccessSetSource,
              StatusSnapshotRbcLaneProjects,
              StatusSnapshotRbcDataspaceProjects}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 25
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..25

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetAvailabilityKeepsTotal ==
  ImplementationActions(AvailabilityResetClears) =
    SpecActions(AvailabilityResetClears)

BugResetAvailabilityKeepsPeers ==
  ImplementationActions(AvailabilityResetClears) =
    SpecActions(AvailabilityResetClears)

BugAvailabilityTotalNotCounted ==
  ImplementationActions(AvailabilityFirstVoteRecords) =
    SpecActions(AvailabilityFirstVoteRecords)

BugAvailabilityPeerVoteNotCounted ==
  ImplementationActions(AvailabilityFirstVoteRecords) =
    SpecActions(AvailabilityFirstVoteRecords)

BugAvailabilityPeerVotesOverwrite ==
  ImplementationActions(AvailabilityRepeatedVoteAccumulates) =
    SpecActions(AvailabilityRepeatedVoteAccumulates)

BugAvailabilityIdxNotUpdated ==
  ImplementationActions(AvailabilityCollectorIndexRefreshes) =
    SpecActions(AvailabilityCollectorIndexRefreshes)

BugAvailabilityDistinctPeersCollide ==
  ImplementationActions(AvailabilityDistinctPeersIndependent) =
    SpecActions(AvailabilityDistinctPeersIndependent)

BugAvailabilitySnapshotNotSorted ==
  ImplementationActions(AvailabilitySnapshotSortedByCollector) =
    SpecActions(AvailabilitySnapshotSortedByCollector)

BugAvailabilityTotalOverflows ==
  ImplementationActions(AvailabilityTotalSaturates) =
    SpecActions(AvailabilityTotalSaturates)

BugResetQcLatencyKeepsEntries ==
  ImplementationActions(QcLatencyResetClears) =
    SpecActions(QcLatencyResetClears)

BugQcLatencyNotRecorded ==
  ImplementationActions(QcLatencyRecordsKind) =
    SpecActions(QcLatencyRecordsKind)

BugQcLatencyOverwriteIgnored ==
  ImplementationActions(QcLatencyOverwriteSameKind) =
    SpecActions(QcLatencyOverwriteSameKind)

BugQcLatencyKindsCollide ==
  ImplementationActions(QcLatencyKindsIndependent) =
    SpecActions(QcLatencyKindsIndependent)

BugQcLatencySnapshotNotSorted ==
  ImplementationActions(QcLatencySnapshotSortedByKind) =
    SpecActions(QcLatencySnapshotSortedByKind)

BugResetBacklogKeepsSummary ==
  ImplementationActions(RbcBacklogResetClears) =
    SpecActions(RbcBacklogResetClears)

BugRbcBacklogSummaryMismatch ==
  ImplementationActions(RbcBacklogSetProjects) =
    SpecActions(RbcBacklogSetProjects)

BugRbcLaneBacklogDropped ==
  ImplementationActions(RbcLaneBacklogProjects) =
    SpecActions(RbcLaneBacklogProjects)

BugRbcDataspaceBacklogDropped ==
  ImplementationActions(RbcDataspaceBacklogProjects) =
    SpecActions(RbcDataspaceBacklogProjects)

BugStatusSnapshotDropsBacklogs ==
  ImplementationActions(StatusSnapshotProjectsBacklogs) =
    SpecActions(StatusSnapshotProjectsBacklogs)

BugPipelineExecutionDropped ==
  ImplementationActions(PipelineExecutionProjects) =
    SpecActions(PipelineExecutionProjects)

BugConflictRateDropped ==
  ImplementationActions(PipelineConflictRateProjects) =
    SpecActions(PipelineConflictRateProjects)

BugLaneActivityDropped ==
  ImplementationActions(LaneActivityProjects) =
    SpecActions(LaneActivityProjects)

BugDataspaceActivityDropped ==
  ImplementationActions(DataspaceActivityProjects) =
    SpecActions(DataspaceActivityProjects)

BugAccessSetSummaryDropped ==
  ImplementationActions(AccessSetSourceProjects) =
    SpecActions(AccessSetSourceProjects)

BugResetKeepsPipelineExecution ==
  ImplementationActions(ResetClearsPipelineAndConflict) =
    SpecActions(ResetClearsPipelineAndConflict)

BugResetKeepsActivity ==
  ImplementationActions(ResetClearsActivityVectors) =
    SpecActions(ResetClearsActivityVectors)

BugResetKeepsAccessSetAndRbcVectors ==
  ImplementationActions(ResetClearsAccessAndRbcVectors) =
    SpecActions(ResetClearsAccessAndRbcVectors)

=============================================================================
