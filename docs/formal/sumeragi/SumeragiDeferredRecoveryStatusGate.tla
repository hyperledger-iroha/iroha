---- MODULE SumeragiDeferredRecoveryStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi deferred-recovery status accounting.

This slice captures `inc_qc_deferred_missing_payload()`,
`inc_qc_deferred_resolved()`, `inc_qc_deferred_expired()`,
`inc_consensus_empty_commit_topology_defer()`,
`inc_consensus_empty_commit_topology_escalation()`, their `snapshot()`
projection, and the relevant subset of the test-only
`reset_missing_block_fetch_counters_for_tests()` helper from `status.rs`.
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
QcMissingPayloadRecord == 2
QcResolvedRecord == 3
QcExpiredRecord == 4
EmptyTopologyDeferRecord == 5
EmptyTopologyEscalationRecord == 6
RepeatedQcMissingPayloadAccumulates == 7
RepeatedEmptyTopologyDeferAccumulates == 8
SnapshotProjectsDeferredQc == 9
SnapshotProjectsEmptyTopology == 10
ResetAfterRecordsClears == 11

Candidates == 1..11

ResetDeferredQc == 1
ResetEmptyTopology == 2
IncrementQcMissingPayload == 3
IncrementQcResolved == 4
IncrementQcExpired == 5
IncrementEmptyTopologyDefer == 6
IncrementEmptyTopologyEscalation == 7
SameCounterAccumulates == 8
SnapshotDeferredQcMatches == 9
SnapshotEmptyTopologyMatches == 10

Actions == 1..10

