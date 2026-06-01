---- MODULE SumeragiRoundGapStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi round-gap status accounting.

This slice captures `record_round_gap_deliver(...)`,
`record_round_gap_state_commit(...)`, `record_round_gap_unblocked(...)`, the
`round_gap_snapshot()` projection, marker pruning, `TimingEma` initialization
and update behavior, and the test-only `reset_round_gap_status_for_tests()`
helper from `status.rs`.
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
DeliverStoresFirstMarker == 2
DeliverRepeatKeepsFirstMarker == 3
StateCommitStoresMarker == 4
StateCommitRepeatKeepsFirstMarker == 5
UnblockStoresMarker == 6
UnblockRepeatKeepsFirstMarker == 7
StateCommitAloneNoSnapshot == 8
UnblockAloneNoSnapshot == 9
DeliverThenStateCommitNoSnapshot == 10
DeliverThenUnblockNoSnapshot == 11
MismatchedHeightIsolated == 12
MismatchedViewIsolated == 13
MismatchedHashIsolated == 14
CompleteInOrderRecordsDurations == 15
CompleteOutOfOrderSaturates == 16
CompleteRemovesMarker == 17
FirstSampleEmaEqualsLast == 18
SecondSampleEmaBlends == 19
LaterDeliverPreservesLast == 20
MarkerCapKeepsAtLimit == 21
MarkerCapEvictsSmallestOverLimit == 22
DurationOverflowClamps == 23
SnapshotProjectsRoundGap == 24
ResetAfterRecordsClears == 25

Candidates == 1..25

ResetSnapshot == 1
ResetMarkers == 2
ResetEmaState == 3
DeliverMarkerStored == 4
DeliverRepeatPreservesFirst == 5
StateCommitMarkerStored == 6
StateCommitRepeatPreservesFirst == 7
UnblockMarkerStored == 8
UnblockRepeatPreservesFirst == 9
NoSnapshotUpdate == 10
HeightKeyMatched == 11
ViewKeyMatched == 12
HashKeyMatched == 13
MismatchedKeyIsolated == 14
LastDeliverToStateCommitStored == 15
LastStateCommitToNextProposeStored == 16
LastDeliverToNextProposeStored == 17
CombinedDurationAtLeastFirstLeg == 18
OutOfOrderSaturatingDuration == 19
CompletedMarkerRemoved == 20
FirstEmaEqualsLastSample == 21
SecondEmaUsesAlphaBlend == 22
IncompleteLaterDeliverPreservesSnapshot == 23
CapAtLimitPreservesMarkers == 24
CapOverLimitDropsSmallest == 25
SnapshotFieldsProjected == 26
OverflowDurationClamped == 27

Actions == 1..27

AllResetActions ==
  {ResetSnapshot, ResetMarkers, ResetEmaState}

AllMarkersStored ==
  {DeliverMarkerStored, StateCommitMarkerStored, UnblockMarkerStored}

