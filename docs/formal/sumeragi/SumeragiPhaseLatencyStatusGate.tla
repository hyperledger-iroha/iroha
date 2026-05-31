---- MODULE SumeragiPhaseLatencyStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi phase-latency status projection.

This slice pins `set_phase_*_ms(...)`, `set_phase_*_ema_ms(...)`,
`set_phase_pipeline_total_ema_ms(...)`, `phase_latencies_snapshot()`, and the
phase fields reset by `reset_gossip_fallback_for_tests()`.  Pipeline totals
sum the core propose/DA/prevote/precommit/commit phases, intentionally exclude
collector fan-out latency, and saturate at `u64::MAX`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetClearsLatest == 1
ResetClearsMax == 2
ResetClearsEma == 3
ResetClearsPipelineEma == 4
ProposeLatestStored == 5
DaLatestStored == 6
PrevoteLatestStored == 7
PrecommitLatestStored == 8
AggregatorLatestStored == 9
CommitLatestStored == 10
MaxFirstObservationStored == 11
MaxKeepsHigherPrior == 12
MaxUpdatesOnHigher == 13
ProposeEmaStored == 14
AggregatorEmaStored == 15
PipelineEmaStored == 16
SnapshotProjectsLatest == 17
SnapshotProjectsMax == 18
SnapshotProjectsEma == 19
PipelineTotalCoreSum == 20
PipelineTotalExcludesAggregator == 21
PipelineTotalSaturates == 22
PipelineMaxCoreSum == 23
PipelineMaxExcludesAggregator == 24
PipelineMaxSaturates == 25
SnapshotProjectsPipelineEma == 26
SnapshotProjectsGossipFallback == 27
SnapshotProjectsBlockCreatedCounters == 28
ResetClearsBlockCreatedCounters == 29

Candidates == 1..29

ResetLatest == 1
ResetMax == 2
ResetEma == 3
ResetPipelineEma == 4
StoreLatest == 5
MaxRecordsFirst == 6
MaxPreservesHigher == 7
MaxRaisesHigher == 8
StoreEma == 9
StorePipelineEma == 10
SnapshotLatestMatches == 11
SnapshotMaxMatches == 12
SnapshotEmaMatches == 13
PipelineTotalSumsCore == 14
PipelineTotalNoAggregator == 15
PipelineTotalSaturatesAtU64 == 16
PipelineMaxSumsCore == 17
PipelineMaxNoAggregator == 18
PipelineMaxSaturatesAtU64 == 19
SnapshotPipelineEmaMatches == 20
SnapshotGossipMatches == 21
SnapshotBlockCreatedMatches == 22
ResetBlockCreatedCounters == 23

Actions == 1..23