AllResetActions == {ResetDeferredQc, ResetEmptyTopology}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = QcMissingPayloadRecord ->
      {IncrementQcMissingPayload}
    [] candidate = QcResolvedRecord ->
      {IncrementQcResolved}
    [] candidate = QcExpiredRecord ->
      {IncrementQcExpired}
    [] candidate = EmptyTopologyDeferRecord ->
      {IncrementEmptyTopologyDefer}
    [] candidate = EmptyTopologyEscalationRecord ->
      {IncrementEmptyTopologyEscalation}
    [] candidate = RepeatedQcMissingPayloadAccumulates ->
      {IncrementQcMissingPayload, SameCounterAccumulates,
       SnapshotDeferredQcMatches}
    [] candidate = RepeatedEmptyTopologyDeferAccumulates ->
      {IncrementEmptyTopologyDefer, SameCounterAccumulates,
       SnapshotEmptyTopologyMatches}
    [] candidate = SnapshotProjectsDeferredQc ->
      {SnapshotDeferredQcMatches}
    [] candidate = SnapshotProjectsEmptyTopology ->
      {SnapshotEmptyTopologyMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\
          Bug = "reset_empty_keeps_deferred_qc" ->
      spec \ {ResetDeferredQc}
    [] candidate = ResetEmpty /\
          Bug = "reset_empty_keeps_empty_topology" ->
      spec \ {ResetEmptyTopology}
    [] candidate = QcMissingPayloadRecord /\
          Bug = "qc_missing_payload_not_counted" ->
      spec \ {IncrementQcMissingPayload}
    [] candidate = QcResolvedRecord /\
          Bug = "qc_resolved_not_counted" ->
      spec \ {IncrementQcResolved}
    [] candidate = QcExpiredRecord /\
          Bug = "qc_expired_not_counted" ->
      spec \ {IncrementQcExpired}
    [] candidate = EmptyTopologyDeferRecord /\
          Bug = "empty_topology_defer_not_counted" ->
      spec \ {IncrementEmptyTopologyDefer}
    [] candidate = EmptyTopologyEscalationRecord /\
          Bug = "empty_topology_escalation_not_counted" ->
      spec \ {IncrementEmptyTopologyEscalation}
    [] candidate = QcMissingPayloadRecord /\
          Bug = "qc_missing_payload_counts_resolved" ->
      (spec \ {IncrementQcMissingPayload}) \cup {IncrementQcResolved}
    [] candidate = EmptyTopologyDeferRecord /\
          Bug = "empty_topology_defer_counts_escalation" ->
      (spec \ {IncrementEmptyTopologyDefer}) \cup
        {IncrementEmptyTopologyEscalation}
    [] candidate = RepeatedQcMissingPayloadAccumulates /\
          Bug = "repeated_qc_missing_payload_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotDeferredQcMatches}
    [] candidate = RepeatedEmptyTopologyDeferAccumulates /\
          Bug = "repeated_empty_topology_defer_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotEmptyTopologyMatches}
    [] candidate = SnapshotProjectsDeferredQc /\
          Bug = "snapshot_deferred_qc_mismatch" ->
      spec \ {SnapshotDeferredQcMatches}
    [] candidate = SnapshotProjectsEmptyTopology /\
          Bug = "snapshot_empty_topology_mismatch" ->
      spec \ {SnapshotEmptyTopologyMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_deferred_qc" ->
      spec \ {ResetDeferredQc}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_empty_topology" ->
      spec \ {ResetEmptyTopology}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 11
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..11

Safety ==
  /\ ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)
  /\ ImplementationActions(QcMissingPayloadRecord) =
       SpecActions(QcMissingPayloadRecord)
  /\ ImplementationActions(QcResolvedRecord) = SpecActions(QcResolvedRecord)
  /\ ImplementationActions(QcExpiredRecord) = SpecActions(QcExpiredRecord)
  /\ ImplementationActions(EmptyTopologyDeferRecord) =
       SpecActions(EmptyTopologyDeferRecord)
  /\ ImplementationActions(EmptyTopologyEscalationRecord) =
       SpecActions(EmptyTopologyEscalationRecord)
  /\ ImplementationActions(RepeatedQcMissingPayloadAccumulates) =
       SpecActions(RepeatedQcMissingPayloadAccumulates)
  /\ ImplementationActions(RepeatedEmptyTopologyDeferAccumulates) =
       SpecActions(RepeatedEmptyTopologyDeferAccumulates)
  /\ ImplementationActions(SnapshotProjectsDeferredQc) =
       SpecActions(SnapshotProjectsDeferredQc)
  /\ ImplementationActions(SnapshotProjectsEmptyTopology) =
       SpecActions(SnapshotProjectsEmptyTopology)
  /\ ImplementationActions(ResetAfterRecordsClears) =
       SpecActions(ResetAfterRecordsClears)

BugResetEmptyKeepsDeferredQc ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsEmptyTopology ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugQcMissingPayloadNotCounted ==
  ImplementationActions(QcMissingPayloadRecord) =
    SpecActions(QcMissingPayloadRecord)

BugQcResolvedNotCounted ==
  ImplementationActions(QcResolvedRecord) = SpecActions(QcResolvedRecord)

BugQcExpiredNotCounted ==
  ImplementationActions(QcExpiredRecord) = SpecActions(QcExpiredRecord)

BugEmptyTopologyDeferNotCounted ==
  ImplementationActions(EmptyTopologyDeferRecord) =
    SpecActions(EmptyTopologyDeferRecord)

BugEmptyTopologyEscalationNotCounted ==
  ImplementationActions(EmptyTopologyEscalationRecord) =
    SpecActions(EmptyTopologyEscalationRecord)

BugQcMissingPayloadCountsResolved ==
  ImplementationActions(QcMissingPayloadRecord) =
    SpecActions(QcMissingPayloadRecord)

BugEmptyTopologyDeferCountsEscalation ==
  ImplementationActions(EmptyTopologyDeferRecord) =
    SpecActions(EmptyTopologyDeferRecord)

BugRepeatedQcMissingPayloadOverwritesCount ==
  ImplementationActions(RepeatedQcMissingPayloadAccumulates) =
    SpecActions(RepeatedQcMissingPayloadAccumulates)

BugRepeatedEmptyTopologyDeferOverwritesCount ==
  ImplementationActions(RepeatedEmptyTopologyDeferAccumulates) =
    SpecActions(RepeatedEmptyTopologyDeferAccumulates)

BugSnapshotDeferredQcMismatch ==
  ImplementationActions(SnapshotProjectsDeferredQc) =
    SpecActions(SnapshotProjectsDeferredQc)

BugSnapshotEmptyTopologyMismatch ==
  ImplementationActions(SnapshotProjectsEmptyTopology) =
    SpecActions(SnapshotProjectsEmptyTopology)

BugResetAfterRecordsKeepsDeferredQc ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsEmptyTopology ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
