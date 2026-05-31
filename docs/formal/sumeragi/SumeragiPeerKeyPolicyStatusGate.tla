---- MODULE SumeragiPeerKeyPolicyStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi peer-key policy status accounting.

This slice captures `record_peer_key_policy_reject(...)`, the internal
`peer_key_policy_snapshot()` projection used by `snapshot()`, and the
test-only `reset_peer_key_policy_counters_for_tests()` helper from
`status.rs`: total and per-reason counters, stable reason labels, last
reason/timestamp updates, top-level status projection, and reset semantics.
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
MissingHsmRecord == 2
AlgorithmRecord == 3
ProviderRecord == 4
LeadTimeRecord == 5
ActivationPastRecord == 6
ExpiryBeforeActivationRecord == 7
IdentifierCollisionRecord == 8
RepeatedSameReasonAccumulates == 9
DifferentReasonsIndependent == 10
LastReasonUsesStableLabels == 11
LastTimestampSet == 12
SnapshotProjectsTotals == 13
SnapshotProjectsBuckets == 14
SnapshotProjectsLastReason == 15
SnapshotProjectsTimestamp == 16
TopLevelSnapshotIncludesPeerKeyPolicy == 17
ResetAfterRecordsClears == 18

Candidates == 1..18

ResetTotal == 1
ResetBuckets == 2
ResetLastReason == 3
ResetTimestamp == 4
IncrementTotal == 5
AccumulateTotal == 6
IncrementMissingHsm == 7
IncrementAlgorithm == 8
IncrementProvider == 9
IncrementLeadTime == 10
IncrementActivationPast == 11
IncrementExpiry == 12
IncrementIdentifierCollision == 13
SameReasonAccumulates == 14
BucketsIndependent == 15
SetLastReason == 16
StableMissingHsmLabel == 17
StableAlgorithmLabel == 18
StableProviderLabel == 19
StableLeadTimeLabel == 20
StableActivationPastLabel == 21
StableExpiryLabel == 22
StableIdentifierCollisionLabel == 23
SetTimestamp == 24
TimestampPositive == 25
SnapshotTotalMatches == 26
SnapshotBucketsMatch == 27
SnapshotLastReasonMatches == 28
SnapshotTimestampMatches == 29
TopLevelPeerKeyPolicyMatches == 30
SnapshotPreservesCounts == 31

Actions == 1..31

AllResetActions ==
  {ResetTotal, ResetBuckets, ResetLastReason, ResetTimestamp}

AllStableLabels ==
  {StableMissingHsmLabel, StableAlgorithmLabel, StableProviderLabel,
   StableLeadTimeLabel, StableActivationPastLabel, StableExpiryLabel,
   StableIdentifierCollisionLabel}