AllDurationActions ==
  {LastDeliverToStateCommitStored, LastStateCommitToNextProposeStored,
   LastDeliverToNextProposeStored}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = DeliverStoresFirstMarker ->
      {DeliverMarkerStored, NoSnapshotUpdate}
    [] candidate = DeliverRepeatKeepsFirstMarker ->
      {DeliverMarkerStored, DeliverRepeatPreservesFirst, NoSnapshotUpdate}
    [] candidate = StateCommitStoresMarker ->
      {StateCommitMarkerStored, NoSnapshotUpdate}
    [] candidate = StateCommitRepeatKeepsFirstMarker ->
      {StateCommitMarkerStored, StateCommitRepeatPreservesFirst,
       NoSnapshotUpdate}
    [] candidate = UnblockStoresMarker ->
      {UnblockMarkerStored, NoSnapshotUpdate}
    [] candidate = UnblockRepeatKeepsFirstMarker ->
      {UnblockMarkerStored, UnblockRepeatPreservesFirst, NoSnapshotUpdate}
    [] candidate = StateCommitAloneNoSnapshot ->
      {StateCommitMarkerStored, NoSnapshotUpdate}
    [] candidate = UnblockAloneNoSnapshot ->
      {UnblockMarkerStored, NoSnapshotUpdate}
    [] candidate = DeliverThenStateCommitNoSnapshot ->
      {DeliverMarkerStored, StateCommitMarkerStored, NoSnapshotUpdate}
    [] candidate = DeliverThenUnblockNoSnapshot ->
      {DeliverMarkerStored, UnblockMarkerStored, NoSnapshotUpdate}
    [] candidate = MismatchedHeightIsolated ->
      {HeightKeyMatched, MismatchedKeyIsolated, NoSnapshotUpdate}
    [] candidate = MismatchedViewIsolated ->
      {ViewKeyMatched, MismatchedKeyIsolated, NoSnapshotUpdate}
    [] candidate = MismatchedHashIsolated ->
      {HashKeyMatched, MismatchedKeyIsolated, NoSnapshotUpdate}
    [] candidate = CompleteInOrderRecordsDurations ->
      AllMarkersStored \cup AllDurationActions \cup
        {CombinedDurationAtLeastFirstLeg, CompletedMarkerRemoved,
         SnapshotFieldsProjected}
    [] candidate = CompleteOutOfOrderSaturates ->
      AllMarkersStored \cup AllDurationActions \cup
        {OutOfOrderSaturatingDuration, CompletedMarkerRemoved}
    [] candidate = CompleteRemovesMarker ->
      {CompletedMarkerRemoved}
    [] candidate = FirstSampleEmaEqualsLast ->
      AllDurationActions \cup {FirstEmaEqualsLastSample}
    [] candidate = SecondSampleEmaBlends ->
      AllDurationActions \cup {SecondEmaUsesAlphaBlend}
    [] candidate = LaterDeliverPreservesLast ->
      {DeliverMarkerStored, IncompleteLaterDeliverPreservesSnapshot}
    [] candidate = MarkerCapKeepsAtLimit ->
      {CapAtLimitPreservesMarkers}
    [] candidate = MarkerCapEvictsSmallestOverLimit ->
      {CapOverLimitDropsSmallest}
    [] candidate = DurationOverflowClamps ->
      {OverflowDurationClamped, LastDeliverToNextProposeStored}
    [] candidate = SnapshotProjectsRoundGap ->
      AllDurationActions \cup {SnapshotFieldsProjected}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_snapshot" ->
      spec \ {ResetSnapshot}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_markers" ->
      spec \ {ResetMarkers}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_ema" ->
      spec \ {ResetEmaState}
    [] candidate = DeliverStoresFirstMarker /\
          Bug = "deliver_marker_not_stored" ->
      spec \ {DeliverMarkerStored}
    [] candidate = DeliverRepeatKeepsFirstMarker /\
          Bug = "repeat_deliver_overwrites_first" ->
      spec \ {DeliverRepeatPreservesFirst}
    [] candidate = StateCommitStoresMarker /\
          Bug = "state_commit_marker_not_stored" ->
      spec \ {StateCommitMarkerStored}
    [] candidate = StateCommitRepeatKeepsFirstMarker /\
          Bug = "repeat_state_commit_overwrites_first" ->
      spec \ {StateCommitRepeatPreservesFirst}
    [] candidate = UnblockStoresMarker /\
          Bug = "unblock_marker_not_stored" ->
      spec \ {UnblockMarkerStored}
    [] candidate = UnblockRepeatKeepsFirstMarker /\
          Bug = "repeat_unblock_overwrites_first" ->
      spec \ {UnblockRepeatPreservesFirst}
    [] candidate = StateCommitAloneNoSnapshot /\
          Bug = "state_commit_alone_updates_snapshot" ->
      (spec \ {NoSnapshotUpdate}) \cup AllDurationActions
    [] candidate = UnblockAloneNoSnapshot /\
          Bug = "unblock_alone_updates_snapshot" ->
      (spec \ {NoSnapshotUpdate}) \cup AllDurationActions
    [] candidate = DeliverThenStateCommitNoSnapshot /\
          Bug = "missing_unblock_updates_snapshot" ->
      (spec \ {NoSnapshotUpdate}) \cup AllDurationActions
    [] candidate = DeliverThenUnblockNoSnapshot /\
          Bug = "missing_state_commit_updates_snapshot" ->
      (spec \ {NoSnapshotUpdate}) \cup AllDurationActions
    [] candidate = MismatchedHeightIsolated /\
          Bug = "height_mismatch_completes" ->
      (spec \ {MismatchedKeyIsolated, NoSnapshotUpdate}) \cup
        AllDurationActions
    [] candidate = MismatchedViewIsolated /\
          Bug = "view_mismatch_completes" ->
      (spec \ {MismatchedKeyIsolated, NoSnapshotUpdate}) \cup
        AllDurationActions
    [] candidate = MismatchedHashIsolated /\
          Bug = "hash_mismatch_completes" ->
      (spec \ {MismatchedKeyIsolated, NoSnapshotUpdate}) \cup
        AllDurationActions
    [] candidate = CompleteInOrderRecordsDurations /\
          Bug = "complete_skips_deliver_to_commit" ->
      spec \ {LastDeliverToStateCommitStored}
    [] candidate = CompleteInOrderRecordsDurations /\
          Bug = "complete_skips_commit_to_unblock" ->
      spec \ {LastStateCommitToNextProposeStored}
    [] candidate = CompleteInOrderRecordsDurations /\
          Bug = "complete_skips_deliver_to_unblock" ->
      spec \ {LastDeliverToNextProposeStored}
    [] candidate = CompleteInOrderRecordsDurations /\
          Bug = "complete_combined_under_first_leg" ->
      spec \ {CombinedDurationAtLeastFirstLeg}
    [] candidate = CompleteOutOfOrderSaturates /\
          Bug = "out_of_order_wraps_duration" ->
      spec \ {OutOfOrderSaturatingDuration}
    [] candidate = CompleteRemovesMarker /\ Bug = "complete_keeps_marker" ->
      spec \ {CompletedMarkerRemoved}
    [] candidate = FirstSampleEmaEqualsLast /\
          Bug = "first_ema_not_initialized" ->
      spec \ {FirstEmaEqualsLastSample}
    [] candidate = SecondSampleEmaBlends /\
          Bug = "second_ema_overwrites_without_blend" ->
      spec \ {SecondEmaUsesAlphaBlend}
    [] candidate = LaterDeliverPreservesLast /\
          Bug = "later_deliver_overwrites_last" ->
      spec \ {IncompleteLaterDeliverPreservesSnapshot}
    [] candidate = MarkerCapKeepsAtLimit /\ Bug = "cap_at_limit_prunes" ->
      spec \ {CapAtLimitPreservesMarkers}
    [] candidate = MarkerCapEvictsSmallestOverLimit /\
          Bug = "cap_over_limit_keeps_smallest" ->
      spec \ {CapOverLimitDropsSmallest}
    [] candidate = DurationOverflowClamps /\ Bug = "overflow_duration_wraps" ->
      spec \ {OverflowDurationClamped}
    [] candidate = SnapshotProjectsRoundGap /\ Bug = "snapshot_drops_fields" ->
      spec \ {SnapshotFieldsProjected}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_snapshot" ->
      spec \ {ResetSnapshot}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_markers" ->
      spec \ {ResetMarkers}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_ema" ->
      spec \ {ResetEmaState}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..25

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllSpecActionsWithinDomain ==
  \A candidate \in Candidates:
    SpecActions(candidate) \subseteq Actions

