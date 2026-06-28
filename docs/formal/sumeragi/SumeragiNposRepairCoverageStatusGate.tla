---- MODULE SumeragiNposRepairCoverageStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi NPoS repair-coverage status projection.

This slice captures `record_npos_repair_coverage(...)`,
`npos_repair_coverage_snapshot()`, the test-only
`reset_npos_repair_coverage_for_tests()` helper, and the
`snapshot().npos_repair_coverage` mode gate. Repair coverage is retained as
the latest local repair-fanout sample, hidden outside NPoS mode, and projected
unchanged when the current status mode is NPoS.
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
UnsetSnapshotNone == 2
RecordStoresHeightView == 3
RecordStoresReason == 4
RecordStoresPeerCount == 5
RecordStoresRequiredBps == 6
RecordStoresSelectedBps == 7
RecordStoresReachedFlag == 8
DirectSnapshotSomeAfterRecord == 9
OverwriteLatestRecord == 10
PermissionedHidesCoverage == 11
NposProjectsCoverage == 12
StatusSnapshotNoneWhenUnset == 13
ResetAfterRecordClears == 14
ModeSwitchPermissionedHidesRecorded == 15
ModeSwitchNposRestoresRecorded == 16
StatusSnapshotMatchesDirectFields == 17

Candidates == 1..17

ResetSetFlag == 1
ResetHeight == 2
ResetView == 3
ResetReason == 4
ResetPeers == 5
ResetRequiredBps == 6
ResetSelectedBps == 7
ResetReached == 8
SnapshotNoneWhenUnset == 9
RecordSetsFlag == 10
StoreHeight == 11
StoreView == 12
StoreReason == 13
StorePeerCount == 14
StoreRequiredBps == 15
StoreSelectedBps == 16
StoreReachedFlag == 17
DirectSnapshotSome == 18
LatestRecordOverwrite == 19
PermissionedStatusNone == 20
NposStatusSome == 21
StatusFieldsMatchDirect == 22
UnsetStatusNone == 23

ResetActions ==
  {ResetSetFlag, ResetHeight, ResetView, ResetReason, ResetPeers,
   ResetRequiredBps, ResetSelectedBps, ResetReached}

RecordFieldActions ==
  {RecordSetsFlag, StoreHeight, StoreView, StoreReason, StorePeerCount,
   StoreRequiredBps, StoreSelectedBps, StoreReachedFlag}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      ResetActions
    [] candidate = UnsetSnapshotNone ->
      {SnapshotNoneWhenUnset, UnsetStatusNone}
    [] candidate = RecordStoresHeightView ->
      {RecordSetsFlag, StoreHeight, StoreView, DirectSnapshotSome}
    [] candidate = RecordStoresReason ->
      {RecordSetsFlag, StoreReason, DirectSnapshotSome}
    [] candidate = RecordStoresPeerCount ->
      {RecordSetsFlag, StorePeerCount, DirectSnapshotSome}
    [] candidate = RecordStoresRequiredBps ->
      {RecordSetsFlag, StoreRequiredBps, DirectSnapshotSome}
    [] candidate = RecordStoresSelectedBps ->
      {RecordSetsFlag, StoreSelectedBps, DirectSnapshotSome}
    [] candidate = RecordStoresReachedFlag ->
      {RecordSetsFlag, StoreReachedFlag, DirectSnapshotSome}
    [] candidate = DirectSnapshotSomeAfterRecord ->
      {RecordSetsFlag, DirectSnapshotSome}
    [] candidate = OverwriteLatestRecord ->
      RecordFieldActions \cup {LatestRecordOverwrite, DirectSnapshotSome}
    [] candidate = PermissionedHidesCoverage ->
      {RecordSetsFlag, PermissionedStatusNone}
    [] candidate = NposProjectsCoverage ->
      {RecordSetsFlag, NposStatusSome, StatusFieldsMatchDirect}
    [] candidate = StatusSnapshotNoneWhenUnset ->
      {UnsetStatusNone}
    [] candidate = ResetAfterRecordClears ->
      ResetActions \cup {SnapshotNoneWhenUnset, UnsetStatusNone}
    [] candidate = ModeSwitchPermissionedHidesRecorded ->
      {RecordSetsFlag, PermissionedStatusNone}
    [] candidate = ModeSwitchNposRestoresRecorded ->
      {RecordSetsFlag, PermissionedStatusNone, NposStatusSome,
       StatusFieldsMatchDirect}
    [] candidate = StatusSnapshotMatchesDirectFields ->
      RecordFieldActions \cup {NposStatusSome, StatusFieldsMatchDirect}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_set" ->
      spec \ {ResetSetFlag}
    [] candidate = UnsetSnapshotNone /\ Bug = "unset_snapshot_nonempty" ->
      (spec \ {SnapshotNoneWhenUnset}) \cup {DirectSnapshotSome}
    [] candidate = RecordStoresHeightView /\ Bug = "height_view_not_stored" ->
      spec \ {StoreHeight, StoreView}
    [] candidate = RecordStoresReason /\ Bug = "reason_not_stored" ->
      spec \ {StoreReason}
    [] candidate = RecordStoresPeerCount /\ Bug = "peer_count_not_stored" ->
      spec \ {StorePeerCount}
    [] candidate = RecordStoresRequiredBps /\ Bug = "required_bps_not_stored" ->
      spec \ {StoreRequiredBps}
    [] candidate = RecordStoresSelectedBps /\ Bug = "selected_bps_not_stored" ->
      spec \ {StoreSelectedBps}
    [] candidate = RecordStoresReachedFlag /\ Bug = "reached_flag_not_stored" ->
      spec \ {StoreReachedFlag}
    [] candidate = DirectSnapshotSomeAfterRecord /\
          Bug = "direct_snapshot_missing_after_record" ->
      spec \ {DirectSnapshotSome}
    [] candidate = OverwriteLatestRecord /\ Bug = "overwrite_keeps_old_record" ->
      spec \ {LatestRecordOverwrite}
    [] candidate = PermissionedHidesCoverage /\
          Bug = "permissioned_leaks_coverage" ->
      (spec \ {PermissionedStatusNone}) \cup {NposStatusSome}
    [] candidate = NposProjectsCoverage /\ Bug = "npos_hides_coverage" ->
      spec \ {NposStatusSome, StatusFieldsMatchDirect}
    [] candidate = NposProjectsCoverage /\ Bug = "status_fields_mismatch" ->
      spec \ {StatusFieldsMatchDirect}
    [] candidate = StatusSnapshotNoneWhenUnset /\ Bug = "unset_status_nonempty" ->
      (spec \ {UnsetStatusNone}) \cup {NposStatusSome}
    [] candidate = ResetAfterRecordClears /\
          Bug = "reset_after_record_keeps_snapshot" ->
      spec \ {ResetSetFlag, SnapshotNoneWhenUnset, UnsetStatusNone}
    [] candidate = ModeSwitchPermissionedHidesRecorded /\
          Bug = "mode_switch_permissioned_leaks" ->
      (spec \ {PermissionedStatusNone}) \cup {NposStatusSome}
    [] candidate = ModeSwitchNposRestoresRecorded /\
          Bug = "mode_switch_npos_loses_record" ->
      spec \ {NposStatusSome, StatusFieldsMatchDirect}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 17
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..17

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

NposRepairCoverageStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

NposRepairCoverageStatusExactness ==
  /\ NposRepairCoverageStatusActionsMatchSpec

NposRepairCoverageStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NposRepairCoverageStatusExactness

BugResetEmptyKeepsSet ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugUnsetSnapshotNonempty ==
  ImplementationActions(UnsetSnapshotNone) = SpecActions(UnsetSnapshotNone)

BugHeightViewNotStored ==
  ImplementationActions(RecordStoresHeightView) =
    SpecActions(RecordStoresHeightView)

BugReasonNotStored ==
  ImplementationActions(RecordStoresReason) = SpecActions(RecordStoresReason)

BugPeerCountNotStored ==
  ImplementationActions(RecordStoresPeerCount) =
    SpecActions(RecordStoresPeerCount)

BugRequiredBpsNotStored ==
  ImplementationActions(RecordStoresRequiredBps) =
    SpecActions(RecordStoresRequiredBps)

BugSelectedBpsNotStored ==
  ImplementationActions(RecordStoresSelectedBps) =
    SpecActions(RecordStoresSelectedBps)

BugReachedFlagNotStored ==
  ImplementationActions(RecordStoresReachedFlag) =
    SpecActions(RecordStoresReachedFlag)

BugDirectSnapshotMissingAfterRecord ==
  ImplementationActions(DirectSnapshotSomeAfterRecord) =
    SpecActions(DirectSnapshotSomeAfterRecord)

BugOverwriteKeepsOldRecord ==
  ImplementationActions(OverwriteLatestRecord) =
    SpecActions(OverwriteLatestRecord)

BugPermissionedLeaksCoverage ==
  ImplementationActions(PermissionedHidesCoverage) =
    SpecActions(PermissionedHidesCoverage)

BugNposHidesCoverage ==
  ImplementationActions(NposProjectsCoverage) =
    SpecActions(NposProjectsCoverage)

BugStatusFieldsMismatch ==
  ImplementationActions(NposProjectsCoverage) =
    SpecActions(NposProjectsCoverage)

BugUnsetStatusNonempty ==
  ImplementationActions(StatusSnapshotNoneWhenUnset) =
    SpecActions(StatusSnapshotNoneWhenUnset)

BugResetAfterRecordKeepsSnapshot ==
  ImplementationActions(ResetAfterRecordClears) =
    SpecActions(ResetAfterRecordClears)

BugModeSwitchPermissionedLeaks ==
  ImplementationActions(ModeSwitchPermissionedHidesRecorded) =
    SpecActions(ModeSwitchPermissionedHidesRecorded)

BugModeSwitchNposLosesRecord ==
  ImplementationActions(ModeSwitchNposRestoresRecorded) =
    SpecActions(ModeSwitchNposRestoresRecorded)

=============================================================================
====
