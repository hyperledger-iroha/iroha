---- MODULE SumeragiQcStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi QC observability status.

This slice captures `set_leader_index(...)`, `set_highest_qc(...)`,
`set_highest_qc_hash(...)`, `highest_qc_hash()`, `set_locked_qc(...)`, and the
corresponding `snapshot()` projection fields. It pins the status-only contract:
leader and highest-QC fields store the latest observed values, highest-QC
subjects round-trip through the getter and top-level snapshot, locked-QC tuples
advance monotonically by `(height, view)`, lower locked-QC observations are
ignored, same-tuple locked-QC subject updates are accepted, and `(0, 0, None)`
clears locked-QC status.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialZero == 1
LeaderStore == 2
LeaderOverwrite == 3
HighestTupleStore == 4
HighestSubjectStore == 5
HighestGetterProjects == 6
HighestSnapshotProjects == 7
HighestOverwrite == 8
LockedReset == 9
LockedHigherAccepted == 10
LockedHigherNoneClearsSubject == 11
LockedLowerHeightIgnored == 12
LockedLowerViewIgnored == 13
LockedSameNonePreservesSubject == 14
LockedSameSubjectStores == 15
LockedSameSubjectOverwrites == 16
LockedSnapshotProjects == 17

Candidates == 1..17

InitialLeaderZero == 1
InitialHighestTupleZero == 2
InitialLockedTupleZero == 3
InitialHighestSubjectAbsent == 4
InitialLockedSubjectAbsent == 5
LeaderStored == 6
LeaderOverwriteLatest == 7
HighestHeightStored == 8
HighestViewStored == 9
HighestSubjectStored == 10
HighestGetterMatches == 11
HighestSnapshotTupleMatches == 12
HighestSnapshotSubjectMatches == 13
HighestTupleOverwriteLatest == 14
HighestSubjectOverwriteLatest == 15
LockedResetTupleZero == 16
LockedResetSubjectCleared == 17
LockedHigherHeightStored == 18
LockedHigherViewStored == 19
LockedHigherSubjectStored == 20
LockedHigherNoneClearsSubjectAction == 21
LockedLowerHeightIgnoredAction == 22
LockedLowerViewIgnoredAction == 23
LockedSameNonePreservesSubjectAction == 24
LockedSameSubjectStored == 25
LockedSameSubjectOverwriteLatest == 26
LockedSnapshotTupleMatches == 27
LockedSnapshotSubjectMatches == 28

Actions == 1..28

InitialActions ==
  {InitialLeaderZero, InitialHighestTupleZero, InitialLockedTupleZero,
   InitialHighestSubjectAbsent, InitialLockedSubjectAbsent}

HighestTupleActions == {HighestHeightStored, HighestViewStored}

HighestProjectionActions ==
  {HighestGetterMatches, HighestSnapshotTupleMatches,
   HighestSnapshotSubjectMatches}

LockedHigherActions ==
  {LockedHigherHeightStored, LockedHigherViewStored, LockedHigherSubjectStored}

