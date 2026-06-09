---- MODULE SumeragiWorkerLoopStageGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi worker-loop stage helpers.

This slice pins `WorkerLoopStage::as_id(...)`,
`WorkerLoopStage::from_id(...)`, and `WorkerLoopStage::as_str(...)` from
`status.rs`. The worker-queue status gate proves that the current stage is
stored and reset correctly; this companion gate fixes the stable helper
surface used by status snapshots and diagnostics.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Idle == 0
DrainVotes == 1
DrainRbcChunks == 2
DrainBlockPayloads == 3
DrainBlocks == 4
Tick == 5
DrainConsensus == 6
DrainLaneRelay == 7
DrainBackground == 8

StageCases == 0..8
IdCases == 0..10
KnownIds == 0..8
UnknownIds == {9, 10}

SpecStageId(stage) ==
  CASE stage = Idle -> 0
    [] stage = DrainVotes -> 1
    [] stage = DrainRbcChunks -> 2
    [] stage = DrainBlockPayloads -> 3
    [] stage = DrainBlocks -> 4
    [] stage = Tick -> 5
    [] stage = DrainConsensus -> 6
    [] stage = DrainLaneRelay -> 7
    [] stage = DrainBackground -> 8

ActualStageId(stage) ==
  CASE Bug = "stage_idle_id_nonzero"
       /\ stage = Idle -> 1
    [] Bug = "stage_votes_id_wrong"
       /\ stage = DrainVotes -> 2
    [] Bug = "stage_rbc_id_wrong"
       /\ stage = DrainRbcChunks -> 1
    [] Bug = "stage_payload_id_wrong"
       /\ stage = DrainBlockPayloads -> 4
    [] Bug = "stage_blocks_id_wrong"
       /\ stage = DrainBlocks -> 3
    [] Bug = "stage_tick_id_wrong"
       /\ stage = Tick -> 6
    [] Bug = "stage_consensus_id_wrong"
       /\ stage = DrainConsensus -> 5
    [] Bug = "stage_lane_id_wrong"
       /\ stage = DrainLaneRelay -> 8
    [] Bug = "stage_background_id_wrong"
       /\ stage = DrainBackground -> 7
    [] OTHER -> SpecStageId(stage)

SpecStageFromId(id) ==
  CASE id = 1 -> DrainVotes
    [] id = 2 -> DrainRbcChunks
    [] id = 3 -> DrainBlockPayloads
    [] id = 4 -> DrainBlocks
    [] id = 5 -> Tick
    [] id = 6 -> DrainConsensus
    [] id = 7 -> DrainLaneRelay
    [] id = 8 -> DrainBackground
    [] OTHER -> Idle

ActualStageFromId(id) ==
  CASE Bug = "from_id_zero_wrong"
       /\ id = 0 -> DrainVotes
    [] Bug = "from_id_unknown_not_idle"
       /\ id \in UnknownIds -> DrainBackground
    [] Bug = "from_id_votes_wrong"
       /\ id = 1 -> DrainRbcChunks
    [] Bug = "from_id_rbc_wrong"
       /\ id = 2 -> DrainVotes
    [] Bug = "from_id_payload_wrong"
       /\ id = 3 -> DrainBlocks
    [] Bug = "from_id_lane_wrong"
       /\ id = 7 -> DrainConsensus
    [] OTHER -> SpecStageFromId(id)

SpecStageLabel(stage) ==
  CASE stage = Idle -> "idle"
    [] stage = DrainVotes -> "drain_votes"
    [] stage = DrainRbcChunks -> "drain_rbc_chunks"
    [] stage = DrainBlockPayloads -> "drain_block_payloads"
    [] stage = DrainBlocks -> "drain_blocks"
    [] stage = Tick -> "tick"
    [] stage = DrainConsensus -> "drain_consensus"
    [] stage = DrainLaneRelay -> "drain_lane_relay"
    [] stage = DrainBackground -> "drain_background"

