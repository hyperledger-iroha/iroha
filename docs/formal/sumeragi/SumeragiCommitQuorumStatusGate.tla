---- MODULE SumeragiCommitQuorumStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-quorum status projection.

This slice captures `record_commit_quorum_snapshot(...)`,
`reset_commit_quorum_for_tests()`, `commit_quorum_snapshot()`, and the JSON
and typed Torii status projections for `commit_quorum`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetClearsRound == 1
ResetClearsHash == 2
ResetClearsCounts == 3
ResetClearsTimestamp == 4
RecordStoresRound == 5
RecordStoresHash == 6
RecordStoresPresent == 7
RecordStoresCounted == 8
RecordStoresSetB == 9
RecordStoresRequired == 10
RecordTimestampRefreshes == 11
RecordOverwritesPrevious == 12
SnapshotDefaultsEmpty == 13
SnapshotProjectsRoundHash == 14
SnapshotProjectsCounts == 15
JsonIncludesCommitQuorum == 16
JsonProjectsRoundHash == 17
JsonProjectsCounts == 18
JsonProjectsTimestamp == 19
TypedProjectsRoundHash == 20
TypedProjectsCounts == 21
TypedProjectsTimestamp == 22

Candidates == 1..22

ResetHeight == 1
ResetView == 2
ResetHash == 3
ResetPresent == 4
ResetCounted == 5
ResetSetB == 6
ResetRequired == 7
ResetTimestamp == 8
StoreHeight == 9
StoreView == 10
StoreHash == 11
StorePresent == 12
StoreCounted == 13
StoreSetB == 14
StoreRequired == 15
StoreTimestamp == 16
OverwritePrevious == 17
SnapshotDefault == 18
SnapshotHeight == 19
SnapshotView == 20
SnapshotHash == 21
SnapshotPresent == 22
SnapshotCounted == 23
SnapshotSetB == 24
SnapshotRequired == 25
SnapshotTimestamp == 26
JsonCommitQuorumObject == 27
JsonHeight == 28
JsonView == 29
JsonHash == 30
JsonPresent == 31
JsonCounted == 32
JsonSetB == 33
JsonRequired == 34
JsonTimestamp == 35
TypedHeight == 36
TypedView == 37
TypedHash == 38
TypedPresent == 39
TypedCounted == 40
TypedSetB == 41
TypedRequired == 42
TypedTimestamp == 43

Actions == 1..43

ResetRoundActions == {ResetHeight, ResetView}
ResetCountActions == {ResetPresent, ResetCounted, ResetSetB, ResetRequired}
StoreRoundActions == {StoreHeight, StoreView}
StoreCountActions == {StorePresent, StoreCounted, StoreSetB, StoreRequired}
SnapshotRoundHashActions == {SnapshotHeight, SnapshotView, SnapshotHash}
SnapshotCountActions ==
  {SnapshotPresent, SnapshotCounted, SnapshotSetB, SnapshotRequired}
JsonRoundHashActions ==
  {JsonCommitQuorumObject, JsonHeight, JsonView, JsonHash}
JsonCountActions ==
  {JsonCommitQuorumObject, JsonPresent, JsonCounted, JsonSetB, JsonRequired}
TypedRoundHashActions == {TypedHeight, TypedView, TypedHash}
TypedCountActions == {TypedPresent, TypedCounted, TypedSetB, TypedRequired}

