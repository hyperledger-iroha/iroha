---- MODULE SumeragiRbcMismatchStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi RBC mismatch status accounting.

This slice captures `RbcMismatchKind::label(...)`,
`record_rbc_mismatch(...)`, `rbc_mismatch_snapshot()`, and the test-only
`reset_rbc_mismatch_for_tests()` helper from `status.rs`: per-peer counters,
per-kind counter separation, snapshot preservation, timestamp refresh, reset
semantics, and saturating counter behavior.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LabelChunkDigest == 1
LabelPayloadHash == 2
LabelChunkRoot == 3
ResetEmpty == 4
FirstChunkDigestRecord == 5
FirstPayloadHashRecord == 6
FirstChunkRootRecord == 7
SamePeerSameKindAccumulates == 8
SamePeerDifferentKindsIndependent == 9
DifferentPeersIndependent == 10
TimestampSetOnFirst == 11
TimestampUpdatedOnRepeat == 12
SnapshotPreservesEntryCounts == 13
SaturatingCounter == 14
ResetAfterRecordsClears == 15

Candidates == 1..15

ChunkDigestLabel == 1
PayloadHashLabel == 2
ChunkRootLabel == 3
ResetClears == 4
CreateEntry == 5
IncrementChunkDigest == 6
IncrementPayloadHash == 7
IncrementChunkRoot == 8
PreserveChunkDigest == 9
PreservePayloadHash == 10
PreserveChunkRoot == 11
SamePeerMerged == 12
KindCountersIndependent == 13
DifferentPeersSeparated == 14
TimestampSet == 15
TimestampUpdated == 16
SnapshotIncludesPeer == 17
SnapshotIncludesAllPeers == 18
SnapshotPreservesCounts == 19
CountSaturates == 20
NoOverflow == 21
NoEntryAfterReset == 22
EmptySnapshot == 23
LastTimestampPositive == 24

Actions == 1..24

