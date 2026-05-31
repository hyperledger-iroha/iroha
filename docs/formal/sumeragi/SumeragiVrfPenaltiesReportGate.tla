---- MODULE SumeragiVrfPenaltiesReportGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the in-memory VRF penalties report store.

This slice captures `sumeragi::epoch_report::{update, get,
last_epoch_index, clear}`. The helper is an operator-facing mirror of
committed VRF penalty snapshots: updates store the exact report under its
epoch, update the best-effort latest epoch to the most recent write, replace
same-epoch reports instead of merging stale fields, preserve other epochs,
return no report for missing epochs, clear all reports and reset the latest
epoch, and reads must not mutate the store.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialEmpty == 1
UpdateStoresReport == 2
UpdateStoresLatestEpoch == 3
UpdateStoresRosterLen == 4
UpdateStoresCommittedList == 5
UpdateStoresNoParticipationList == 6
ReplaceSameEpoch == 7
MultipleEpochsPreserved == 8
LastEpochFollowsLatestWrite == 9
GetMissingAbsent == 10
ClearRemovesReports == 11
ClearResetsLatestEpoch == 12
ClearThenUpdateWorks == 13
GetSideEffectFree == 14

Candidates == 1..14

InitialLastZero == 1
InitialNoReports == 2
ReportStoredAtEpoch == 3
LatestEpochStored == 4
RosterLenExact == 5
CommittedNoRevealExact == 6
NoParticipationExact == 7
ReplacementUsesNewReport == 8
ReplacementDropsOldReport == 9
EarlierEpochRetained == 10
LaterEpochRetained == 11
LatestWriteCanMoveBackward == 12
MissingGetReturnsNone == 13
ClearReportsEmpty == 14
ClearLatestZero == 15
PostClearUpdateStored == 16
PostClearLatestStored == 17
GetDoesNotRemoveReport == 18

Actions == 1..18

SpecActions(candidate) ==
  CASE candidate = InitialEmpty ->
      {InitialLastZero, InitialNoReports}
    [] candidate = UpdateStoresReport ->
      {ReportStoredAtEpoch}
    [] candidate = UpdateStoresLatestEpoch ->
      {LatestEpochStored}
    [] candidate = UpdateStoresRosterLen ->
      {RosterLenExact}
    [] candidate = UpdateStoresCommittedList ->
      {CommittedNoRevealExact}
    [] candidate = UpdateStoresNoParticipationList ->
      {NoParticipationExact}
    [] candidate = ReplaceSameEpoch ->
      {ReplacementUsesNewReport, ReplacementDropsOldReport}
    [] candidate = MultipleEpochsPreserved ->
      {EarlierEpochRetained, LaterEpochRetained}
    [] candidate = LastEpochFollowsLatestWrite ->
      {LatestWriteCanMoveBackward}
    [] candidate = GetMissingAbsent ->
      {MissingGetReturnsNone}
    [] candidate = ClearRemovesReports ->
      {ClearReportsEmpty}
    [] candidate = ClearResetsLatestEpoch ->
      {ClearLatestZero}
    [] candidate = ClearThenUpdateWorks ->
      {PostClearUpdateStored, PostClearLatestStored}
    [] candidate = GetSideEffectFree ->
      {GetDoesNotRemoveReport}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialEmpty /\ Bug = "initial_last_nonzero" ->
      spec \ {InitialLastZero}
    [] candidate = InitialEmpty /\ Bug = "initial_report_present" ->
      spec \ {InitialNoReports}
    [] candidate = UpdateStoresReport /\ Bug = "update_skips_report" ->
      spec \ {ReportStoredAtEpoch}
    [] candidate = UpdateStoresLatestEpoch /\ Bug = "update_skips_last" ->
      spec \ {LatestEpochStored}
    [] candidate = UpdateStoresReport /\ Bug = "update_stores_wrong_key" ->
      spec \ {ReportStoredAtEpoch}
    [] candidate = UpdateStoresRosterLen /\ Bug = "update_drops_roster_len" ->
      spec \ {RosterLenExact}
    [] candidate = UpdateStoresCommittedList /\ Bug = "update_drops_committed_list" ->
      spec \ {CommittedNoRevealExact}
    [] candidate = UpdateStoresNoParticipationList /\
          Bug = "update_drops_no_participation_list" ->
      spec \ {NoParticipationExact}
    [] candidate = ReplaceSameEpoch /\ Bug = "replace_ignored" ->
      spec \ {ReplacementUsesNewReport}
    [] candidate = ReplaceSameEpoch /\ Bug = "replace_merges_old" ->
      spec \ {ReplacementDropsOldReport}
    [] candidate = MultipleEpochsPreserved /\
          Bug = "multiple_update_drops_old_epoch" ->
      spec \ {EarlierEpochRetained}
    [] candidate = LastEpochFollowsLatestWrite /\ Bug = "last_uses_max_epoch" ->
      spec \ {LatestWriteCanMoveBackward}
    [] candidate = GetMissingAbsent /\ Bug = "missing_get_synthesizes" ->
      spec \ {MissingGetReturnsNone}
    [] candidate = ClearRemovesReports /\ Bug = "clear_keeps_reports" ->
      spec \ {ClearReportsEmpty}
    [] candidate = ClearResetsLatestEpoch /\ Bug = "clear_keeps_last" ->
      spec \ {ClearLatestZero}
    [] candidate = GetSideEffectFree /\ Bug = "get_mutates_store" ->
      spec \ {GetDoesNotRemoveReport}
    [] candidate = ClearThenUpdateWorks /\ Bug = "clear_blocks_future_update" ->
      spec \ {PostClearUpdateStored}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 14
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in {
       "none",
       "initial_last_nonzero",
       "initial_report_present",
       "update_skips_report",
       "update_skips_last",
       "update_stores_wrong_key",
       "update_drops_roster_len",
       "update_drops_committed_list",
       "update_drops_no_participation_list",
       "replace_ignored",
       "replace_merges_old",
       "multiple_update_drops_old_epoch",
       "last_uses_max_epoch",
       "missing_get_synthesizes",
       "clear_keeps_reports",
       "clear_keeps_last",
       "get_mutates_store",
       "clear_blocks_future_update"
     }
  /\ checked \in 0..14
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugInitialLastNonzero ==
  ImplementationActions(InitialEmpty) = SpecActions(InitialEmpty)