SpecActions(candidate) ==
  CASE candidate = InitialZero ->
      InitialActions
    [] candidate = LeaderStore ->
      {LeaderStored}
    [] candidate = LeaderOverwrite ->
      {LeaderStored, LeaderOverwriteLatest}
    [] candidate = HighestTupleStore ->
      HighestTupleActions
    [] candidate = HighestSubjectStore ->
      {HighestSubjectStored}
    [] candidate = HighestGetterProjects ->
      {HighestGetterMatches}
    [] candidate = HighestSnapshotProjects ->
      {HighestSnapshotTupleMatches, HighestSnapshotSubjectMatches}
    [] candidate = HighestOverwrite ->
      HighestTupleActions \cup
        {HighestSubjectStored, HighestTupleOverwriteLatest,
         HighestSubjectOverwriteLatest}
    [] candidate = LockedReset ->
      {LockedResetTupleZero, LockedResetSubjectCleared}
    [] candidate = LockedHigherAccepted ->
      LockedHigherActions
    [] candidate = LockedHigherNoneClearsSubject ->
      {LockedHigherHeightStored, LockedHigherViewStored,
       LockedHigherNoneClearsSubjectAction}
    [] candidate = LockedLowerHeightIgnored ->
      {LockedLowerHeightIgnoredAction}
    [] candidate = LockedLowerViewIgnored ->
      {LockedLowerViewIgnoredAction}
    [] candidate = LockedSameNonePreservesSubject ->
      {LockedSameNonePreservesSubjectAction}
    [] candidate = LockedSameSubjectStores ->
      {LockedSameSubjectStored}
    [] candidate = LockedSameSubjectOverwrites ->
      {LockedSameSubjectStored, LockedSameSubjectOverwriteLatest}
    [] candidate = LockedSnapshotProjects ->
      {LockedSnapshotTupleMatches, LockedSnapshotSubjectMatches}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialZero /\ Bug = "initial_subjects_present" ->
      spec \ {InitialHighestSubjectAbsent, InitialLockedSubjectAbsent}
    [] candidate \in {LeaderStore, LeaderOverwrite} /\
          Bug = "leader_not_stored" ->
      spec \ {LeaderStored}
    [] candidate = LeaderOverwrite /\ Bug = "leader_overwrite_ignored" ->
      spec \ {LeaderOverwriteLatest}
    [] candidate \in {HighestTupleStore, HighestOverwrite} /\
          Bug = "highest_height_not_stored" ->
      spec \ {HighestHeightStored}
    [] candidate \in {HighestTupleStore, HighestOverwrite} /\
          Bug = "highest_view_not_stored" ->
      spec \ {HighestViewStored}
    [] candidate \in {HighestSubjectStore, HighestOverwrite} /\
          Bug = "highest_subject_not_stored" ->
      spec \ {HighestSubjectStored}
    [] candidate = HighestGetterProjects /\
          Bug = "highest_getter_missing" ->
      spec \ {HighestGetterMatches}
    [] candidate = HighestSnapshotProjects /\
          Bug = "highest_snapshot_tuple_mismatch" ->
      spec \ {HighestSnapshotTupleMatches}
    [] candidate = HighestSnapshotProjects /\
          Bug = "highest_snapshot_subject_mismatch" ->
      spec \ {HighestSnapshotSubjectMatches}
    [] candidate = HighestOverwrite /\
          Bug = "highest_overwrite_tuple_ignored" ->
      spec \ {HighestTupleOverwriteLatest}
    [] candidate = HighestOverwrite /\
          Bug = "highest_overwrite_subject_ignored" ->
      spec \ {HighestSubjectOverwriteLatest}
    [] candidate = LockedReset /\ Bug = "locked_reset_keeps_tuple" ->
      spec \ {LockedResetTupleZero}
    [] candidate = LockedReset /\ Bug = "locked_reset_keeps_subject" ->
      spec \ {LockedResetSubjectCleared}
    [] candidate \in {LockedHigherAccepted, LockedHigherNoneClearsSubject} /\
          Bug = "locked_higher_height_not_stored" ->
      spec \ {LockedHigherHeightStored}
    [] candidate \in {LockedHigherAccepted, LockedHigherNoneClearsSubject} /\
          Bug = "locked_higher_view_not_stored" ->
      spec \ {LockedHigherViewStored}
    [] candidate = LockedHigherAccepted /\
          Bug = "locked_higher_subject_not_stored" ->
      spec \ {LockedHigherSubjectStored}
    [] candidate = LockedHigherNoneClearsSubject /\
          Bug = "locked_higher_none_keeps_subject" ->
      spec \ {LockedHigherNoneClearsSubjectAction}
    [] candidate = LockedLowerHeightIgnored /\
          Bug = "locked_accepts_lower_height" ->
      spec \ {LockedLowerHeightIgnoredAction}
    [] candidate = LockedLowerViewIgnored /\
          Bug = "locked_accepts_lower_view" ->
      spec \ {LockedLowerViewIgnoredAction}
    [] candidate = LockedSameNonePreservesSubject /\
          Bug = "locked_same_none_clears_subject" ->
      spec \ {LockedSameNonePreservesSubjectAction}
    [] candidate \in {LockedSameSubjectStores, LockedSameSubjectOverwrites} /\
          Bug = "locked_same_subject_not_stored" ->
      spec \ {LockedSameSubjectStored}
    [] candidate = LockedSameSubjectOverwrites /\
          Bug = "locked_same_subject_overwrite_ignored" ->
      spec \ {LockedSameSubjectOverwriteLatest}
    [] candidate = LockedSnapshotProjects /\
          Bug = "locked_snapshot_tuple_mismatch" ->
      spec \ {LockedSnapshotTupleMatches}
    [] candidate = LockedSnapshotProjects /\
          Bug = "locked_snapshot_subject_mismatch" ->
      spec \ {LockedSnapshotSubjectMatches}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 17
     /\ checked' = checked + 1
  \/ /\ checked = 17
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in {
       "none",
       "initial_subjects_present",
       "leader_not_stored",
       "leader_overwrite_ignored",
       "highest_height_not_stored",
       "highest_view_not_stored",
       "highest_subject_not_stored",
       "highest_getter_missing",
       "highest_snapshot_tuple_mismatch",
       "highest_snapshot_subject_mismatch",
       "highest_overwrite_tuple_ignored",
       "highest_overwrite_subject_ignored",
       "locked_reset_keeps_tuple",
       "locked_reset_keeps_subject",
       "locked_higher_height_not_stored",
       "locked_higher_view_not_stored",
       "locked_higher_subject_not_stored",
       "locked_higher_none_keeps_subject",
       "locked_accepts_lower_height",
       "locked_accepts_lower_view",
       "locked_same_none_clears_subject",
       "locked_same_subject_not_stored",
       "locked_same_subject_overwrite_ignored",
       "locked_snapshot_tuple_mismatch",
       "locked_snapshot_subject_mismatch"
     }
  /\ checked \in 0..17
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugInitialSubjectsPresent ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugLeaderNotStored ==
  ImplementationActions(LeaderStore) = SpecActions(LeaderStore)