ActualStageLabel(stage) ==
  CASE Bug = "label_idle_wrong"
       /\ stage = Idle -> "drain_votes"
    [] Bug = "label_votes_wrong"
       /\ stage = DrainVotes -> "idle"
    [] Bug = "label_rbc_wrong"
       /\ stage = DrainRbcChunks -> "drain_blocks"
    [] Bug = "label_payload_wrong"
       /\ stage = DrainBlockPayloads -> "drain_rbc_chunks"
    [] Bug = "label_blocks_wrong"
       /\ stage = DrainBlocks -> "drain_block_payloads"
    [] Bug = "label_tick_wrong"
       /\ stage = Tick -> "drain_consensus"
    [] Bug = "label_consensus_wrong"
       /\ stage = DrainConsensus -> "tick"
    [] Bug = "label_lane_wrong"
       /\ stage = DrainLaneRelay -> "drain_background"
    [] Bug = "label_background_wrong"
       /\ stage = DrainBackground -> "drain_lane_relay"
    [] OTHER -> SpecStageLabel(stage)

StageLabels == {SpecStageLabel(stage): stage \in StageCases}

BugSet == {
  "none",
  "stage_idle_id_nonzero",
  "stage_votes_id_wrong",
  "stage_rbc_id_wrong",
  "stage_payload_id_wrong",
  "stage_blocks_id_wrong",
  "stage_tick_id_wrong",
  "stage_consensus_id_wrong",
  "stage_lane_id_wrong",
  "stage_background_id_wrong",
  "from_id_zero_wrong",
  "from_id_unknown_not_idle",
  "from_id_votes_wrong",
  "from_id_rbc_wrong",
  "from_id_payload_wrong",
  "from_id_lane_wrong",
  "label_idle_wrong",
  "label_votes_wrong",
  "label_rbc_wrong",
  "label_payload_wrong",
  "label_blocks_wrong",
  "label_tick_wrong",
  "label_consensus_wrong",
  "label_lane_wrong",
  "label_background_wrong"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A stage \in StageCases: ActualStageId(stage) \in KnownIds
  /\ \A id \in IdCases: ActualStageFromId(id) \in StageCases
  /\ \A stage \in StageCases: ActualStageLabel(stage) \in StageLabels

StageIdsExact ==
  \A stage \in StageCases:
    ActualStageId(stage) = SpecStageId(stage)

StageFromIdsExact ==
  \A id \in IdCases:
    ActualStageFromId(id) = SpecStageFromId(id)

StageLabelsExact ==
  \A stage \in StageCases:
    ActualStageLabel(stage) = SpecStageLabel(stage)

KnownIdsRoundTrip ==
  \A stage \in StageCases:
    ActualStageFromId(ActualStageId(stage)) = stage

UnknownIdsFallbackIdle ==
  \A id \in UnknownIds:
    ActualStageFromId(id) = Idle

StageLabelsDistinct ==
  \A a, b \in StageCases:
    a # b => ActualStageLabel(a) # ActualStageLabel(b)

RepresentativeStatusLabelsStable ==
  /\ ActualStageLabel(Idle) = "idle"
  /\ ActualStageLabel(DrainRbcChunks) = "drain_rbc_chunks"
  /\ ActualStageLabel(DrainBackground) = "drain_background"

StageIdImageExact ==
  {ActualStageId(stage): stage \in StageCases} = KnownIds

StageIdsDistinct ==
  \A a, b \in StageCases:
    a # b => ActualStageId(a) # ActualStageId(b)

KnownIdsReverseRoundTrip ==
  \A id \in KnownIds:
    ActualStageId(ActualStageFromId(id)) = id

StageLabelImageExact ==
  {ActualStageLabel(stage): stage \in StageCases} = StageLabels

IdleAndUnknownFallbackAnchors ==
  /\ ActualStageFromId(0) = Idle
  /\ \A id \in UnknownIds: ActualStageFromId(id) = Idle

WorkerLoopStageCoreSafety ==
  /\ StageIdsExact
  /\ StageFromIdsExact
  /\ StageLabelsExact
  /\ KnownIdsRoundTrip
  /\ UnknownIdsFallbackIdle
  /\ StageLabelsDistinct
  /\ RepresentativeStatusLabelsStable
  /\ StageIdImageExact
  /\ StageIdsDistinct
  /\ KnownIdsReverseRoundTrip
  /\ StageLabelImageExact
  /\ IdleAndUnknownFallbackAnchors

SafetyFast ==
  WorkerLoopStageCoreSafety

====
