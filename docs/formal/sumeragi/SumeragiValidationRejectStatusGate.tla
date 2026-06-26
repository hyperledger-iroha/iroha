---- MODULE SumeragiValidationRejectStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi validation-reject status accounting.

This slice captures `record_validation_reject(...)`, the internal
`validation_reject_snapshot()` projection used by `snapshot()`, and the
test-only `reset_validation_reject_counters_for_tests()` helper from
`status.rs`: total and per-reason counters, unknown-label behavior, last
reason/height/view/block/timestamp updates, top-level status projection, and
reset semantics.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetEmpty == 1
StatelessRecord == 2
ExecutionRecord == 3
PrevHashRecord == 4
PrevHeightRecord == 5
TopologyRecord == 6
UnknownReasonRecord == 7
RepeatedSameReasonAccumulates == 8
DifferentReasonsIndependent == 9
LastReasonUpdates == 10
LastSlotUpdates == 11
LastBlockUpdates == 12
TimestampSetOnRecord == 13
StatusSnapshotProjectsTotals == 14
StatusSnapshotProjectsLastReason == 15
ResetAfterRecordsClears == 16

Candidates == 1..16

ResetTotal == 1
ResetBuckets == 2
ResetLastReason == 3
ResetLastHeight == 4
ResetLastView == 5
ResetLastBlock == 6
ResetTimestamp == 7
IncrementTotal == 8
AccumulateTotal == 9
IncrementStateless == 10
IncrementExecution == 11
IncrementPrevHash == 12
IncrementPrevHeight == 13
IncrementTopology == 14
NoKnownBucketIncrement == 15
BucketsIndependent == 16
SameReasonAccumulates == 17
SetLastReason == 18
SetLastHeight == 19
SetLastView == 20
SetLastBlock == 21
SetTimestamp == 22
TimestampPositive == 23
StatusTotalMatches == 24
StatusReasonMatches == 25
StatusDetailedSnapshotMatches == 26
SnapshotPreservesCounts == 27