BugLeaderOverwriteIgnored ==
  ImplementationActions(LeaderOverwrite) = SpecActions(LeaderOverwrite)

BugHighestHeightNotStored ==
  ImplementationActions(HighestTupleStore) = SpecActions(HighestTupleStore)

BugHighestViewNotStored ==
  ImplementationActions(HighestTupleStore) = SpecActions(HighestTupleStore)

BugHighestSubjectNotStored ==
  ImplementationActions(HighestSubjectStore) =
    SpecActions(HighestSubjectStore)

BugHighestGetterMissing ==
  ImplementationActions(HighestGetterProjects) =
    SpecActions(HighestGetterProjects)

BugHighestSnapshotTupleMismatch ==
  ImplementationActions(HighestSnapshotProjects) =
    SpecActions(HighestSnapshotProjects)

BugHighestSnapshotSubjectMismatch ==
  ImplementationActions(HighestSnapshotProjects) =
    SpecActions(HighestSnapshotProjects)

BugHighestOverwriteTupleIgnored ==
  ImplementationActions(HighestOverwrite) = SpecActions(HighestOverwrite)

BugHighestOverwriteSubjectIgnored ==
  ImplementationActions(HighestOverwrite) = SpecActions(HighestOverwrite)

BugLockedResetKeepsTuple ==
  ImplementationActions(LockedReset) = SpecActions(LockedReset)

BugLockedResetKeepsSubject ==
  ImplementationActions(LockedReset) = SpecActions(LockedReset)

BugLockedHigherHeightNotStored ==
  ImplementationActions(LockedHigherAccepted) =
    SpecActions(LockedHigherAccepted)

BugLockedHigherViewNotStored ==
  ImplementationActions(LockedHigherAccepted) =
    SpecActions(LockedHigherAccepted)

BugLockedHigherSubjectNotStored ==
  ImplementationActions(LockedHigherAccepted) =
    SpecActions(LockedHigherAccepted)

BugLockedHigherNoneKeepsSubject ==
  ImplementationActions(LockedHigherNoneClearsSubject) =
    SpecActions(LockedHigherNoneClearsSubject)

BugLockedAcceptsLowerHeight ==
  ImplementationActions(LockedLowerHeightIgnored) =
    SpecActions(LockedLowerHeightIgnored)

BugLockedAcceptsLowerView ==
  ImplementationActions(LockedLowerViewIgnored) =
    SpecActions(LockedLowerViewIgnored)

BugLockedSameNoneClearsSubject ==
  ImplementationActions(LockedSameNonePreservesSubject) =
    SpecActions(LockedSameNonePreservesSubject)

BugLockedSameSubjectNotStored ==
  ImplementationActions(LockedSameSubjectStores) =
    SpecActions(LockedSameSubjectStores)

