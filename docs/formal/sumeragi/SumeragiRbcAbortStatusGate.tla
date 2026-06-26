---- MODULE SumeragiRbcAbortStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi RBC abort status accounting.

This slice captures `record_rbc_abort(...)`, `rbc_abort_snapshot()`, the
`snapshot().rbc_abort` projection, and the test-only
`reset_rbc_abort_counters_for_tests()` helper from `status.rs`: abort totals,
latest height/view recording, lower-slot and zero-slot behavior, snapshot
projection, and reset semantics.
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
FirstRecord == 2
SecondRecord == 3
LatestHeightUpdates == 4
LatestViewUpdates == 5
LowerHeightStillUpdates == 6
ZeroSlotRecords == 7
DirectSnapshotMatches == 8
StatusSnapshotProjects == 9
ResetAfterRecordsClears == 10

Candidates == 1..10

ResetTotal == 1
ResetHeight == 2
ResetView == 3
IncrementTotal == 4
AccumulateTotal == 5
StoreHeight == 6
StoreView == 7
OverwriteHeight == 8
OverwriteView == 9
LowerHeightAccepted == 10
ZeroHeightAccepted == 11
ZeroViewAccepted == 12
DirectSnapshotTotal == 13
DirectSnapshotHeight == 14
DirectSnapshotView == 15
StatusSnapshotAbort == 16
NoAbortEntryAfterReset == 17

Actions == 1..17

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      {ResetTotal, ResetHeight, ResetView, NoAbortEntryAfterReset}
    [] candidate = FirstRecord ->
      {IncrementTotal, StoreHeight, StoreView, DirectSnapshotTotal,
       DirectSnapshotHeight, DirectSnapshotView}
    [] candidate = SecondRecord ->
      {AccumulateTotal, OverwriteHeight, OverwriteView, DirectSnapshotTotal,
       DirectSnapshotHeight, DirectSnapshotView}
    [] candidate = LatestHeightUpdates ->
      {OverwriteHeight, DirectSnapshotHeight}
    [] candidate = LatestViewUpdates ->
      {OverwriteView, DirectSnapshotView}
    [] candidate = LowerHeightStillUpdates ->
      {LowerHeightAccepted, OverwriteHeight, OverwriteView}
    [] candidate = ZeroSlotRecords ->
      {ZeroHeightAccepted, ZeroViewAccepted, StoreHeight, StoreView}
    [] candidate = DirectSnapshotMatches ->
      {DirectSnapshotTotal, DirectSnapshotHeight, DirectSnapshotView}
    [] candidate = StatusSnapshotProjects ->
      {StatusSnapshotAbort, DirectSnapshotTotal, DirectSnapshotHeight,
       DirectSnapshotView}
    [] candidate = ResetAfterRecordsClears ->
      {ResetTotal, ResetHeight, ResetView, NoAbortEntryAfterReset}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_total" ->
      spec \ {ResetTotal, NoAbortEntryAfterReset}
    [] candidate = FirstRecord /\ Bug = "first_record_not_counted" ->
      spec \ {IncrementTotal, DirectSnapshotTotal}
    [] candidate = FirstRecord /\ Bug = "first_height_missing" ->
      spec \ {StoreHeight, DirectSnapshotHeight}
    [] candidate = FirstRecord /\ Bug = "first_view_missing" ->
      spec \ {StoreView, DirectSnapshotView}
    [] candidate = SecondRecord /\ Bug = "second_overwrites_total" ->
      (spec \ {AccumulateTotal}) \cup {IncrementTotal}
    [] candidate = SecondRecord /\ Bug = "second_keeps_old_height" ->
      spec \ {OverwriteHeight, DirectSnapshotHeight}
    [] candidate = SecondRecord /\ Bug = "second_keeps_old_view" ->
      spec \ {OverwriteView, DirectSnapshotView}
    [] candidate = LowerHeightStillUpdates /\ Bug = "lower_height_ignored" ->
      spec \ {LowerHeightAccepted, OverwriteHeight, OverwriteView}
    [] candidate = ZeroSlotRecords /\ Bug = "zero_slot_ignored" ->
      spec \ {ZeroHeightAccepted, ZeroViewAccepted, StoreHeight, StoreView}
    [] candidate = StatusSnapshotProjects /\ Bug = "status_snapshot_omits_abort" ->
      spec \ {StatusSnapshotAbort}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_last" ->
      spec \ {ResetHeight, ResetView, NoAbortEntryAfterReset}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 10
     /\ checked' = checked + 1
  \/ /\ checked = 10
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..10

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsTotal ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugFirstRecordNotCounted ==
  ImplementationActions(FirstRecord) = SpecActions(FirstRecord)