AllImplementationActionsWithinDomain ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) \subseteq Actions

ResetAnchors ==
  /\ ImplementationActions(ResetEmpty) = AllResetActions
  /\ ImplementationActions(ResetAfterRecordsClears) = AllResetActions

MarkerStoreAnchors ==
  /\ DeliverMarkerStored \in ImplementationActions(DeliverStoresFirstMarker)
  /\ DeliverRepeatPreservesFirst \in
       ImplementationActions(DeliverRepeatKeepsFirstMarker)
  /\ StateCommitMarkerStored \in
       ImplementationActions(StateCommitStoresMarker)
  /\ StateCommitRepeatPreservesFirst \in
       ImplementationActions(StateCommitRepeatKeepsFirstMarker)
  /\ UnblockMarkerStored \in ImplementationActions(UnblockStoresMarker)
  /\ UnblockRepeatPreservesFirst \in
       ImplementationActions(UnblockRepeatKeepsFirstMarker)

IncompleteIsolationAnchors ==
  /\ NoSnapshotUpdate \in ImplementationActions(StateCommitAloneNoSnapshot)
  /\ NoSnapshotUpdate \in ImplementationActions(UnblockAloneNoSnapshot)
  /\ NoSnapshotUpdate \in
       ImplementationActions(DeliverThenStateCommitNoSnapshot)
  /\ NoSnapshotUpdate \in
       ImplementationActions(DeliverThenUnblockNoSnapshot)
  /\ ~(LastDeliverToNextProposeStored \in
       ImplementationActions(DeliverThenStateCommitNoSnapshot))
  /\ ~(LastDeliverToStateCommitStored \in
       ImplementationActions(DeliverThenUnblockNoSnapshot))