SpecActions(candidate) ==
  CASE candidate = ResetClearsLatest ->
      {ResetLatest}
    [] candidate = ResetClearsMax ->
      {ResetMax}
    [] candidate = ResetClearsEma ->
      {ResetEma}
    [] candidate = ResetClearsPipelineEma ->
      {ResetPipelineEma}
    [] candidate = ProposeLatestStored ->
      {StoreLatest, SnapshotLatestMatches}
    [] candidate = DaLatestStored ->
      {StoreLatest, SnapshotLatestMatches}
    [] candidate = PrevoteLatestStored ->
      {StoreLatest, SnapshotLatestMatches}
    [] candidate = PrecommitLatestStored ->
      {StoreLatest, SnapshotLatestMatches}
    [] candidate = AggregatorLatestStored ->
      {StoreLatest, SnapshotLatestMatches}
    [] candidate = CommitLatestStored ->
      {StoreLatest, SnapshotLatestMatches}
    [] candidate = MaxFirstObservationStored ->
      {MaxRecordsFirst, SnapshotMaxMatches}
    [] candidate = MaxKeepsHigherPrior ->
      {MaxPreservesHigher, SnapshotMaxMatches}
    [] candidate = MaxUpdatesOnHigher ->
      {MaxRaisesHigher, SnapshotMaxMatches}
    [] candidate = ProposeEmaStored ->
      {StoreEma, SnapshotEmaMatches}
    [] candidate = AggregatorEmaStored ->
      {StoreEma, SnapshotEmaMatches}
    [] candidate = PipelineEmaStored ->
      {StorePipelineEma, SnapshotPipelineEmaMatches}
    [] candidate = SnapshotProjectsLatest ->
      {SnapshotLatestMatches}
    [] candidate = SnapshotProjectsMax ->
      {SnapshotMaxMatches}
    [] candidate = SnapshotProjectsEma ->
      {SnapshotEmaMatches}
    [] candidate = PipelineTotalCoreSum ->
      {PipelineTotalSumsCore}
    [] candidate = PipelineTotalExcludesAggregator ->
      {PipelineTotalNoAggregator}
    [] candidate = PipelineTotalSaturates ->
      {PipelineTotalSaturatesAtU64}
    [] candidate = PipelineMaxCoreSum ->
      {PipelineMaxSumsCore}
    [] candidate = PipelineMaxExcludesAggregator ->
      {PipelineMaxNoAggregator}
    [] candidate = PipelineMaxSaturates ->
      {PipelineMaxSaturatesAtU64}
    [] candidate = SnapshotProjectsPipelineEma ->
      {SnapshotPipelineEmaMatches}
    [] candidate = SnapshotProjectsGossipFallback ->
      {SnapshotGossipMatches}
    [] candidate = SnapshotProjectsBlockCreatedCounters ->
      {SnapshotBlockCreatedMatches}
    [] candidate = ResetClearsBlockCreatedCounters ->
      {ResetBlockCreatedCounters}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetClearsLatest /\
          Bug = "reset_keeps_latest" ->
      spec \ {ResetLatest}
    [] candidate = ResetClearsMax /\
          Bug = "reset_keeps_max" ->
      spec \ {ResetMax}
    [] candidate = ResetClearsEma /\
          Bug = "reset_keeps_ema" ->
      spec \ {ResetEma}
    [] candidate = ResetClearsPipelineEma /\
          Bug = "reset_keeps_pipeline_ema" ->
      spec \ {ResetPipelineEma}
    [] candidate = ProposeLatestStored /\
          Bug = "propose_latest_not_stored" ->
      spec \ {StoreLatest}
    [] candidate = DaLatestStored /\
          Bug = "da_latest_not_stored" ->
      spec \ {StoreLatest}
    [] candidate = PrevoteLatestStored /\
          Bug = "prevote_latest_not_stored" ->
      spec \ {StoreLatest}
    [] candidate = PrecommitLatestStored /\
          Bug = "precommit_latest_not_stored" ->
      spec \ {StoreLatest}
    [] candidate = AggregatorLatestStored /\
          Bug = "aggregator_latest_not_stored" ->
      spec \ {StoreLatest}
    [] candidate = CommitLatestStored /\
          Bug = "commit_latest_not_stored" ->
      spec \ {StoreLatest}
    [] candidate = MaxFirstObservationStored /\
          Bug = "max_first_missing" ->
      spec \ {MaxRecordsFirst}
    [] candidate = MaxKeepsHigherPrior /\
          Bug = "max_overwrites_lower" ->
      spec \ {MaxPreservesHigher}
    [] candidate = MaxUpdatesOnHigher /\
          Bug = "max_ignores_higher" ->
      spec \ {MaxRaisesHigher}
    [] candidate = ProposeEmaStored /\
          Bug = "ema_propose_not_stored" ->
      spec \ {StoreEma}
    [] candidate = AggregatorEmaStored /\
          Bug = "ema_aggregator_not_stored" ->
      spec \ {StoreEma}
    [] candidate = PipelineEmaStored /\
          Bug = "pipeline_ema_not_stored" ->
      spec \ {StorePipelineEma}
    [] candidate = SnapshotProjectsLatest /\
          Bug = "snapshot_drops_latest" ->
      spec \ {SnapshotLatestMatches}
    [] candidate = SnapshotProjectsMax /\
          Bug = "snapshot_drops_max" ->
      spec \ {SnapshotMaxMatches}
    [] candidate = SnapshotProjectsEma /\
          Bug = "snapshot_drops_ema" ->
      spec \ {SnapshotEmaMatches}
    [] candidate = PipelineTotalCoreSum /\
          Bug = "pipeline_total_omits_precommit" ->
      spec \ {PipelineTotalSumsCore}
    [] candidate = PipelineTotalExcludesAggregator /\
          Bug = "pipeline_total_includes_aggregator" ->
      spec \ {PipelineTotalNoAggregator}
    [] candidate = PipelineTotalSaturates /\
          Bug = "pipeline_total_overflows" ->
      spec \ {PipelineTotalSaturatesAtU64}
    [] candidate = PipelineMaxCoreSum /\
          Bug = "pipeline_max_omits_commit" ->
      spec \ {PipelineMaxSumsCore}
    [] candidate = PipelineMaxExcludesAggregator /\
          Bug = "pipeline_max_includes_aggregator" ->
      spec \ {PipelineMaxNoAggregator}
    [] candidate = PipelineMaxSaturates /\
          Bug = "pipeline_max_overflows" ->
      spec \ {PipelineMaxSaturatesAtU64}
    [] candidate = SnapshotProjectsPipelineEma /\
          Bug = "snapshot_pipeline_ema_mismatch" ->
      spec \ {SnapshotPipelineEmaMatches}
    [] candidate = SnapshotProjectsGossipFallback /\
          Bug = "snapshot_gossip_mismatch" ->
      spec \ {SnapshotGossipMatches}
    [] candidate = SnapshotProjectsBlockCreatedCounters /\
          Bug = "snapshot_block_created_mismatch" ->
      spec \ {SnapshotBlockCreatedMatches}
    [] candidate = ResetClearsBlockCreatedCounters /\
          Bug = "reset_keeps_block_created" ->
      spec \ {ResetBlockCreatedCounters}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reset_keeps_latest",
       "reset_keeps_max",
       "reset_keeps_ema",
       "reset_keeps_pipeline_ema",
       "propose_latest_not_stored",
       "da_latest_not_stored",
       "prevote_latest_not_stored",
       "precommit_latest_not_stored",
       "aggregator_latest_not_stored",
       "commit_latest_not_stored",
       "max_first_missing",
       "max_overwrites_lower",
       "max_ignores_higher",
       "ema_propose_not_stored",
       "ema_aggregator_not_stored",
       "pipeline_ema_not_stored",
       "snapshot_drops_latest",
       "snapshot_drops_max",
       "snapshot_drops_ema",
       "pipeline_total_omits_precommit",
       "pipeline_total_includes_aggregator",
       "pipeline_total_overflows",
       "pipeline_max_omits_commit",
       "pipeline_max_includes_aggregator",
       "pipeline_max_overflows",
       "snapshot_pipeline_ema_mismatch",
       "snapshot_gossip_mismatch",
       "snapshot_block_created_mismatch",
       "reset_keeps_block_created"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BugResetKeepsLatest ==
  ImplementationActions(ResetClearsLatest) = SpecActions(ResetClearsLatest)