SpecActions(candidate) ==
  CASE candidate = LabelChunkDigest ->
      {ChunkDigestLabel}
    [] candidate = LabelPayloadHash ->
      {PayloadHashLabel}
    [] candidate = LabelChunkRoot ->
      {ChunkRootLabel}
    [] candidate = ResetEmpty ->
      {ResetClears, EmptySnapshot, NoEntryAfterReset}
    [] candidate = FirstChunkDigestRecord ->
      {CreateEntry, IncrementChunkDigest, PreservePayloadHash,
       PreserveChunkRoot, TimestampSet, LastTimestampPositive,
       SnapshotIncludesPeer}
    [] candidate = FirstPayloadHashRecord ->
      {CreateEntry, PreserveChunkDigest, IncrementPayloadHash,
       PreserveChunkRoot, TimestampSet, LastTimestampPositive,
       SnapshotIncludesPeer}
    [] candidate = FirstChunkRootRecord ->
      {CreateEntry, PreserveChunkDigest, PreservePayloadHash,
       IncrementChunkRoot, TimestampSet, LastTimestampPositive,
       SnapshotIncludesPeer}
    [] candidate = SamePeerSameKindAccumulates ->
      {SamePeerMerged, IncrementChunkDigest, PreservePayloadHash,
       PreserveChunkRoot, TimestampUpdated, LastTimestampPositive,
       SnapshotPreservesCounts}
    [] candidate = SamePeerDifferentKindsIndependent ->
      {SamePeerMerged, KindCountersIndependent, IncrementChunkDigest,
       IncrementPayloadHash, PreserveChunkRoot, SnapshotPreservesCounts}
    [] candidate = DifferentPeersIndependent ->
      {DifferentPeersSeparated, SnapshotIncludesAllPeers,
       SnapshotPreservesCounts}
    [] candidate = TimestampSetOnFirst ->
      {TimestampSet, LastTimestampPositive}
    [] candidate = TimestampUpdatedOnRepeat ->
      {TimestampUpdated, LastTimestampPositive}
    [] candidate = SnapshotPreservesEntryCounts ->
      {SnapshotIncludesPeer, SnapshotPreservesCounts}
    [] candidate = SaturatingCounter ->
      {CountSaturates, NoOverflow, PreservePayloadHash, PreserveChunkRoot}
    [] candidate = ResetAfterRecordsClears ->
      {ResetClears, EmptySnapshot, NoEntryAfterReset}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = LabelChunkDigest /\
          Bug = "chunk_digest_label_payload" ->
      (spec \ {ChunkDigestLabel}) \cup {PayloadHashLabel}
    [] candidate = LabelPayloadHash /\
          Bug = "payload_hash_label_chunk_digest" ->
      (spec \ {PayloadHashLabel}) \cup {ChunkDigestLabel}
    [] candidate = LabelChunkRoot /\ Bug = "chunk_root_label_payload_hash" ->
      (spec \ {ChunkRootLabel}) \cup {PayloadHashLabel}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_entry" ->
      (spec \ {EmptySnapshot, NoEntryAfterReset}) \cup {SnapshotIncludesPeer}
    [] candidate = FirstChunkDigestRecord /\
          Bug = "chunk_digest_increments_payload" ->
      (spec \ {IncrementChunkDigest, PreservePayloadHash}) \cup
        {PreserveChunkDigest, IncrementPayloadHash}
    [] candidate = FirstPayloadHashRecord /\
          Bug = "payload_hash_increments_chunk_digest" ->
      (spec \ {PreserveChunkDigest, IncrementPayloadHash}) \cup
        {IncrementChunkDigest, PreservePayloadHash}
    [] candidate = FirstChunkRootRecord /\
          Bug = "chunk_root_increments_payload" ->
      (spec \ {PreservePayloadHash, IncrementChunkRoot}) \cup
        {IncrementPayloadHash, PreserveChunkRoot}
    [] candidate = SamePeerSameKindAccumulates /\
          Bug = "same_kind_overwrites_count" ->
      (spec \ {IncrementChunkDigest, SnapshotPreservesCounts}) \cup
        {PreserveChunkDigest}
    [] candidate = SamePeerDifferentKindsIndependent /\
          Bug = "different_kinds_collide" ->
      (spec \ {KindCountersIndependent, IncrementPayloadHash,
               SnapshotPreservesCounts}) \cup
        {PreservePayloadHash}
    [] candidate = DifferentPeersIndependent /\
          Bug = "different_peers_merge" ->
      (spec \ {DifferentPeersSeparated, SnapshotIncludesAllPeers}) \cup
        {SamePeerMerged, SnapshotIncludesPeer}
    [] candidate = TimestampSetOnFirst /\ Bug = "first_timestamp_zero" ->
      spec \ {TimestampSet, LastTimestampPositive}
    [] candidate = TimestampUpdatedOnRepeat /\
          Bug = "repeat_timestamp_not_updated" ->
      (spec \ {TimestampUpdated}) \cup {TimestampSet}
    [] candidate = SnapshotPreservesEntryCounts /\
          Bug = "snapshot_drops_peer" ->
      spec \ {SnapshotIncludesPeer}
    [] candidate = SnapshotPreservesEntryCounts /\
          Bug = "snapshot_wrong_counts" ->
      spec \ {SnapshotPreservesCounts}
    [] candidate = SaturatingCounter /\ Bug = "saturating_overflows" ->
      (spec \ {CountSaturates, NoOverflow}) \cup {IncrementChunkDigest}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counts" ->
      (spec \ {ResetClears, EmptySnapshot, NoEntryAfterReset}) \cup
        {SnapshotIncludesPeer, SnapshotPreservesCounts}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 15
     /\ checked' = checked + 1
  \/ /\ checked = 15
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..15

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugChunkDigestLabelPayload ==
  ImplementationActions(LabelChunkDigest) = SpecActions(LabelChunkDigest)

BugPayloadHashLabelChunkDigest ==
  ImplementationActions(LabelPayloadHash) = SpecActions(LabelPayloadHash)

BugChunkRootLabelPayloadHash ==
  ImplementationActions(LabelChunkRoot) = SpecActions(LabelChunkRoot)

BugResetEmptyKeepsEntry ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugChunkDigestIncrementsPayload ==
  ImplementationActions(FirstChunkDigestRecord) =
    SpecActions(FirstChunkDigestRecord)

BugPayloadHashIncrementsChunkDigest ==
  ImplementationActions(FirstPayloadHashRecord) =
    SpecActions(FirstPayloadHashRecord)

BugChunkRootIncrementsPayload ==
  ImplementationActions(FirstChunkRootRecord) =
    SpecActions(FirstChunkRootRecord)

BugSameKindOverwritesCount ==
  ImplementationActions(SamePeerSameKindAccumulates) =
    SpecActions(SamePeerSameKindAccumulates)

BugDifferentKindsCollide ==
  ImplementationActions(SamePeerDifferentKindsIndependent) =
    SpecActions(SamePeerDifferentKindsIndependent)

BugDifferentPeersMerge ==
  ImplementationActions(DifferentPeersIndependent) =
    SpecActions(DifferentPeersIndependent)

