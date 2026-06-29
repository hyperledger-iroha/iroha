---- MODULE SumeragiCommitmentSnapshotBuilderGate ----

(***************************************************************************
A bounded abstract model for `build_commitment_snapshots_from_totals(...)`.

The helper converts sorted lane and dataspace aggregate maps into status
snapshot records. This scalar model pins field preservation, block context
copying, and BTreeMap iteration order for two lanes and two lane/dataspace
pairs without introducing nested snapshot tuples for the checker.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BlockHeight == 21
BlockHash == 99
LaneA == 3
LaneB == 5
DataspaceA == 11
DataspaceB == 9

SpecLaneHeight == BlockHeight
ActualLaneHeight == IF Bug = 1 THEN 0 ELSE SpecLaneHeight

SpecLaneId == LaneA
ActualLaneId == IF Bug = 2 THEN 0 ELSE SpecLaneId

SpecLaneTxCount == 7
ActualLaneTxCount == IF Bug = 3 THEN 0 ELSE SpecLaneTxCount

SpecLaneChunkCount == 9
ActualLaneChunkCount == IF Bug = 4 THEN 0 ELSE SpecLaneChunkCount

SpecLaneRbcBytes == 512
SpecLaneTeuTotal == 400
ActualLaneRbcBytes == IF Bug = 5 THEN SpecLaneTeuTotal ELSE SpecLaneRbcBytes
ActualLaneTeuTotal == IF Bug = 5 THEN SpecLaneRbcBytes ELSE SpecLaneTeuTotal

SpecLaneHash == BlockHash
ActualLaneHash == IF Bug = 6 THEN 0 ELSE SpecLaneHash

SpecLaneFirst == LaneA
SpecLaneSecond == LaneB
ActualLaneFirst == IF Bug = 7 THEN LaneB ELSE LaneA
ActualLaneSecond == IF Bug = 7 THEN LaneA ELSE LaneB

SpecDataspaceLaneId == LaneA
ActualDataspaceLaneId == IF Bug = 8 THEN 0 ELSE SpecDataspaceLaneId

SpecDataspaceId == DataspaceA
ActualDataspaceId == IF Bug = 9 THEN 0 ELSE SpecDataspaceId

SpecDataspaceTxCount == 4
SpecDataspaceChunkCount == 6
ActualDataspaceTxCount ==
  IF Bug = 10 THEN SpecDataspaceChunkCount ELSE SpecDataspaceTxCount
ActualDataspaceChunkCount ==
  IF Bug = 10 THEN SpecDataspaceTxCount ELSE SpecDataspaceChunkCount

SpecDataspaceRbcBytes == 256
SpecDataspaceTeuTotal == 128
ActualDataspaceRbcBytes ==
  IF Bug = 11 THEN SpecDataspaceTeuTotal ELSE SpecDataspaceRbcBytes
ActualDataspaceTeuTotal ==
  IF Bug = 11 THEN SpecDataspaceRbcBytes ELSE SpecDataspaceTeuTotal

SpecDataspaceHash == BlockHash
ActualDataspaceHash == IF Bug = 12 THEN 0 ELSE SpecDataspaceHash

SpecDataspaceFirstLane == LaneA
SpecDataspaceFirstId == DataspaceA
SpecDataspaceSecondLane == LaneB
SpecDataspaceSecondId == DataspaceB
ActualDataspaceFirstLane == IF Bug = 13 THEN LaneB ELSE LaneA
ActualDataspaceFirstId == IF Bug = 13 THEN DataspaceB ELSE DataspaceA
ActualDataspaceSecondLane == IF Bug = 13 THEN LaneA ELSE LaneB
ActualDataspaceSecondId == IF Bug = 13 THEN DataspaceA ELSE DataspaceB

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

BugLaneHeightPreserved ==
  ActualLaneHeight = SpecLaneHeight

BugLaneIdPreserved ==
  ActualLaneId = SpecLaneId

BugLaneTxCountPreserved ==
  ActualLaneTxCount = SpecLaneTxCount

BugLaneChunkCountPreserved ==
  ActualLaneChunkCount = SpecLaneChunkCount

BugLaneByteAndTeuFieldsIndependent ==
  /\ ActualLaneRbcBytes = SpecLaneRbcBytes
  /\ ActualLaneTeuTotal = SpecLaneTeuTotal

BugLaneHashPreserved ==
  ActualLaneHash = SpecLaneHash

BugLaneOrderPreserved ==
  /\ ActualLaneFirst = SpecLaneFirst
  /\ ActualLaneSecond = SpecLaneSecond

BugDataspaceLaneIdPreserved ==
  ActualDataspaceLaneId = SpecDataspaceLaneId

BugDataspaceIdPreserved ==
  ActualDataspaceId = SpecDataspaceId

BugDataspaceCountFieldsIndependent ==
  /\ ActualDataspaceTxCount = SpecDataspaceTxCount
  /\ ActualDataspaceChunkCount = SpecDataspaceChunkCount

BugDataspaceByteAndTeuFieldsIndependent ==
  /\ ActualDataspaceRbcBytes = SpecDataspaceRbcBytes
  /\ ActualDataspaceTeuTotal = SpecDataspaceTeuTotal

BugDataspaceHashPreserved ==
  ActualDataspaceHash = SpecDataspaceHash

BugDataspaceOrderPreserved ==
  /\ ActualDataspaceFirstLane = SpecDataspaceFirstLane
  /\ ActualDataspaceFirstId = SpecDataspaceFirstId
  /\ ActualDataspaceSecondLane = SpecDataspaceSecondLane
  /\ ActualDataspaceSecondId = SpecDataspaceSecondId

CommitmentSnapshotBuilderExactness ==
  /\ BugLaneHeightPreserved
  /\ BugLaneIdPreserved
  /\ BugLaneTxCountPreserved
  /\ BugLaneChunkCountPreserved
  /\ BugLaneByteAndTeuFieldsIndependent
  /\ BugLaneHashPreserved
  /\ BugLaneOrderPreserved
  /\ BugDataspaceLaneIdPreserved
  /\ BugDataspaceIdPreserved
  /\ BugDataspaceCountFieldsIndependent
  /\ BugDataspaceByteAndTeuFieldsIndependent
  /\ BugDataspaceHashPreserved
  /\ BugDataspaceOrderPreserved

CommitmentSnapshotBuilderCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CommitmentSnapshotBuilderExactness

SafetyFast == CommitmentSnapshotBuilderExactness

====