MismatchIsolationAnchors ==
  /\ HeightKeyMatched \in ImplementationActions(MismatchedHeightIsolated)
  /\ ViewKeyMatched \in ImplementationActions(MismatchedViewIsolated)
  /\ HashKeyMatched \in ImplementationActions(MismatchedHashIsolated)
  /\ MismatchedKeyIsolated \in
       ImplementationActions(MismatchedHeightIsolated)
  /\ MismatchedKeyIsolated \in
       ImplementationActions(MismatchedViewIsolated)
  /\ MismatchedKeyIsolated \in
       ImplementationActions(MismatchedHashIsolated)
  /\ NoSnapshotUpdate \in ImplementationActions(MismatchedHeightIsolated)
  /\ NoSnapshotUpdate \in ImplementationActions(MismatchedViewIsolated)
  /\ NoSnapshotUpdate \in ImplementationActions(MismatchedHashIsolated)

CompletionDurationAnchors ==
  /\ AllMarkersStored \subseteq ImplementationActions(CompleteInOrderRecordsDurations)
  /\ AllDurationActions \subseteq
       ImplementationActions(CompleteInOrderRecordsDurations)
  /\ CombinedDurationAtLeastFirstLeg \in
       ImplementationActions(CompleteInOrderRecordsDurations)
  /\ CompletedMarkerRemoved \in
       ImplementationActions(CompleteInOrderRecordsDurations)
  /\ SnapshotFieldsProjected \in
       ImplementationActions(CompleteInOrderRecordsDurations)
  /\ OutOfOrderSaturatingDuration \in
       ImplementationActions(CompleteOutOfOrderSaturates)
  /\ CompletedMarkerRemoved \in
       ImplementationActions(CompleteOutOfOrderSaturates)
  /\ CompletedMarkerRemoved \in ImplementationActions(CompleteRemovesMarker)

EmaAnchors ==
  /\ AllDurationActions \subseteq ImplementationActions(FirstSampleEmaEqualsLast)
  /\ FirstEmaEqualsLastSample \in ImplementationActions(FirstSampleEmaEqualsLast)
  /\ AllDurationActions \subseteq ImplementationActions(SecondSampleEmaBlends)
  /\ SecondEmaUsesAlphaBlend \in ImplementationActions(SecondSampleEmaBlends)
  /\ IncompleteLaterDeliverPreservesSnapshot \in
       ImplementationActions(LaterDeliverPreservesLast)

PruneAndOverflowAnchors ==
  /\ CapAtLimitPreservesMarkers \in ImplementationActions(MarkerCapKeepsAtLimit)
  /\ CapOverLimitDropsSmallest \in
       ImplementationActions(MarkerCapEvictsSmallestOverLimit)
  /\ OverflowDurationClamped \in ImplementationActions(DurationOverflowClamps)
  /\ LastDeliverToNextProposeStored \in
       ImplementationActions(DurationOverflowClamps)

SnapshotAnchors ==
  /\ AllDurationActions \subseteq ImplementationActions(SnapshotProjectsRoundGap)
  /\ SnapshotFieldsProjected \in ImplementationActions(SnapshotProjectsRoundGap)

RoundGapSafetyAnchors ==
  /\ AllCandidatesMatchSpec
  /\ AllSpecActionsWithinDomain
  /\ AllImplementationActionsWithinDomain
  /\ ResetAnchors
  /\ MarkerStoreAnchors
  /\ IncompleteIsolationAnchors
  /\ MismatchIsolationAnchors
  /\ CompletionDurationAnchors
  /\ EmaAnchors
  /\ PruneAndOverflowAnchors
  /\ SnapshotAnchors

BugResetEmptyKeepsSnapshot ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsMarkers ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsEma ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugDeliverMarkerNotStored ==
  ImplementationActions(DeliverStoresFirstMarker) =
    SpecActions(DeliverStoresFirstMarker)