SpecActions(candidate) ==
  CASE candidate = ResetClearsRound ->
      ResetRoundActions \cup {SnapshotDefault}
    [] candidate = ResetClearsHash ->
      {ResetHash, SnapshotDefault}
    [] candidate = ResetClearsCounts ->
      ResetCountActions \cup {SnapshotDefault}
    [] candidate = ResetClearsTimestamp ->
      {ResetTimestamp, SnapshotDefault}
    [] candidate = RecordStoresRound ->
      StoreRoundActions \cup {SnapshotHeight, SnapshotView}
    [] candidate = RecordStoresHash ->
      {StoreHash, SnapshotHash}
    [] candidate = RecordStoresPresent ->
      {StorePresent, SnapshotPresent}
    [] candidate = RecordStoresCounted ->
      {StoreCounted, SnapshotCounted}
    [] candidate = RecordStoresSetB ->
      {StoreSetB, SnapshotSetB}
    [] candidate = RecordStoresRequired ->
      {StoreRequired, SnapshotRequired}
    [] candidate = RecordTimestampRefreshes ->
      {StoreTimestamp, SnapshotTimestamp}
    [] candidate = RecordOverwritesPrevious ->
      StoreRoundActions \cup {StoreHash, OverwritePrevious} \cup StoreCountActions
    [] candidate = SnapshotDefaultsEmpty ->
      {SnapshotDefault}
    [] candidate = SnapshotProjectsRoundHash ->
      StoreRoundActions \cup {StoreHash} \cup SnapshotRoundHashActions
    [] candidate = SnapshotProjectsCounts ->
      StoreCountActions \cup SnapshotCountActions
    [] candidate = JsonIncludesCommitQuorum ->
      {JsonCommitQuorumObject}
    [] candidate = JsonProjectsRoundHash ->
      SnapshotRoundHashActions \cup JsonRoundHashActions
    [] candidate = JsonProjectsCounts ->
      SnapshotCountActions \cup JsonCountActions
    [] candidate = JsonProjectsTimestamp ->
      {SnapshotTimestamp, JsonCommitQuorumObject, JsonTimestamp}
    [] candidate = TypedProjectsRoundHash ->
      SnapshotRoundHashActions \cup TypedRoundHashActions
    [] candidate = TypedProjectsCounts ->
      SnapshotCountActions \cup TypedCountActions
    [] candidate = TypedProjectsTimestamp ->
      {SnapshotTimestamp, TypedTimestamp}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetClearsRound /\
          Bug = "reset_keeps_round" ->
      spec \ ResetRoundActions
    [] candidate = ResetClearsHash /\
          Bug = "reset_keeps_hash" ->
      spec \ {ResetHash}
    [] candidate = ResetClearsCounts /\
          Bug = "reset_keeps_counts" ->
      spec \ ResetCountActions
    [] candidate = ResetClearsTimestamp /\
          Bug = "reset_keeps_timestamp" ->
      spec \ {ResetTimestamp}
    [] candidate = RecordStoresRound /\
          Bug = "record_round_dropped" ->
      spec \ StoreRoundActions
    [] candidate = RecordStoresHash /\
          Bug = "record_hash_dropped" ->
      spec \ {StoreHash, SnapshotHash}
    [] candidate = RecordStoresPresent /\
          Bug = "present_not_stored" ->
      spec \ {StorePresent, SnapshotPresent}
    [] candidate = RecordStoresCounted /\
          Bug = "counted_not_stored" ->
      spec \ {StoreCounted, SnapshotCounted}
    [] candidate = RecordStoresSetB /\
          Bug = "set_b_not_stored" ->
      spec \ {StoreSetB, SnapshotSetB}
    [] candidate = RecordStoresRequired /\
          Bug = "required_not_stored" ->
      spec \ {StoreRequired, SnapshotRequired}
    [] candidate = RecordTimestampRefreshes /\
          Bug = "timestamp_zero" ->
      spec \ {StoreTimestamp, SnapshotTimestamp}
    [] candidate = RecordOverwritesPrevious /\
          Bug = "overwrite_ignored" ->
      spec \ {OverwritePrevious}
    [] candidate = SnapshotDefaultsEmpty /\
          Bug = "snapshot_empty_nondefault" ->
      spec \ {SnapshotDefault}
    [] candidate = SnapshotProjectsRoundHash /\
          Bug = "snapshot_drops_hash" ->
      spec \ {SnapshotHash}
    [] candidate = SnapshotProjectsCounts /\
          Bug = "snapshot_drops_counts" ->
      spec \ SnapshotCountActions
    [] candidate = JsonIncludesCommitQuorum /\
          Bug = "json_missing_commit_quorum" ->
      spec \ {JsonCommitQuorumObject}
    [] candidate = JsonProjectsRoundHash /\
          Bug = "json_round_hash_mismatch" ->
      spec \ {JsonHeight, JsonView, JsonHash}
    [] candidate = JsonProjectsCounts /\
          Bug = "json_counts_mismatch" ->
      spec \ {JsonPresent, JsonCounted, JsonSetB, JsonRequired}
    [] candidate = JsonProjectsTimestamp /\
          Bug = "json_timestamp_mismatch" ->
      spec \ {JsonTimestamp}
    [] candidate = TypedProjectsRoundHash /\
          Bug = "typed_round_hash_mismatch" ->
      spec \ {TypedHeight, TypedView, TypedHash}
    [] candidate = TypedProjectsCounts /\
          Bug = "typed_counts_mismatch" ->
      spec \ {TypedPresent, TypedCounted, TypedSetB, TypedRequired}
    [] candidate = TypedProjectsTimestamp /\
          Bug = "typed_timestamp_mismatch" ->
      spec \ {TypedTimestamp}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 22
     /\ checked' = checked + 1
  \/ /\ checked = 22
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..22

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetKeepsRound ==
  ImplementationActions(ResetClearsRound) =
    SpecActions(ResetClearsRound)

