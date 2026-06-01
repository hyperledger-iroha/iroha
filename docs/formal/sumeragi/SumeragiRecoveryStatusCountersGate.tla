---- MODULE SumeragiRecoveryStatusCountersGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi recovery status counters.

This slice captures the status accounting helpers in `status.rs` around
missing-block fetch telemetry and recovery suppression:
`record_missing_block_fetch(...)`,
`inc_missing_request_pruned_stale_height(...)`,
`inc_pending_queue_evictions_total(...)`,
`inc_missing_qc_trigger_suppressed_stale(...)`,
`inc_committed_edge_conflict_obsolete(...)`,
`inc_roster_sidecar_mismatch_obsolete(...)`,
`inc_qc_missing_payload_aggressive_fetch()`, their `snapshot()` projection,
and the test-only `reset_missing_block_fetch_counters_for_tests()` helper.
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
RecordFetchFirst == 2
RecordFetchSecondOverwrites == 3
RecordFetchZeroTargets == 4
PruneZeroNoop == 5
PrunePositiveAccumulates == 6
PendingEvictZeroNoop == 7
PendingEvictPositiveAccumulates == 8
MissingQcSuppressedIncrements == 9
CommittedConflictIncrements == 10
RosterSidecarIncrements == 11
AggressiveFetchIncrements == 12
SnapshotProjectsFetch == 13
SnapshotProjectsCounters == 14
ResetAfterRecordsClears == 15

Candidates == 1..15

ResetFetchTotal == 1
ResetFetchTargets == 2
ResetFetchDwell == 3
ResetPruned == 4
ResetPendingEvictions == 5
ResetSuppressed == 6
ResetCommittedConflict == 7
ResetRosterSidecar == 8
ResetAggressiveFetch == 9
FetchIncrementsTotal == 10
FetchAccumulatesTotal == 11
FetchStoresTargets == 12
FetchStoresDwell == 13
FetchAllowsZeroTargets == 14
PruneZeroNoopAction == 15
PrunePositiveAddsCount == 16
PendingZeroNoopAction == 17
PendingPositiveAddsCount == 18
SuppressedIncrementsAction == 19
CommittedConflictIncrementsAction == 20
RosterSidecarIncrementsAction == 21
AggressiveFetchIncrementsAction == 22
SnapshotFetchMatches == 23
SnapshotCountersMatch == 24

Actions == 1..24