BugLockedSameSubjectOverwriteIgnored ==
  ImplementationActions(LockedSameSubjectOverwrites) =
    SpecActions(LockedSameSubjectOverwrites)

BugLockedSnapshotTupleMismatch ==
  ImplementationActions(LockedSnapshotProjects) =
    SpecActions(LockedSnapshotProjects)

BugLockedSnapshotSubjectMismatch ==
  ImplementationActions(LockedSnapshotProjects) =
    SpecActions(LockedSnapshotProjects)

AllQcStatusCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

InitialStatusAnchors ==
  InitialActions \subseteq ImplementationActions(InitialZero)

LeaderStorageAnchors ==
  /\ LeaderStored \in ImplementationActions(LeaderStore)
  /\ LeaderStored \in ImplementationActions(LeaderOverwrite)
  /\ LeaderOverwriteLatest \in ImplementationActions(LeaderOverwrite)

HighestQcStorageAnchors ==
  /\ HighestTupleActions \subseteq ImplementationActions(HighestTupleStore)
  /\ HighestSubjectStored \in ImplementationActions(HighestSubjectStore)

HighestQcProjectionAnchors ==
  /\ HighestGetterMatches \in ImplementationActions(HighestGetterProjects)
  /\ HighestProjectionActions \ {HighestGetterMatches} \subseteq
       ImplementationActions(HighestSnapshotProjects)

HighestOverwriteAnchors ==
  /\ HighestTupleActions \subseteq ImplementationActions(HighestOverwrite)
  /\ HighestSubjectStored \in ImplementationActions(HighestOverwrite)
  /\ HighestTupleOverwriteLatest \in ImplementationActions(HighestOverwrite)
  /\ HighestSubjectOverwriteLatest \in
       ImplementationActions(HighestOverwrite)

LockedResetAnchors ==
  /\ LockedResetTupleZero \in ImplementationActions(LockedReset)
  /\ LockedResetSubjectCleared \in ImplementationActions(LockedReset)

LockedHigherAnchors ==
  /\ LockedHigherActions \subseteq ImplementationActions(LockedHigherAccepted)
  /\ LockedHigherHeightStored \in
       ImplementationActions(LockedHigherNoneClearsSubject)
  /\ LockedHigherViewStored \in
       ImplementationActions(LockedHigherNoneClearsSubject)
  /\ LockedHigherNoneClearsSubjectAction \in
       ImplementationActions(LockedHigherNoneClearsSubject)
  /\ ~(LockedHigherSubjectStored \in
       ImplementationActions(LockedHigherNoneClearsSubject))

LockedMonotonicityAnchors ==
  /\ LockedLowerHeightIgnoredAction \in
       ImplementationActions(LockedLowerHeightIgnored)
  /\ LockedLowerViewIgnoredAction \in
       ImplementationActions(LockedLowerViewIgnored)
  /\ ~(LockedHigherHeightStored \in
       ImplementationActions(LockedLowerHeightIgnored))
  /\ ~(LockedHigherViewStored \in
       ImplementationActions(LockedLowerViewIgnored))

LockedSameTupleAnchors ==
  /\ LockedSameNonePreservesSubjectAction \in
       ImplementationActions(LockedSameNonePreservesSubject)
  /\ LockedSameSubjectStored \in
       ImplementationActions(LockedSameSubjectStores)
  /\ LockedSameSubjectStored \in
       ImplementationActions(LockedSameSubjectOverwrites)
  /\ LockedSameSubjectOverwriteLatest \in
       ImplementationActions(LockedSameSubjectOverwrites)

LockedSnapshotAnchors ==
  /\ LockedSnapshotTupleMatches \in
       ImplementationActions(LockedSnapshotProjects)
  /\ LockedSnapshotSubjectMatches \in
       ImplementationActions(LockedSnapshotProjects)

SafetyAnchors ==
  /\ AllQcStatusCandidatesMatchSpec
  /\ InitialStatusAnchors
  /\ LeaderStorageAnchors
  /\ HighestQcStorageAnchors
  /\ HighestQcProjectionAnchors
  /\ HighestOverwriteAnchors
  /\ LockedResetAnchors
  /\ LockedHigherAnchors
  /\ LockedMonotonicityAnchors
  /\ LockedSameTupleAnchors
  /\ LockedSnapshotAnchors

QcStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ Safety
  /\ SafetyAnchors

====