BugResetKeepsHash ==
  ImplementationActions(ResetClearsHash) =
    SpecActions(ResetClearsHash)

BugResetKeepsCounts ==
  ImplementationActions(ResetClearsCounts) =
    SpecActions(ResetClearsCounts)

BugResetKeepsTimestamp ==
  ImplementationActions(ResetClearsTimestamp) =
    SpecActions(ResetClearsTimestamp)

BugRecordRoundDropped ==
  ImplementationActions(RecordStoresRound) =
    SpecActions(RecordStoresRound)

BugRecordHashDropped ==
  ImplementationActions(RecordStoresHash) =
    SpecActions(RecordStoresHash)

BugPresentNotStored ==
  ImplementationActions(RecordStoresPresent) =
    SpecActions(RecordStoresPresent)

BugCountedNotStored ==
  ImplementationActions(RecordStoresCounted) =
    SpecActions(RecordStoresCounted)

BugSetBNotStored ==
  ImplementationActions(RecordStoresSetB) =
    SpecActions(RecordStoresSetB)

BugRequiredNotStored ==
  ImplementationActions(RecordStoresRequired) =
    SpecActions(RecordStoresRequired)

BugTimestampZero ==
  ImplementationActions(RecordTimestampRefreshes) =
    SpecActions(RecordTimestampRefreshes)

BugOverwriteIgnored ==
  ImplementationActions(RecordOverwritesPrevious) =
    SpecActions(RecordOverwritesPrevious)

BugSnapshotEmptyNondefault ==
  ImplementationActions(SnapshotDefaultsEmpty) =
    SpecActions(SnapshotDefaultsEmpty)

BugSnapshotDropsHash ==
  ImplementationActions(SnapshotProjectsRoundHash) =
    SpecActions(SnapshotProjectsRoundHash)

BugSnapshotDropsCounts ==
  ImplementationActions(SnapshotProjectsCounts) =
    SpecActions(SnapshotProjectsCounts)

BugJsonMissingCommitQuorum ==
  ImplementationActions(JsonIncludesCommitQuorum) =
    SpecActions(JsonIncludesCommitQuorum)

BugJsonRoundHashMismatch ==
  ImplementationActions(JsonProjectsRoundHash) =
    SpecActions(JsonProjectsRoundHash)