AllResetActions ==
  {ResetFetchTotal, ResetFetchTargets, ResetFetchDwell, ResetPruned,
   ResetPendingEvictions, ResetSuppressed, ResetCommittedConflict,
   ResetRosterSidecar, ResetAggressiveFetch}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = RecordFetchFirst ->
      {FetchIncrementsTotal, FetchStoresTargets, FetchStoresDwell,
       SnapshotFetchMatches}
    [] candidate = RecordFetchSecondOverwrites ->
      {FetchAccumulatesTotal, FetchStoresTargets, FetchStoresDwell,
       SnapshotFetchMatches}
    [] candidate = RecordFetchZeroTargets ->
      {FetchIncrementsTotal, FetchStoresTargets, FetchStoresDwell,
       FetchAllowsZeroTargets, SnapshotFetchMatches}
    [] candidate = PruneZeroNoop ->
      {PruneZeroNoopAction, SnapshotCountersMatch}
    [] candidate = PrunePositiveAccumulates ->
      {PrunePositiveAddsCount, SnapshotCountersMatch}
    [] candidate = PendingEvictZeroNoop ->
      {PendingZeroNoopAction, SnapshotCountersMatch}
    [] candidate = PendingEvictPositiveAccumulates ->
      {PendingPositiveAddsCount, SnapshotCountersMatch}
    [] candidate = MissingQcSuppressedIncrements ->
      {SuppressedIncrementsAction, SnapshotCountersMatch}
    [] candidate = CommittedConflictIncrements ->
      {CommittedConflictIncrementsAction, SnapshotCountersMatch}
    [] candidate = RosterSidecarIncrements ->
      {RosterSidecarIncrementsAction, SnapshotCountersMatch}
    [] candidate = AggressiveFetchIncrements ->
      {AggressiveFetchIncrementsAction, SnapshotCountersMatch}
    [] candidate = SnapshotProjectsFetch ->
      {SnapshotFetchMatches}
    [] candidate = SnapshotProjectsCounters ->
      {SnapshotCountersMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_fetch_total" ->
      spec \ {ResetFetchTotal}
    [] candidate = RecordFetchFirst /\ Bug = "record_fetch_drops_total" ->
      spec \ {FetchIncrementsTotal}
    [] candidate = RecordFetchFirst /\ Bug = "record_fetch_drops_targets" ->
      spec \ {FetchStoresTargets}
    [] candidate = RecordFetchFirst /\ Bug = "record_fetch_drops_dwell" ->
      spec \ {FetchStoresDwell}
    [] candidate = RecordFetchSecondOverwrites /\
          Bug = "record_fetch_second_keeps_old_targets" ->
      spec \ {FetchStoresTargets}
    [] candidate = RecordFetchSecondOverwrites /\
          Bug = "record_fetch_second_keeps_old_dwell" ->
      spec \ {FetchStoresDwell}
    [] candidate = RecordFetchZeroTargets /\
          Bug = "record_fetch_zero_targets_rejected" ->
      spec \ {FetchAllowsZeroTargets}
    [] candidate = PruneZeroNoop /\ Bug = "prune_zero_increments" ->
      (spec \ {PruneZeroNoopAction}) \cup {PrunePositiveAddsCount}
    [] candidate = PrunePositiveAccumulates /\
          Bug = "prune_positive_ignored" ->
      spec \ {PrunePositiveAddsCount}
    [] candidate = PendingEvictZeroNoop /\
          Bug = "pending_eviction_zero_increments" ->
      (spec \ {PendingZeroNoopAction}) \cup {PendingPositiveAddsCount}
    [] candidate = PendingEvictPositiveAccumulates /\
          Bug = "pending_eviction_positive_ignored" ->
      spec \ {PendingPositiveAddsCount}
    [] candidate = MissingQcSuppressedIncrements /\
          Bug = "missing_qc_suppressed_ignored" ->
      spec \ {SuppressedIncrementsAction}
    [] candidate = CommittedConflictIncrements /\
          Bug = "committed_conflict_ignored" ->
      spec \ {CommittedConflictIncrementsAction}
    [] candidate = RosterSidecarIncrements /\
          Bug = "roster_sidecar_ignored" ->
      spec \ {RosterSidecarIncrementsAction}
    [] candidate = AggressiveFetchIncrements /\
          Bug = "aggressive_fetch_ignored" ->
      spec \ {AggressiveFetchIncrementsAction}
    [] candidate = SnapshotProjectsFetch /\
          Bug = "snapshot_fetch_mismatch" ->
      spec \ {SnapshotFetchMatches}
    [] candidate = SnapshotProjectsCounters /\
          Bug = "snapshot_counter_mismatch" ->
      spec \ {SnapshotCountersMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetPruned, ResetPendingEvictions, ResetSuppressed,
              ResetCommittedConflict, ResetRosterSidecar, ResetAggressiveFetch}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 15
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..15

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsFetchTotal ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugRecordFetchDropsTotal ==
  ImplementationActions(RecordFetchFirst) = SpecActions(RecordFetchFirst)

BugRecordFetchDropsTargets ==
  ImplementationActions(RecordFetchFirst) = SpecActions(RecordFetchFirst)

BugRecordFetchDropsDwell ==
  ImplementationActions(RecordFetchFirst) = SpecActions(RecordFetchFirst)

BugRecordFetchSecondKeepsOldTargets ==
  ImplementationActions(RecordFetchSecondOverwrites) =
    SpecActions(RecordFetchSecondOverwrites)

BugRecordFetchSecondKeepsOldDwell ==
  ImplementationActions(RecordFetchSecondOverwrites) =
    SpecActions(RecordFetchSecondOverwrites)

BugRecordFetchZeroTargetsRejected ==
  ImplementationActions(RecordFetchZeroTargets) =
    SpecActions(RecordFetchZeroTargets)

BugPruneZeroIncrements ==
  ImplementationActions(PruneZeroNoop) = SpecActions(PruneZeroNoop)

BugPrunePositiveIgnored ==
  ImplementationActions(PrunePositiveAccumulates) =
    SpecActions(PrunePositiveAccumulates)

BugPendingEvictionZeroIncrements ==
  ImplementationActions(PendingEvictZeroNoop) =
    SpecActions(PendingEvictZeroNoop)

BugPendingEvictionPositiveIgnored ==
  ImplementationActions(PendingEvictPositiveAccumulates) =
    SpecActions(PendingEvictPositiveAccumulates)

BugMissingQcSuppressedIgnored ==
  ImplementationActions(MissingQcSuppressedIncrements) =
    SpecActions(MissingQcSuppressedIncrements)

BugCommittedConflictIgnored ==
  ImplementationActions(CommittedConflictIncrements) =
    SpecActions(CommittedConflictIncrements)

BugRosterSidecarIgnored ==
  ImplementationActions(RosterSidecarIncrements) =
    SpecActions(RosterSidecarIncrements)

BugAggressiveFetchIgnored ==
  ImplementationActions(AggressiveFetchIncrements) =
    SpecActions(AggressiveFetchIncrements)

BugSnapshotFetchMismatch ==
  ImplementationActions(SnapshotProjectsFetch) =
    SpecActions(SnapshotProjectsFetch)

BugSnapshotCounterMismatch ==
  ImplementationActions(SnapshotProjectsCounters) =
    SpecActions(SnapshotProjectsCounters)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