BugResetKeepsMax ==
  ImplementationActions(ResetClearsMax) = SpecActions(ResetClearsMax)

BugResetKeepsEma ==
  ImplementationActions(ResetClearsEma) = SpecActions(ResetClearsEma)

BugResetKeepsPipelineEma ==
  ImplementationActions(ResetClearsPipelineEma) =
    SpecActions(ResetClearsPipelineEma)

BugProposeLatestNotStored ==
  ImplementationActions(ProposeLatestStored) = SpecActions(ProposeLatestStored)

BugDaLatestNotStored ==
  ImplementationActions(DaLatestStored) = SpecActions(DaLatestStored)

BugPrevoteLatestNotStored ==
  ImplementationActions(PrevoteLatestStored) = SpecActions(PrevoteLatestStored)

BugPrecommitLatestNotStored ==
  ImplementationActions(PrecommitLatestStored) =
    SpecActions(PrecommitLatestStored)

BugAggregatorLatestNotStored ==
  ImplementationActions(AggregatorLatestStored) =
    SpecActions(AggregatorLatestStored)

BugCommitLatestNotStored ==
  ImplementationActions(CommitLatestStored) = SpecActions(CommitLatestStored)

BugMaxFirstMissing ==
  ImplementationActions(MaxFirstObservationStored) =
    SpecActions(MaxFirstObservationStored)

BugMaxOverwritesLower ==
  ImplementationActions(MaxKeepsHigherPrior) =
    SpecActions(MaxKeepsHigherPrior)

BugMaxIgnoresHigher ==
  ImplementationActions(MaxUpdatesOnHigher) =
    SpecActions(MaxUpdatesOnHigher)

BugEmaProposeNotStored ==
  ImplementationActions(ProposeEmaStored) = SpecActions(ProposeEmaStored)

BugEmaAggregatorNotStored ==
  ImplementationActions(AggregatorEmaStored) =
    SpecActions(AggregatorEmaStored)

BugPipelineEmaNotStored ==
  ImplementationActions(PipelineEmaStored) = SpecActions(PipelineEmaStored)

BugSnapshotDropsLatest ==
  ImplementationActions(SnapshotProjectsLatest) =
    SpecActions(SnapshotProjectsLatest)

BugSnapshotDropsMax ==
  ImplementationActions(SnapshotProjectsMax) =
    SpecActions(SnapshotProjectsMax)

BugSnapshotDropsEma ==
  ImplementationActions(SnapshotProjectsEma) =
    SpecActions(SnapshotProjectsEma)

BugPipelineTotalOmitsPrecommit ==
  ImplementationActions(PipelineTotalCoreSum) =
    SpecActions(PipelineTotalCoreSum)

BugPipelineTotalIncludesAggregator ==
  ImplementationActions(PipelineTotalExcludesAggregator) =
    SpecActions(PipelineTotalExcludesAggregator)

BugPipelineTotalOverflows ==
  ImplementationActions(PipelineTotalSaturates) =
    SpecActions(PipelineTotalSaturates)

BugPipelineMaxOmitsCommit ==
  ImplementationActions(PipelineMaxCoreSum) =
    SpecActions(PipelineMaxCoreSum)

BugPipelineMaxIncludesAggregator ==
  ImplementationActions(PipelineMaxExcludesAggregator) =
    SpecActions(PipelineMaxExcludesAggregator)

BugPipelineMaxOverflows ==
  ImplementationActions(PipelineMaxSaturates) =
    SpecActions(PipelineMaxSaturates)

BugSnapshotPipelineEmaMismatch ==
  ImplementationActions(SnapshotProjectsPipelineEma) =
    SpecActions(SnapshotProjectsPipelineEma)

BugSnapshotGossipMismatch ==
  ImplementationActions(SnapshotProjectsGossipFallback) =
    SpecActions(SnapshotProjectsGossipFallback)

BugSnapshotBlockCreatedMismatch ==
  ImplementationActions(SnapshotProjectsBlockCreatedCounters) =
    SpecActions(SnapshotProjectsBlockCreatedCounters)

BugResetKeepsBlockCreated ==
  ImplementationActions(ResetClearsBlockCreatedCounters) =
    SpecActions(ResetClearsBlockCreatedCounters)

====