BugFirstHeightMissing ==
  ImplementationActions(FirstRecord) = SpecActions(FirstRecord)

BugFirstViewMissing ==
  ImplementationActions(FirstRecord) = SpecActions(FirstRecord)

BugSecondOverwritesTotal ==
  ImplementationActions(SecondRecord) = SpecActions(SecondRecord)

BugSecondKeepsOldHeight ==
  ImplementationActions(SecondRecord) = SpecActions(SecondRecord)

BugSecondKeepsOldView ==
  ImplementationActions(SecondRecord) = SpecActions(SecondRecord)

BugLowerHeightIgnored ==
  ImplementationActions(LowerHeightStillUpdates) =
    SpecActions(LowerHeightStillUpdates)

BugZeroSlotIgnored ==
  ImplementationActions(ZeroSlotRecords) = SpecActions(ZeroSlotRecords)

BugStatusSnapshotOmitsAbort ==
  ImplementationActions(StatusSnapshotProjects) =
    SpecActions(StatusSnapshotProjects)

BugResetAfterRecordsKeepsLast ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

AllRbcAbortCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ResetAnchors ==
  /\ {ResetTotal, ResetHeight, ResetView, NoAbortEntryAfterReset} \subseteq
       ImplementationActions(ResetEmpty)
  /\ {ResetTotal, ResetHeight, ResetView, NoAbortEntryAfterReset} \subseteq
       ImplementationActions(ResetAfterRecordsClears)

FirstRecordAnchors ==
  /\ {IncrementTotal, StoreHeight, StoreView, DirectSnapshotTotal,
      DirectSnapshotHeight, DirectSnapshotView} \subseteq
       ImplementationActions(FirstRecord)

SecondRecordAnchors ==
  /\ {AccumulateTotal, OverwriteHeight, OverwriteView, DirectSnapshotTotal,
      DirectSnapshotHeight, DirectSnapshotView} \subseteq
       ImplementationActions(SecondRecord)
  /\ ~(IncrementTotal \in ImplementationActions(SecondRecord))

LatestSlotAnchors ==
  /\ {OverwriteHeight, DirectSnapshotHeight} \subseteq
       ImplementationActions(LatestHeightUpdates)
  /\ {OverwriteView, DirectSnapshotView} \subseteq
       ImplementationActions(LatestViewUpdates)
  /\ {LowerHeightAccepted, OverwriteHeight, OverwriteView} \subseteq
       ImplementationActions(LowerHeightStillUpdates)

ZeroSlotAnchors ==
  /\ {ZeroHeightAccepted, ZeroViewAccepted, StoreHeight, StoreView} \subseteq
       ImplementationActions(ZeroSlotRecords)

SnapshotAnchors ==
  /\ {DirectSnapshotTotal, DirectSnapshotHeight, DirectSnapshotView} \subseteq
       ImplementationActions(DirectSnapshotMatches)
  /\ {StatusSnapshotAbort, DirectSnapshotTotal, DirectSnapshotHeight,
      DirectSnapshotView} \subseteq
       ImplementationActions(StatusSnapshotProjects)

SafetyAnchors ==
  /\ AllRbcAbortCandidatesMatchSpec
  /\ ResetAnchors
  /\ FirstRecordAnchors
  /\ SecondRecordAnchors
  /\ LatestSlotAnchors
  /\ ZeroSlotAnchors
  /\ SnapshotAnchors

RbcAbortStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ Safety
  /\ SafetyAnchors

=============================================================================
====