BugFirstTimestampZero ==
  ImplementationActions(TimestampSetOnFirst) =
    SpecActions(TimestampSetOnFirst)

BugRepeatTimestampNotUpdated ==
  ImplementationActions(TimestampUpdatedOnRepeat) =
    SpecActions(TimestampUpdatedOnRepeat)

BugSnapshotDropsPeer ==
  ImplementationActions(SnapshotPreservesEntryCounts) =
    SpecActions(SnapshotPreservesEntryCounts)

BugSnapshotWrongCounts ==
  ImplementationActions(SnapshotPreservesEntryCounts) =
    SpecActions(SnapshotPreservesEntryCounts)

BugSaturatingOverflows ==
  ImplementationActions(SaturatingCounter) = SpecActions(SaturatingCounter)

BugResetAfterRecordsKeepsCounts ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

AllRbcMismatchCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

LabelAnchors ==
  /\ ChunkDigestLabel \in ImplementationActions(LabelChunkDigest)
  /\ PayloadHashLabel \in ImplementationActions(LabelPayloadHash)
  /\ ChunkRootLabel \in ImplementationActions(LabelChunkRoot)

ResetAnchors ==
  /\ {ResetClears, EmptySnapshot, NoEntryAfterReset} \subseteq
       ImplementationActions(ResetEmpty)
  /\ {ResetClears, EmptySnapshot, NoEntryAfterReset} \subseteq
       ImplementationActions(ResetAfterRecordsClears)

FirstRecordAnchors ==
  /\ {CreateEntry, IncrementChunkDigest, PreservePayloadHash,
      PreserveChunkRoot, TimestampSet, LastTimestampPositive,
      SnapshotIncludesPeer} \subseteq
       ImplementationActions(FirstChunkDigestRecord)
  /\ {CreateEntry, PreserveChunkDigest, IncrementPayloadHash,
      PreserveChunkRoot, TimestampSet, LastTimestampPositive,
      SnapshotIncludesPeer} \subseteq
       ImplementationActions(FirstPayloadHashRecord)
  /\ {CreateEntry, PreserveChunkDigest, PreservePayloadHash,
      IncrementChunkRoot, TimestampSet, LastTimestampPositive,
      SnapshotIncludesPeer} \subseteq
       ImplementationActions(FirstChunkRootRecord)

AccumulationAnchors ==
  /\ {SamePeerMerged, IncrementChunkDigest, PreservePayloadHash,
      PreserveChunkRoot, TimestampUpdated, LastTimestampPositive,
      SnapshotPreservesCounts} \subseteq
       ImplementationActions(SamePeerSameKindAccumulates)
  /\ {SamePeerMerged, KindCountersIndependent, IncrementChunkDigest,
      IncrementPayloadHash, PreserveChunkRoot,
      SnapshotPreservesCounts} \subseteq
       ImplementationActions(SamePeerDifferentKindsIndependent)

PeerSeparationAnchors ==
  /\ {DifferentPeersSeparated, SnapshotIncludesAllPeers,
      SnapshotPreservesCounts} \subseteq
       ImplementationActions(DifferentPeersIndependent)
  /\ ~(SamePeerMerged \in ImplementationActions(DifferentPeersIndependent))

TimestampAnchors ==
  /\ {TimestampSet, LastTimestampPositive} \subseteq
       ImplementationActions(TimestampSetOnFirst)
  /\ {TimestampUpdated, LastTimestampPositive} \subseteq
       ImplementationActions(TimestampUpdatedOnRepeat)

SnapshotAnchors ==
  /\ {SnapshotIncludesPeer, SnapshotPreservesCounts} \subseteq
       ImplementationActions(SnapshotPreservesEntryCounts)

SaturationAnchors ==
  /\ {CountSaturates, NoOverflow, PreservePayloadHash,
      PreserveChunkRoot} \subseteq ImplementationActions(SaturatingCounter)
  /\ ~(IncrementChunkDigest \in ImplementationActions(SaturatingCounter))

SafetyAnchors ==
  /\ AllRbcMismatchCandidatesMatchSpec
  /\ LabelAnchors
  /\ ResetAnchors
  /\ FirstRecordAnchors
  /\ AccumulationAnchors
  /\ PeerSeparationAnchors
  /\ TimestampAnchors
  /\ SnapshotAnchors
  /\ SaturationAnchors

RbcMismatchStatusExactness ==
  /\ \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)
  /\ SafetyAnchors

RbcMismatchStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcMismatchStatusExactness

=============================================================================
====
