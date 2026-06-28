---- MODULE SumeragiSettlementStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi settlement telemetry status projection.

This slice captures `record_dvp_settlement_event(...)`,
`record_pvp_settlement_event(...)`, `settlement_snapshot()`,
`settlement_status_reset_for_tests()`, and the JSON Torii status projection for
the DvP/PvP `settlement` object.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetClearsDvp == 1
ResetClearsPvp == 2
DvpSuccessCounter == 3
DvpFailureCounter == 4
DvpFinalStateCounts == 5
DvpFailureReasonCounts == 6
DvpLastIdentityPlan == 7
DvpLastOutcomeState == 8
DvpLastLegs == 9
DvpRepeatedAccumulates == 10
PvpSuccessCounter == 11
PvpFailureCounter == 12
PvpFinalStateCounts == 13
PvpFailureReasonCounts == 14
PvpLastIdentityPlan == 15
PvpLastOutcomeState == 16
PvpLastLegsAndFx == 17
PvpRepeatedAccumulates == 18
SnapshotIncludesSettlement == 19
JsonProjectsDvpCounters == 20
JsonProjectsDvpLast == 21
JsonProjectsPvpCounters == 22
JsonProjectsPvpLast == 23
JsonNullLastEvents == 24
JsonProjectsPlanLabels == 25
JsonProjectsOutcomeReasonLabels == 26

Candidates == 1..26

ResetDvpCounters == 1
ResetDvpMaps == 2
ResetDvpLastEvent == 3
ResetPvpCounters == 4
ResetPvpMaps == 5
ResetPvpLastEvent == 6
DvpSuccessIncrement == 7
DvpFailurePreserved == 8
DvpFailureIncrement == 9
DvpSuccessPreserved == 10
DvpFinalStateIncrement == 11
DvpFailureReasonIncrement == 12
DvpReasonOnlyWhenPresent == 13
DvpLastObservedAt == 14
DvpLastSettlementId == 15
DvpLastPlanOrder == 16
DvpLastPlanAtomicity == 17
DvpLastOutcome == 18
DvpLastFailureReason == 19
DvpLastFinalState == 20
DvpLastDeliveryLeg == 21
DvpLastPaymentLeg == 22
DvpRepeatedAdds == 23
DvpLastEventLatest == 24
PvpSuccessIncrement == 25
PvpFailurePreserved == 26
PvpFailureIncrement == 27
PvpSuccessPreserved == 28
PvpFinalStateIncrement == 29
PvpFailureReasonIncrement == 30
PvpReasonOnlyWhenPresent == 31
PvpLastObservedAt == 32
PvpLastSettlementId == 33
PvpLastPlanOrder == 34
PvpLastPlanAtomicity == 35
PvpLastOutcome == 36
PvpLastFailureReason == 37
PvpLastFinalState == 38
PvpLastPrimaryLeg == 39
PvpLastCounterLeg == 40
PvpLastFxWindow == 41
PvpRepeatedAdds == 42
PvpLastEventLatest == 43
SnapshotHasSettlement == 44
SnapshotHasDvp == 45
SnapshotHasPvp == 46
JsonHasSettlement == 47
JsonDvpCountersMatch == 48
JsonDvpMapsMatch == 49
JsonDvpLastFieldsMatch == 50
JsonDvpLastPlanMatches == 51
JsonDvpLastLegsMatch == 52
JsonPvpCountersMatch == 53
JsonPvpMapsMatch == 54
JsonPvpLastFieldsMatch == 55
JsonPvpLastPlanMatches == 56
JsonPvpLastLegsMatch == 57
JsonPvpFxWindowMatches == 58
JsonDvpAbsentLastNull == 59
JsonPvpAbsentLastNull == 60
JsonOrderLabelsMatch == 61
JsonAtomicityLabelsMatch == 62
JsonOutcomeLabelsMatch == 63
JsonReasonNullabilityMatches == 64

DvpResetActions == {ResetDvpCounters, ResetDvpMaps, ResetDvpLastEvent}
PvpResetActions == {ResetPvpCounters, ResetPvpMaps, ResetPvpLastEvent}
DvpIdentityPlanActions ==
  {DvpLastObservedAt, DvpLastSettlementId, DvpLastPlanOrder,
   DvpLastPlanAtomicity}
DvpOutcomeStateActions ==
  {DvpLastOutcome, DvpLastFailureReason, DvpLastFinalState}
DvpLegActions == {DvpLastDeliveryLeg, DvpLastPaymentLeg}
PvpIdentityPlanActions ==
  {PvpLastObservedAt, PvpLastSettlementId, PvpLastPlanOrder,
   PvpLastPlanAtomicity}
PvpOutcomeStateActions ==
  {PvpLastOutcome, PvpLastFailureReason, PvpLastFinalState}