BugJsonCountsMismatch ==
  ImplementationActions(JsonProjectsCounts) =
    SpecActions(JsonProjectsCounts)

BugJsonTimestampMismatch ==
  ImplementationActions(JsonProjectsTimestamp) =
    SpecActions(JsonProjectsTimestamp)

BugTypedRoundHashMismatch ==
  ImplementationActions(TypedProjectsRoundHash) =
    SpecActions(TypedProjectsRoundHash)

BugTypedCountsMismatch ==
  ImplementationActions(TypedProjectsCounts) =
    SpecActions(TypedProjectsCounts)

BugTypedTimestampMismatch ==
  ImplementationActions(TypedProjectsTimestamp) =
    SpecActions(TypedProjectsTimestamp)

AllCommitQuorumStatusCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ResetAnchors ==
  /\ ResetRoundActions \subseteq ImplementationActions(ResetClearsRound)
  /\ ResetHash \in ImplementationActions(ResetClearsHash)
  /\ ResetCountActions \subseteq ImplementationActions(ResetClearsCounts)
  /\ ResetTimestamp \in ImplementationActions(ResetClearsTimestamp)

RecordRoundHashAnchors ==
  /\ StoreRoundActions \subseteq ImplementationActions(RecordStoresRound)
  /\ StoreHash \in ImplementationActions(RecordStoresHash)
  /\ SnapshotRoundHashActions \subseteq
       ImplementationActions(SnapshotProjectsRoundHash)

RecordCountAnchors ==
  /\ StorePresent \in ImplementationActions(RecordStoresPresent)
  /\ StoreCounted \in ImplementationActions(RecordStoresCounted)
  /\ StoreSetB \in ImplementationActions(RecordStoresSetB)
  /\ StoreRequired \in ImplementationActions(RecordStoresRequired)
  /\ StoreCountActions \subseteq ImplementationActions(SnapshotProjectsCounts)

TimestampAndOverwriteAnchors ==
  /\ StoreTimestamp \in ImplementationActions(RecordTimestampRefreshes)
  /\ SnapshotTimestamp \in ImplementationActions(RecordTimestampRefreshes)
  /\ OverwritePrevious \in ImplementationActions(RecordOverwritesPrevious)
  /\ StoreRoundActions \subseteq
       ImplementationActions(RecordOverwritesPrevious)
  /\ StoreCountActions \subseteq
       ImplementationActions(RecordOverwritesPrevious)

SnapshotAnchors ==
  /\ SnapshotDefault \in ImplementationActions(SnapshotDefaultsEmpty)
  /\ SnapshotRoundHashActions \subseteq
       ImplementationActions(SnapshotProjectsRoundHash)
  /\ SnapshotCountActions \subseteq
       ImplementationActions(SnapshotProjectsCounts)

JsonProjectionAnchors ==
  /\ JsonCommitQuorumObject \in
       ImplementationActions(JsonIncludesCommitQuorum)
  /\ JsonRoundHashActions \subseteq
       ImplementationActions(JsonProjectsRoundHash)
  /\ JsonCountActions \subseteq
       ImplementationActions(JsonProjectsCounts)
  /\ JsonTimestamp \in ImplementationActions(JsonProjectsTimestamp)

TypedProjectionAnchors ==
  /\ TypedRoundHashActions \subseteq
       ImplementationActions(TypedProjectsRoundHash)
  /\ TypedCountActions \subseteq ImplementationActions(TypedProjectsCounts)
  /\ TypedTimestamp \in ImplementationActions(TypedProjectsTimestamp)

SafetyAnchors ==
  /\ AllCommitQuorumStatusCandidatesMatchSpec
  /\ ResetAnchors
  /\ RecordRoundHashAnchors
  /\ RecordCountAnchors
  /\ TimestampAndOverwriteAnchors
  /\ SnapshotAnchors
  /\ JsonProjectionAnchors
  /\ TypedProjectionAnchors

=============================================================================
====