BugRepeatDeliverOverwritesFirst ==
  ImplementationActions(DeliverRepeatKeepsFirstMarker) =
    SpecActions(DeliverRepeatKeepsFirstMarker)

BugStateCommitMarkerNotStored ==
  ImplementationActions(StateCommitStoresMarker) =
    SpecActions(StateCommitStoresMarker)

BugRepeatStateCommitOverwritesFirst ==
  ImplementationActions(StateCommitRepeatKeepsFirstMarker) =
    SpecActions(StateCommitRepeatKeepsFirstMarker)

BugUnblockMarkerNotStored ==
  ImplementationActions(UnblockStoresMarker) =
    SpecActions(UnblockStoresMarker)

BugRepeatUnblockOverwritesFirst ==
  ImplementationActions(UnblockRepeatKeepsFirstMarker) =
    SpecActions(UnblockRepeatKeepsFirstMarker)

BugStateCommitAloneUpdatesSnapshot ==
  ImplementationActions(StateCommitAloneNoSnapshot) =
    SpecActions(StateCommitAloneNoSnapshot)

BugUnblockAloneUpdatesSnapshot ==
  ImplementationActions(UnblockAloneNoSnapshot) =
    SpecActions(UnblockAloneNoSnapshot)

BugMissingUnblockUpdatesSnapshot ==
  ImplementationActions(DeliverThenStateCommitNoSnapshot) =
    SpecActions(DeliverThenStateCommitNoSnapshot)

BugMissingStateCommitUpdatesSnapshot ==
  ImplementationActions(DeliverThenUnblockNoSnapshot) =
    SpecActions(DeliverThenUnblockNoSnapshot)

BugHeightMismatchCompletes ==
  ImplementationActions(MismatchedHeightIsolated) =
    SpecActions(MismatchedHeightIsolated)

BugViewMismatchCompletes ==
  ImplementationActions(MismatchedViewIsolated) =
    SpecActions(MismatchedViewIsolated)

BugHashMismatchCompletes ==
  ImplementationActions(MismatchedHashIsolated) =
    SpecActions(MismatchedHashIsolated)

BugCompleteSkipsDeliverToCommit ==
  ImplementationActions(CompleteInOrderRecordsDurations) =
    SpecActions(CompleteInOrderRecordsDurations)

BugCompleteSkipsCommitToUnblock ==
  ImplementationActions(CompleteInOrderRecordsDurations) =
    SpecActions(CompleteInOrderRecordsDurations)

BugCompleteSkipsDeliverToUnblock ==
  ImplementationActions(CompleteInOrderRecordsDurations) =
    SpecActions(CompleteInOrderRecordsDurations)

BugCompleteCombinedUnderFirstLeg ==
  ImplementationActions(CompleteInOrderRecordsDurations) =
    SpecActions(CompleteInOrderRecordsDurations)

BugOutOfOrderWrapsDuration ==
  ImplementationActions(CompleteOutOfOrderSaturates) =
    SpecActions(CompleteOutOfOrderSaturates)

BugCompleteKeepsMarker ==
  ImplementationActions(CompleteRemovesMarker) =
    SpecActions(CompleteRemovesMarker)

BugFirstEmaNotInitialized ==
  ImplementationActions(FirstSampleEmaEqualsLast) =
    SpecActions(FirstSampleEmaEqualsLast)

BugSecondEmaOverwritesWithoutBlend ==
  ImplementationActions(SecondSampleEmaBlends) =
    SpecActions(SecondSampleEmaBlends)

BugLaterDeliverOverwritesLast ==
  ImplementationActions(LaterDeliverPreservesLast) =
    SpecActions(LaterDeliverPreservesLast)

BugCapAtLimitPrunes ==
  ImplementationActions(MarkerCapKeepsAtLimit) =
    SpecActions(MarkerCapKeepsAtLimit)

BugCapOverLimitKeepsSmallest ==
  ImplementationActions(MarkerCapEvictsSmallestOverLimit) =
    SpecActions(MarkerCapEvictsSmallestOverLimit)

BugOverflowDurationWraps ==
  ImplementationActions(DurationOverflowClamps) =
    SpecActions(DurationOverflowClamps)

BugSnapshotDropsFields ==
  ImplementationActions(SnapshotProjectsRoundGap) =
    SpecActions(SnapshotProjectsRoundGap)

BugResetAfterRecordsKeepsSnapshot ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsMarkers ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsEma ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