CommonRecordActions ==
  {IncrementTotal, SetLastReason, SetTimestamp, TimestampPositive}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = MissingHsmRecord ->
      CommonRecordActions \cup {IncrementMissingHsm, StableMissingHsmLabel}
    [] candidate = AlgorithmRecord ->
      CommonRecordActions \cup {IncrementAlgorithm, StableAlgorithmLabel}
    [] candidate = ProviderRecord ->
      CommonRecordActions \cup {IncrementProvider, StableProviderLabel}
    [] candidate = LeadTimeRecord ->
      CommonRecordActions \cup {IncrementLeadTime, StableLeadTimeLabel}
    [] candidate = ActivationPastRecord ->
      CommonRecordActions \cup
        {IncrementActivationPast, StableActivationPastLabel}
    [] candidate = ExpiryBeforeActivationRecord ->
      CommonRecordActions \cup {IncrementExpiry, StableExpiryLabel}
    [] candidate = IdentifierCollisionRecord ->
      CommonRecordActions \cup
        {IncrementIdentifierCollision, StableIdentifierCollisionLabel}
    [] candidate = RepeatedSameReasonAccumulates ->
      {AccumulateTotal, SameReasonAccumulates, SnapshotPreservesCounts,
       SetLastReason, SetTimestamp, TimestampPositive}
    [] candidate = DifferentReasonsIndependent ->
      {AccumulateTotal, BucketsIndependent, SnapshotPreservesCounts,
       SetLastReason}
    [] candidate = LastReasonUsesStableLabels ->
      {SetLastReason} \cup AllStableLabels
    [] candidate = LastTimestampSet ->
      {SetTimestamp, TimestampPositive}
    [] candidate = SnapshotProjectsTotals ->
      {SnapshotTotalMatches, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsBuckets ->
      {SnapshotBucketsMatch, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsLastReason ->
      {SnapshotLastReasonMatches}
    [] candidate = SnapshotProjectsTimestamp ->
      {SnapshotTimestampMatches}
    [] candidate = TopLevelSnapshotIncludesPeerKeyPolicy ->
      {TopLevelPeerKeyPolicyMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_total" ->
      spec \ {ResetTotal}
    [] candidate = MissingHsmRecord /\ Bug = "total_not_incremented" ->
      spec \ {IncrementTotal}
    [] candidate = MissingHsmRecord /\ Bug = "total_double_counted" ->
      spec \cup {AccumulateTotal}
    [] candidate = MissingHsmRecord /\ Bug = "missing_hsm_not_counted" ->
      spec \ {IncrementMissingHsm}
    [] candidate = AlgorithmRecord /\ Bug = "algorithm_not_counted" ->
      spec \ {IncrementAlgorithm}
    [] candidate = ProviderRecord /\ Bug = "provider_not_counted" ->
      spec \ {IncrementProvider}
    [] candidate = LeadTimeRecord /\ Bug = "lead_time_not_counted" ->
      spec \ {IncrementLeadTime}
    [] candidate = ActivationPastRecord /\
          Bug = "activation_past_not_counted" ->
      spec \ {IncrementActivationPast}
    [] candidate = ExpiryBeforeActivationRecord /\
          Bug = "expiry_not_counted" ->
      spec \ {IncrementExpiry}
    [] candidate = IdentifierCollisionRecord /\
          Bug = "identifier_collision_not_counted" ->
      spec \ {IncrementIdentifierCollision}
    [] candidate = MissingHsmRecord /\
          Bug = "missing_hsm_counts_algorithm" ->
      (spec \ {IncrementMissingHsm}) \cup {IncrementAlgorithm}
    [] candidate = RepeatedSameReasonAccumulates /\
          Bug = "same_reason_overwrites_count" ->
      (spec \ {SameReasonAccumulates, SnapshotPreservesCounts}) \cup
        {IncrementTotal}
    [] candidate = DifferentReasonsIndependent /\
          Bug = "different_reasons_collide" ->
      (spec \ {BucketsIndependent, SnapshotPreservesCounts}) \cup
        {SameReasonAccumulates}
    [] candidate = LastReasonUsesStableLabels /\
          Bug = "last_reason_not_updated" ->
      spec \ {SetLastReason}
    [] candidate = LastReasonUsesStableLabels /\
          Bug = "wrong_stable_label" ->
      spec \ {StableLeadTimeLabel}
    [] candidate = LastTimestampSet /\ Bug = "timestamp_zero" ->
      spec \ {SetTimestamp, TimestampPositive}
    [] candidate = SnapshotProjectsTotals /\
          Bug = "snapshot_total_mismatch" ->
      spec \ {SnapshotTotalMatches}
    [] candidate = SnapshotProjectsBuckets /\
          Bug = "snapshot_bucket_mismatch" ->
      spec \ {SnapshotBucketsMatch}
    [] candidate = SnapshotProjectsLastReason /\
          Bug = "snapshot_last_reason_mismatch" ->
      spec \ {SnapshotLastReasonMatches}
    [] candidate = SnapshotProjectsTimestamp /\
          Bug = "snapshot_timestamp_mismatch" ->
      spec \ {SnapshotTimestampMatches}
    [] candidate = TopLevelSnapshotIncludesPeerKeyPolicy /\
          Bug = "top_level_snapshot_drops_peer_key_policy" ->
      spec \ {TopLevelPeerKeyPolicyMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_buckets" ->
      spec \ {ResetBuckets}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_last" ->
      spec \ {ResetLastReason, ResetTimestamp}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 18
     /\ checked' = checked + 1
  \/ /\ checked = 18
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..18

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsTotal ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugTotalNotIncremented ==
  ImplementationActions(MissingHsmRecord) = SpecActions(MissingHsmRecord)

BugTotalDoubleCounted ==
  ImplementationActions(MissingHsmRecord) = SpecActions(MissingHsmRecord)

BugMissingHsmNotCounted ==
  ImplementationActions(MissingHsmRecord) = SpecActions(MissingHsmRecord)

BugAlgorithmNotCounted ==
  ImplementationActions(AlgorithmRecord) = SpecActions(AlgorithmRecord)

BugProviderNotCounted ==
  ImplementationActions(ProviderRecord) = SpecActions(ProviderRecord)

BugLeadTimeNotCounted ==
  ImplementationActions(LeadTimeRecord) = SpecActions(LeadTimeRecord)

BugActivationPastNotCounted ==
  ImplementationActions(ActivationPastRecord) =
    SpecActions(ActivationPastRecord)

BugExpiryNotCounted ==
  ImplementationActions(ExpiryBeforeActivationRecord) =
    SpecActions(ExpiryBeforeActivationRecord)

BugIdentifierCollisionNotCounted ==
  ImplementationActions(IdentifierCollisionRecord) =
    SpecActions(IdentifierCollisionRecord)

BugMissingHsmCountsAlgorithm ==
  ImplementationActions(MissingHsmRecord) = SpecActions(MissingHsmRecord)

BugSameReasonOverwritesCount ==
  ImplementationActions(RepeatedSameReasonAccumulates) =
    SpecActions(RepeatedSameReasonAccumulates)

BugDifferentReasonsCollide ==
  ImplementationActions(DifferentReasonsIndependent) =
    SpecActions(DifferentReasonsIndependent)

BugLastReasonNotUpdated ==
  ImplementationActions(LastReasonUsesStableLabels) =
    SpecActions(LastReasonUsesStableLabels)

BugWrongStableLabel ==
  ImplementationActions(LastReasonUsesStableLabels) =
    SpecActions(LastReasonUsesStableLabels)

BugTimestampZero ==
  ImplementationActions(LastTimestampSet) = SpecActions(LastTimestampSet)

BugSnapshotTotalMismatch ==
  ImplementationActions(SnapshotProjectsTotals) =
    SpecActions(SnapshotProjectsTotals)

BugSnapshotBucketMismatch ==
  ImplementationActions(SnapshotProjectsBuckets) =
    SpecActions(SnapshotProjectsBuckets)

BugSnapshotLastReasonMismatch ==
  ImplementationActions(SnapshotProjectsLastReason) =
    SpecActions(SnapshotProjectsLastReason)

BugSnapshotTimestampMismatch ==
  ImplementationActions(SnapshotProjectsTimestamp) =
    SpecActions(SnapshotProjectsTimestamp)

BugTopLevelSnapshotDropsPeerKeyPolicy ==
  ImplementationActions(TopLevelSnapshotIncludesPeerKeyPolicy) =
    SpecActions(TopLevelSnapshotIncludesPeerKeyPolicy)

BugResetAfterRecordsKeepsBuckets ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsLast ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

AllPeerKeyPolicyCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ResetEmptyClearsAllAnchors ==
  /\ ResetTotal \in ImplementationActions(ResetEmpty)
  /\ ResetBuckets \in ImplementationActions(ResetEmpty)
  /\ ResetLastReason \in ImplementationActions(ResetEmpty)
  /\ ResetTimestamp \in ImplementationActions(ResetEmpty)

TotalIncrementAnchors ==
  /\ IncrementTotal \in ImplementationActions(MissingHsmRecord)
  /\ ~(AccumulateTotal \in ImplementationActions(MissingHsmRecord))

ReasonBucketAnchors ==
  /\ IncrementMissingHsm \in ImplementationActions(MissingHsmRecord)
  /\ ~(IncrementAlgorithm \in ImplementationActions(MissingHsmRecord))
  /\ IncrementAlgorithm \in ImplementationActions(AlgorithmRecord)
  /\ IncrementProvider \in ImplementationActions(ProviderRecord)
  /\ IncrementLeadTime \in ImplementationActions(LeadTimeRecord)
  /\ IncrementActivationPast \in
       ImplementationActions(ActivationPastRecord)
  /\ IncrementExpiry \in
       ImplementationActions(ExpiryBeforeActivationRecord)
  /\ IncrementIdentifierCollision \in
       ImplementationActions(IdentifierCollisionRecord)

StableLabelAnchors ==
  /\ StableMissingHsmLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ StableAlgorithmLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ StableProviderLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ StableLeadTimeLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ StableActivationPastLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ StableExpiryLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ StableIdentifierCollisionLabel \in
       ImplementationActions(LastReasonUsesStableLabels)
  /\ SetLastReason \in ImplementationActions(LastReasonUsesStableLabels)

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

TimestampAnchors ==
  /\ SetTimestamp \in ImplementationActions(LastTimestampSet)
  /\ TimestampPositive \in ImplementationActions(LastTimestampSet)

SnapshotProjectionAnchors ==
  /\ SnapshotTotalMatches \in ImplementationActions(SnapshotProjectsTotals)
  /\ SnapshotPreservesCounts \in ImplementationActions(SnapshotProjectsTotals)
  /\ SnapshotBucketsMatch \in ImplementationActions(SnapshotProjectsBuckets)
  /\ SnapshotPreservesCounts \in ImplementationActions(SnapshotProjectsBuckets)
  /\ SnapshotLastReasonMatches \in
       ImplementationActions(SnapshotProjectsLastReason)
  /\ SnapshotTimestampMatches \in
       ImplementationActions(SnapshotProjectsTimestamp)
  /\ TopLevelPeerKeyPolicyMatches \in
       ImplementationActions(TopLevelSnapshotIncludesPeerKeyPolicy)

ResetAfterRecordsClearsAllAnchors ==
  /\ ResetTotal \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetBuckets \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastReason \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetTimestamp \in ImplementationActions(ResetAfterRecordsClears)

SafetyAnchors ==
  /\ AllPeerKeyPolicyCandidatesMatchSpec
  /\ ResetEmptyClearsAllAnchors
  /\ TotalIncrementAnchors
  /\ ReasonBucketAnchors
  /\ StableLabelAnchors
  /\ AccumulationAnchors
  /\ TimestampAnchors
  /\ SnapshotProjectionAnchors
  /\ ResetAfterRecordsClearsAllAnchors

=============================================================================