Actions == 1..27

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      {ResetTotal, ResetBuckets, ResetLastReason, ResetLastHeight,
       ResetLastView, ResetLastBlock, ResetTimestamp}
    [] candidate = StatelessRecord ->
      {IncrementTotal, IncrementStateless, SetLastReason, SetLastHeight,
       SetLastView, SetLastBlock, SetTimestamp, TimestampPositive}
    [] candidate = ExecutionRecord ->
      {IncrementTotal, IncrementExecution, SetLastReason, SetLastHeight,
       SetLastView, SetLastBlock, SetTimestamp, TimestampPositive}
    [] candidate = PrevHashRecord ->
      {IncrementTotal, IncrementPrevHash, SetLastReason, SetLastHeight,
       SetLastView, SetLastBlock, SetTimestamp, TimestampPositive}
    [] candidate = PrevHeightRecord ->
      {IncrementTotal, IncrementPrevHeight, SetLastReason, SetLastHeight,
       SetLastView, SetLastBlock, SetTimestamp, TimestampPositive}
    [] candidate = TopologyRecord ->
      {IncrementTotal, IncrementTopology, SetLastReason, SetLastHeight,
       SetLastView, SetLastBlock, SetTimestamp, TimestampPositive}
    [] candidate = UnknownReasonRecord ->
      {IncrementTotal, NoKnownBucketIncrement, SetLastReason, SetLastHeight,
       SetLastView, SetLastBlock, SetTimestamp, TimestampPositive}
    [] candidate = RepeatedSameReasonAccumulates ->
      {AccumulateTotal, SameReasonAccumulates, SnapshotPreservesCounts,
       SetLastReason, SetTimestamp, TimestampPositive}
    [] candidate = DifferentReasonsIndependent ->
      {AccumulateTotal, BucketsIndependent, SnapshotPreservesCounts,
       SetLastReason}
    [] candidate = LastReasonUpdates ->
      {SetLastReason}
    [] candidate = LastSlotUpdates ->
      {SetLastHeight, SetLastView}
    [] candidate = LastBlockUpdates ->
      {SetLastBlock}
    [] candidate = TimestampSetOnRecord ->
      {SetTimestamp, TimestampPositive}
    [] candidate = StatusSnapshotProjectsTotals ->
      {StatusTotalMatches, StatusDetailedSnapshotMatches,
       SnapshotPreservesCounts}
    [] candidate = StatusSnapshotProjectsLastReason ->
      {StatusReasonMatches, StatusDetailedSnapshotMatches}
    [] candidate = ResetAfterRecordsClears ->
      {ResetTotal, ResetBuckets, ResetLastReason, ResetLastHeight,
       ResetLastView, ResetLastBlock, ResetTimestamp}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_total" ->
      spec \ {ResetTotal}
    [] candidate = StatelessRecord /\
          Bug = "stateless_increments_execution" ->
      (spec \ {IncrementStateless}) \cup {IncrementExecution}
    [] candidate = ExecutionRecord /\
          Bug = "execution_increments_stateless" ->
      (spec \ {IncrementExecution}) \cup {IncrementStateless}
    [] candidate = PrevHashRecord /\
          Bug = "prev_hash_increments_prev_height" ->
      (spec \ {IncrementPrevHash}) \cup {IncrementPrevHeight}
    [] candidate = PrevHeightRecord /\
          Bug = "prev_height_increments_prev_hash" ->
      (spec \ {IncrementPrevHeight}) \cup {IncrementPrevHash}
    [] candidate = TopologyRecord /\ Bug = "topology_not_counted" ->
      spec \ {IncrementTopology}
    [] candidate = UnknownReasonRecord /\
          Bug = "unknown_increments_known_bucket" ->
      (spec \ {NoKnownBucketIncrement}) \cup {IncrementStateless}
    [] candidate = RepeatedSameReasonAccumulates /\
          Bug = "same_reason_overwrites_count" ->
      (spec \ {SameReasonAccumulates, SnapshotPreservesCounts}) \cup
        {IncrementTotal}
    [] candidate = DifferentReasonsIndependent /\
          Bug = "different_reasons_collide" ->
      (spec \ {BucketsIndependent, SnapshotPreservesCounts}) \cup
        {SameReasonAccumulates}
    [] candidate = LastReasonUpdates /\ Bug = "last_reason_not_updated" ->
      spec \ {SetLastReason}
    [] candidate = LastSlotUpdates /\ Bug = "last_height_not_updated" ->
      spec \ {SetLastHeight}
    [] candidate = LastSlotUpdates /\ Bug = "last_view_not_updated" ->
      spec \ {SetLastView}
    [] candidate = LastBlockUpdates /\ Bug = "last_block_not_updated" ->
      spec \ {SetLastBlock}
    [] candidate = TimestampSetOnRecord /\ Bug = "timestamp_zero" ->
      spec \ {SetTimestamp, TimestampPositive}
    [] candidate = StatusSnapshotProjectsTotals /\
          Bug = "status_total_mismatch" ->
      spec \ {StatusTotalMatches}
    [] candidate = StatusSnapshotProjectsLastReason /\
          Bug = "status_reason_mismatch" ->
      spec \ {StatusReasonMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_last" ->
      spec \ {ResetLastReason, ResetLastHeight, ResetLastView, ResetLastBlock,
              ResetTimestamp}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 16
     /\ checked' = checked + 1
  \/ /\ checked = 16
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..16

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsTotal ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugStatelessIncrementsExecution ==
  ImplementationActions(StatelessRecord) = SpecActions(StatelessRecord)

BugExecutionIncrementsStateless ==
  ImplementationActions(ExecutionRecord) = SpecActions(ExecutionRecord)

BugPrevHashIncrementsPrevHeight ==
  ImplementationActions(PrevHashRecord) = SpecActions(PrevHashRecord)

BugPrevHeightIncrementsPrevHash ==
  ImplementationActions(PrevHeightRecord) = SpecActions(PrevHeightRecord)

BugTopologyNotCounted ==
  ImplementationActions(TopologyRecord) = SpecActions(TopologyRecord)

BugUnknownIncrementsKnownBucket ==
  ImplementationActions(UnknownReasonRecord) =
    SpecActions(UnknownReasonRecord)

BugSameReasonOverwritesCount ==
  ImplementationActions(RepeatedSameReasonAccumulates) =
    SpecActions(RepeatedSameReasonAccumulates)

BugDifferentReasonsCollide ==
  ImplementationActions(DifferentReasonsIndependent) =
    SpecActions(DifferentReasonsIndependent)

BugLastReasonNotUpdated ==
  ImplementationActions(LastReasonUpdates) = SpecActions(LastReasonUpdates)

BugLastHeightNotUpdated ==
  ImplementationActions(LastSlotUpdates) = SpecActions(LastSlotUpdates)

BugLastViewNotUpdated ==
  ImplementationActions(LastSlotUpdates) = SpecActions(LastSlotUpdates)

BugLastBlockNotUpdated ==
  ImplementationActions(LastBlockUpdates) = SpecActions(LastBlockUpdates)

BugTimestampZero ==
  ImplementationActions(TimestampSetOnRecord) =
    SpecActions(TimestampSetOnRecord)

BugStatusTotalMismatch ==
  ImplementationActions(StatusSnapshotProjectsTotals) =
    SpecActions(StatusSnapshotProjectsTotals)

BugStatusReasonMismatch ==
  ImplementationActions(StatusSnapshotProjectsLastReason) =
    SpecActions(StatusSnapshotProjectsLastReason)

BugResetAfterRecordsKeepsLast ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

AllStatusCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ResetEmptyClearsAllAnchors ==
  /\ ResetTotal \in ImplementationActions(ResetEmpty)
  /\ ResetBuckets \in ImplementationActions(ResetEmpty)
  /\ ResetLastReason \in ImplementationActions(ResetEmpty)
  /\ ResetLastHeight \in ImplementationActions(ResetEmpty)
  /\ ResetLastView \in ImplementationActions(ResetEmpty)
  /\ ResetLastBlock \in ImplementationActions(ResetEmpty)
  /\ ResetTimestamp \in ImplementationActions(ResetEmpty)

KnownBucketIncrementAnchors ==
  /\ IncrementStateless \in ImplementationActions(StatelessRecord)
  /\ ~(IncrementExecution \in ImplementationActions(StatelessRecord))
  /\ IncrementExecution \in ImplementationActions(ExecutionRecord)
  /\ ~(IncrementStateless \in ImplementationActions(ExecutionRecord))
  /\ IncrementPrevHash \in ImplementationActions(PrevHashRecord)
  /\ ~(IncrementPrevHeight \in ImplementationActions(PrevHashRecord))
  /\ IncrementPrevHeight \in ImplementationActions(PrevHeightRecord)
  /\ ~(IncrementPrevHash \in ImplementationActions(PrevHeightRecord))
  /\ IncrementTopology \in ImplementationActions(TopologyRecord)

UnknownReasonNoKnownBucketAnchors ==
  /\ IncrementTotal \in ImplementationActions(UnknownReasonRecord)
  /\ NoKnownBucketIncrement \in ImplementationActions(UnknownReasonRecord)
  /\ ~(IncrementStateless \in ImplementationActions(UnknownReasonRecord))
  /\ ~(IncrementExecution \in ImplementationActions(UnknownReasonRecord))
  /\ ~(IncrementPrevHash \in ImplementationActions(UnknownReasonRecord))
  /\ ~(IncrementPrevHeight \in ImplementationActions(UnknownReasonRecord))
  /\ ~(IncrementTopology \in ImplementationActions(UnknownReasonRecord))

AccumulationAnchors ==
  /\ AccumulateTotal \in
       ImplementationActions(RepeatedSameReasonAccumulates)
  /\ SameReasonAccumulates \in
       ImplementationActions(RepeatedSameReasonAccumulates)
  /\ SnapshotPreservesCounts \in
       ImplementationActions(RepeatedSameReasonAccumulates)
  /\ AccumulateTotal \in
       ImplementationActions(DifferentReasonsIndependent)
  /\ BucketsIndependent \in
       ImplementationActions(DifferentReasonsIndependent)
  /\ SnapshotPreservesCounts \in
       ImplementationActions(DifferentReasonsIndependent)
  /\ ~(SameReasonAccumulates \in
       ImplementationActions(DifferentReasonsIndependent))

LastFieldUpdateAnchors ==
  /\ SetLastReason \in ImplementationActions(LastReasonUpdates)
  /\ SetLastHeight \in ImplementationActions(LastSlotUpdates)
  /\ SetLastView \in ImplementationActions(LastSlotUpdates)
  /\ SetLastBlock \in ImplementationActions(LastBlockUpdates)

TimestampAnchors ==
  /\ SetTimestamp \in ImplementationActions(TimestampSetOnRecord)
  /\ TimestampPositive \in ImplementationActions(TimestampSetOnRecord)

StatusProjectionAnchors ==
  /\ StatusTotalMatches \in
       ImplementationActions(StatusSnapshotProjectsTotals)
  /\ StatusDetailedSnapshotMatches \in
       ImplementationActions(StatusSnapshotProjectsTotals)
  /\ SnapshotPreservesCounts \in
       ImplementationActions(StatusSnapshotProjectsTotals)
  /\ StatusReasonMatches \in
       ImplementationActions(StatusSnapshotProjectsLastReason)
  /\ StatusDetailedSnapshotMatches \in
       ImplementationActions(StatusSnapshotProjectsLastReason)

ResetAfterRecordsClearsAllAnchors ==
  /\ ResetTotal \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetBuckets \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastReason \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastHeight \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastView \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastBlock \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetTimestamp \in ImplementationActions(ResetAfterRecordsClears)

SafetyAnchors ==
  /\ AllStatusCandidatesMatchSpec
  /\ ResetEmptyClearsAllAnchors
  /\ KnownBucketIncrementAnchors
  /\ UnknownReasonNoKnownBucketAnchors
  /\ AccumulationAnchors
  /\ LastFieldUpdateAnchors
  /\ TimestampAnchors
  /\ StatusProjectionAnchors
  /\ ResetAfterRecordsClearsAllAnchors

ValidationRejectStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ Safety
  /\ SafetyAnchors

=============================================================================
====