BugInitialReportPresent ==
  ImplementationActions(InitialEmpty) = SpecActions(InitialEmpty)

BugUpdateSkipsReport ==
  ImplementationActions(UpdateStoresReport) = SpecActions(UpdateStoresReport)

BugUpdateSkipsLast ==
  ImplementationActions(UpdateStoresLatestEpoch) =
    SpecActions(UpdateStoresLatestEpoch)

BugUpdateStoresWrongKey ==
  ImplementationActions(UpdateStoresReport) = SpecActions(UpdateStoresReport)

BugUpdateDropsRosterLen ==
  ImplementationActions(UpdateStoresRosterLen) =
    SpecActions(UpdateStoresRosterLen)

BugUpdateDropsCommittedList ==
  ImplementationActions(UpdateStoresCommittedList) =
    SpecActions(UpdateStoresCommittedList)

BugUpdateDropsNoParticipationList ==
  ImplementationActions(UpdateStoresNoParticipationList) =
    SpecActions(UpdateStoresNoParticipationList)

BugReplaceIgnored ==
  ImplementationActions(ReplaceSameEpoch) = SpecActions(ReplaceSameEpoch)

BugReplaceMergesOld ==
  ImplementationActions(ReplaceSameEpoch) = SpecActions(ReplaceSameEpoch)

BugMultipleUpdateDropsOldEpoch ==
  ImplementationActions(MultipleEpochsPreserved) =
    SpecActions(MultipleEpochsPreserved)

BugLastUsesMaxEpoch ==
  ImplementationActions(LastEpochFollowsLatestWrite) =
    SpecActions(LastEpochFollowsLatestWrite)

BugMissingGetSynthesizes ==
  ImplementationActions(GetMissingAbsent) = SpecActions(GetMissingAbsent)

BugClearKeepsReports ==
  ImplementationActions(ClearRemovesReports) = SpecActions(ClearRemovesReports)

BugClearKeepsLast ==
  ImplementationActions(ClearResetsLatestEpoch) =
    SpecActions(ClearResetsLatestEpoch)

BugGetMutatesStore ==
  ImplementationActions(GetSideEffectFree) = SpecActions(GetSideEffectFree)

BugClearBlocksFutureUpdate ==
  ImplementationActions(ClearThenUpdateWorks) = SpecActions(ClearThenUpdateWorks)

====