PvpLegFxActions == {PvpLastPrimaryLeg, PvpLastCounterLeg, PvpLastFxWindow}
SnapshotActions == {SnapshotHasSettlement, SnapshotHasDvp, SnapshotHasPvp}
DvpJsonCounterActions ==
  {JsonHasSettlement, JsonDvpCountersMatch, JsonDvpMapsMatch}
DvpJsonLastActions ==
  {JsonHasSettlement, JsonDvpLastFieldsMatch, JsonDvpLastPlanMatches,
   JsonDvpLastLegsMatch}
PvpJsonCounterActions ==
  {JsonHasSettlement, JsonPvpCountersMatch, JsonPvpMapsMatch}
PvpJsonLastActions ==
  {JsonHasSettlement, JsonPvpLastFieldsMatch, JsonPvpLastPlanMatches,
   JsonPvpLastLegsMatch, JsonPvpFxWindowMatches}
JsonPlanLabelActions == {JsonOrderLabelsMatch, JsonAtomicityLabelsMatch}
JsonOutcomeReasonActions == {JsonOutcomeLabelsMatch, JsonReasonNullabilityMatches}

SpecActions(candidate) ==
  CASE candidate = ResetClearsDvp ->
      DvpResetActions
    [] candidate = ResetClearsPvp ->
      PvpResetActions
    [] candidate = DvpSuccessCounter ->
      {DvpSuccessIncrement, DvpFailurePreserved}
    [] candidate = DvpFailureCounter ->
      {DvpFailureIncrement, DvpSuccessPreserved}
    [] candidate = DvpFinalStateCounts ->
      {DvpFinalStateIncrement}
    [] candidate = DvpFailureReasonCounts ->
      {DvpFailureReasonIncrement, DvpReasonOnlyWhenPresent}
    [] candidate = DvpLastIdentityPlan ->
      DvpIdentityPlanActions
    [] candidate = DvpLastOutcomeState ->
      DvpOutcomeStateActions
    [] candidate = DvpLastLegs ->
      DvpLegActions
    [] candidate = DvpRepeatedAccumulates ->
      {DvpRepeatedAdds, DvpLastEventLatest}
    [] candidate = PvpSuccessCounter ->
      {PvpSuccessIncrement, PvpFailurePreserved}
    [] candidate = PvpFailureCounter ->
      {PvpFailureIncrement, PvpSuccessPreserved}
    [] candidate = PvpFinalStateCounts ->
      {PvpFinalStateIncrement}
    [] candidate = PvpFailureReasonCounts ->
      {PvpFailureReasonIncrement, PvpReasonOnlyWhenPresent}
    [] candidate = PvpLastIdentityPlan ->
      PvpIdentityPlanActions
    [] candidate = PvpLastOutcomeState ->
      PvpOutcomeStateActions
    [] candidate = PvpLastLegsAndFx ->
      PvpLegFxActions
    [] candidate = PvpRepeatedAccumulates ->
      {PvpRepeatedAdds, PvpLastEventLatest}
    [] candidate = SnapshotIncludesSettlement ->
      SnapshotActions
    [] candidate = JsonProjectsDvpCounters ->
      DvpJsonCounterActions
    [] candidate = JsonProjectsDvpLast ->
      DvpJsonLastActions
    [] candidate = JsonProjectsPvpCounters ->
      PvpJsonCounterActions
    [] candidate = JsonProjectsPvpLast ->
      PvpJsonLastActions
    [] candidate = JsonNullLastEvents ->
      {JsonDvpAbsentLastNull, JsonPvpAbsentLastNull}
    [] candidate = JsonProjectsPlanLabels ->
      JsonPlanLabelActions
    [] candidate = JsonProjectsOutcomeReasonLabels ->
      JsonOutcomeReasonActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetClearsDvp /\
          Bug = "reset_keeps_dvp" ->
      spec \ DvpResetActions
    [] candidate = ResetClearsPvp /\
          Bug = "reset_keeps_pvp" ->
      spec \ PvpResetActions
    [] candidate = DvpSuccessCounter /\
          Bug = "dvp_success_not_counted" ->
      spec \ {DvpSuccessIncrement}
    [] candidate = DvpFailureCounter /\
          Bug = "dvp_failure_not_counted" ->
      spec \ {DvpFailureIncrement}
    [] candidate = DvpFinalStateCounts /\
          Bug = "dvp_final_state_not_counted" ->
      spec \ {DvpFinalStateIncrement}
    [] candidate = DvpFailureReasonCounts /\
          Bug = "dvp_failure_reason_not_counted" ->
      spec \ {DvpFailureReasonIncrement}
    [] candidate = DvpFailureReasonCounts /\
          Bug = "dvp_absent_reason_counted" ->
      spec \ {DvpReasonOnlyWhenPresent}
    [] candidate = DvpLastIdentityPlan /\
          Bug = "dvp_last_identity_dropped" ->
      spec \ {DvpLastObservedAt, DvpLastSettlementId}
    [] candidate = DvpLastIdentityPlan /\
          Bug = "dvp_last_plan_mismatch" ->
      spec \ {DvpLastPlanOrder, DvpLastPlanAtomicity}
    [] candidate = DvpLastOutcomeState /\
          Bug = "dvp_last_outcome_state_mismatch" ->
      spec \ DvpOutcomeStateActions
    [] candidate = DvpLastLegs /\
          Bug = "dvp_legs_swapped" ->
      spec \ DvpLegActions
    [] candidate = DvpRepeatedAccumulates /\
          Bug = "dvp_repeated_overwrites" ->
      spec \ {DvpRepeatedAdds}
    [] candidate = PvpSuccessCounter /\
          Bug = "pvp_success_not_counted" ->
      spec \ {PvpSuccessIncrement}
    [] candidate = PvpFailureCounter /\
          Bug = "pvp_failure_not_counted" ->
      spec \ {PvpFailureIncrement}
    [] candidate = PvpFinalStateCounts /\
          Bug = "pvp_final_state_not_counted" ->
      spec \ {PvpFinalStateIncrement}
    [] candidate = PvpFailureReasonCounts /\
          Bug = "pvp_failure_reason_not_counted" ->
      spec \ {PvpFailureReasonIncrement}
    [] candidate = PvpFailureReasonCounts /\
          Bug = "pvp_absent_reason_counted" ->
      spec \ {PvpReasonOnlyWhenPresent}
    [] candidate = PvpLastIdentityPlan /\
          Bug = "pvp_last_identity_dropped" ->
      spec \ {PvpLastObservedAt, PvpLastSettlementId}
    [] candidate = PvpLastIdentityPlan /\
          Bug = "pvp_last_plan_mismatch" ->
      spec \ {PvpLastPlanOrder, PvpLastPlanAtomicity}
    [] candidate = PvpLastOutcomeState /\
          Bug = "pvp_last_outcome_state_mismatch" ->
      spec \ PvpOutcomeStateActions
    [] candidate = PvpLastLegsAndFx /\
          Bug = "pvp_legs_or_fx_mismatch" ->
      spec \ PvpLegFxActions
    [] candidate = PvpRepeatedAccumulates /\
          Bug = "pvp_repeated_overwrites" ->
      spec \ {PvpRepeatedAdds}
    [] candidate = SnapshotIncludesSettlement /\
          Bug = "snapshot_drops_dvp" ->
      spec \ {SnapshotHasDvp}
    [] candidate = SnapshotIncludesSettlement /\
          Bug = "snapshot_drops_pvp" ->
      spec \ {SnapshotHasPvp}
    [] candidate = JsonProjectsDvpCounters /\
          Bug = "json_dvp_counters_mismatch" ->
      spec \ {JsonDvpCountersMatch, JsonDvpMapsMatch}
    [] candidate = JsonProjectsDvpLast /\
          Bug = "json_dvp_last_event_mismatch" ->
      spec \ {JsonDvpLastFieldsMatch, JsonDvpLastPlanMatches,
              JsonDvpLastLegsMatch}
    [] candidate = JsonProjectsPvpCounters /\
          Bug = "json_pvp_counters_mismatch" ->
      spec \ {JsonPvpCountersMatch, JsonPvpMapsMatch}
    [] candidate = JsonProjectsPvpLast /\
          Bug = "json_pvp_last_event_mismatch" ->
      spec \ {JsonPvpLastFieldsMatch, JsonPvpLastPlanMatches,
              JsonPvpLastLegsMatch, JsonPvpFxWindowMatches}
    [] candidate = JsonNullLastEvents /\
          Bug = "json_null_last_events_nonempty" ->
      spec \ {JsonDvpAbsentLastNull, JsonPvpAbsentLastNull}
    [] candidate = JsonProjectsPlanLabels /\
          Bug = "json_order_label_mismatch" ->
      spec \ {JsonOrderLabelsMatch}
    [] candidate = JsonProjectsPlanLabels /\
          Bug = "json_atomicity_label_mismatch" ->
      spec \ {JsonAtomicityLabelsMatch}
    [] candidate = JsonProjectsOutcomeReasonLabels /\
          Bug = "json_outcome_reason_mismatch" ->
      spec \ JsonOutcomeReasonActions
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 26
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..26

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

SettlementStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

SettlementStatusExactness ==
  /\ SettlementStatusActionsMatchSpec

SettlementStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SettlementStatusExactness

BugResetKeepsDvp ==
  ImplementationActions(ResetClearsDvp) = SpecActions(ResetClearsDvp)

BugResetKeepsPvp ==
  ImplementationActions(ResetClearsPvp) = SpecActions(ResetClearsPvp)

BugDvpSuccessNotCounted ==
  ImplementationActions(DvpSuccessCounter) = SpecActions(DvpSuccessCounter)

BugDvpFailureNotCounted ==
  ImplementationActions(DvpFailureCounter) = SpecActions(DvpFailureCounter)

BugDvpFinalStateNotCounted ==
  ImplementationActions(DvpFinalStateCounts) = SpecActions(DvpFinalStateCounts)

BugDvpFailureReasonNotCounted ==
  ImplementationActions(DvpFailureReasonCounts) = SpecActions(DvpFailureReasonCounts)

BugDvpAbsentReasonCounted ==
  ImplementationActions(DvpFailureReasonCounts) = SpecActions(DvpFailureReasonCounts)

BugDvpLastIdentityDropped ==
  ImplementationActions(DvpLastIdentityPlan) = SpecActions(DvpLastIdentityPlan)

BugDvpLastPlanMismatch ==
  ImplementationActions(DvpLastIdentityPlan) = SpecActions(DvpLastIdentityPlan)

BugDvpLastOutcomeStateMismatch ==
  ImplementationActions(DvpLastOutcomeState) = SpecActions(DvpLastOutcomeState)

BugDvpLegsSwapped ==
  ImplementationActions(DvpLastLegs) = SpecActions(DvpLastLegs)

BugDvpRepeatedOverwrites ==
  ImplementationActions(DvpRepeatedAccumulates) =
    SpecActions(DvpRepeatedAccumulates)

BugPvpSuccessNotCounted ==
  ImplementationActions(PvpSuccessCounter) = SpecActions(PvpSuccessCounter)

BugPvpFailureNotCounted ==
  ImplementationActions(PvpFailureCounter) = SpecActions(PvpFailureCounter)

BugPvpFinalStateNotCounted ==
  ImplementationActions(PvpFinalStateCounts) = SpecActions(PvpFinalStateCounts)

BugPvpFailureReasonNotCounted ==
  ImplementationActions(PvpFailureReasonCounts) = SpecActions(PvpFailureReasonCounts)

BugPvpAbsentReasonCounted ==
  ImplementationActions(PvpFailureReasonCounts) = SpecActions(PvpFailureReasonCounts)

BugPvpLastIdentityDropped ==
  ImplementationActions(PvpLastIdentityPlan) = SpecActions(PvpLastIdentityPlan)

BugPvpLastPlanMismatch ==
  ImplementationActions(PvpLastIdentityPlan) = SpecActions(PvpLastIdentityPlan)

BugPvpLastOutcomeStateMismatch ==
  ImplementationActions(PvpLastOutcomeState) = SpecActions(PvpLastOutcomeState)

BugPvpLegsOrFxMismatch ==
  ImplementationActions(PvpLastLegsAndFx) = SpecActions(PvpLastLegsAndFx)

BugPvpRepeatedOverwrites ==
  ImplementationActions(PvpRepeatedAccumulates) =
    SpecActions(PvpRepeatedAccumulates)

BugSnapshotDropsDvp ==
  ImplementationActions(SnapshotIncludesSettlement) =
    SpecActions(SnapshotIncludesSettlement)

BugSnapshotDropsPvp ==
  ImplementationActions(SnapshotIncludesSettlement) =
    SpecActions(SnapshotIncludesSettlement)

BugJsonDvpCountersMismatch ==
  ImplementationActions(JsonProjectsDvpCounters) =
    SpecActions(JsonProjectsDvpCounters)

BugJsonDvpLastEventMismatch ==
  ImplementationActions(JsonProjectsDvpLast) = SpecActions(JsonProjectsDvpLast)

BugJsonPvpCountersMismatch ==
  ImplementationActions(JsonProjectsPvpCounters) =
    SpecActions(JsonProjectsPvpCounters)

BugJsonPvpLastEventMismatch ==
  ImplementationActions(JsonProjectsPvpLast) = SpecActions(JsonProjectsPvpLast)

BugJsonNullLastEventsNonempty ==
  ImplementationActions(JsonNullLastEvents) = SpecActions(JsonNullLastEvents)

BugJsonOrderLabelMismatch ==
  ImplementationActions(JsonProjectsPlanLabels) =
    SpecActions(JsonProjectsPlanLabels)

BugJsonAtomicityLabelMismatch ==
  ImplementationActions(JsonProjectsPlanLabels) =
    SpecActions(JsonProjectsPlanLabels)

BugJsonOutcomeReasonMismatch ==
  ImplementationActions(JsonProjectsOutcomeReasonLabels) =
    SpecActions(JsonProjectsOutcomeReasonLabels)

=============================================================================
====
