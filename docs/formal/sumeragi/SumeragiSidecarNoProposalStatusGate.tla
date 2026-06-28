---- MODULE SumeragiSidecarNoProposalStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi sidecar/no-proposal status accounting.

This slice captures `inc_consensus_sidecar_quarantine()`,
`inc_consensus_sidecar_final_drop()`,
`inc_consensus_sidecar_recovery_trigger()`,
`inc_consensus_no_proposal_storm()`,
`observe_consensus_no_proposal_storm_state(...)`, their `snapshot()`
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
SidecarQuarantineRecord == 2
SidecarFinalDropRecord == 3
SidecarRecoveryTriggerRecord == 4
NoProposalStormRecord == 5
RepeatedSidecarQuarantineAccumulates == 6
RepeatedNoProposalStormAccumulates == 7
ObserveStormSetsFields == 8
ObserveStormFloorsMaxToCount == 9
ObserveStormPreservesHigherMax == 10
ObserveStormRaisesMax == 11
ObserveStormOverwritesLast == 12
SnapshotProjectsSidecar == 13
SnapshotProjectsNoProposal == 14
ResetAfterRecordsClears == 15

Candidates == 1..15

ResetSidecarCounters == 1
ResetNoProposalCounters == 2
ResetNoProposalDiagnostics == 3
IncrementSidecarQuarantine == 4
IncrementSidecarFinalDrop == 5
IncrementSidecarRecoveryTrigger == 6
IncrementNoProposalStorm == 7
SameCounterAccumulates == 8
SetStormLastHeight == 9
SetStormLastCount == 10
SetStormMaxFromObservation == 11
StormMaxFloorsToCount == 12
StormMaxPreservesHigher == 13
StormMaxRaisesOnHigher == 14
StormLastOverwrites == 15
SnapshotSidecarMatches == 16
SnapshotNoProposalMatches == 17

Actions == 1..17

AllResetActions ==
  {ResetSidecarCounters, ResetNoProposalCounters,
   ResetNoProposalDiagnostics}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = SidecarQuarantineRecord ->
      {IncrementSidecarQuarantine}
    [] candidate = SidecarFinalDropRecord ->
      {IncrementSidecarFinalDrop}
    [] candidate = SidecarRecoveryTriggerRecord ->
      {IncrementSidecarRecoveryTrigger}
    [] candidate = NoProposalStormRecord ->
      {IncrementNoProposalStorm}
    [] candidate = RepeatedSidecarQuarantineAccumulates ->
      {IncrementSidecarQuarantine, SameCounterAccumulates,
       SnapshotSidecarMatches}
    [] candidate = RepeatedNoProposalStormAccumulates ->
      {IncrementNoProposalStorm, SameCounterAccumulates,
       SnapshotNoProposalMatches}
    [] candidate = ObserveStormSetsFields ->
      {SetStormLastHeight, SetStormLastCount, SetStormMaxFromObservation,
       SnapshotNoProposalMatches}
    [] candidate = ObserveStormFloorsMaxToCount ->
      {SetStormLastHeight, SetStormLastCount, SetStormMaxFromObservation,
       StormMaxFloorsToCount, SnapshotNoProposalMatches}
    [] candidate = ObserveStormPreservesHigherMax ->
      {SetStormLastHeight, SetStormLastCount, StormMaxPreservesHigher,
       SnapshotNoProposalMatches}
    [] candidate = ObserveStormRaisesMax ->
      {SetStormLastHeight, SetStormLastCount, SetStormMaxFromObservation,
       StormMaxRaisesOnHigher, SnapshotNoProposalMatches}
    [] candidate = ObserveStormOverwritesLast ->
      {SetStormLastHeight, SetStormLastCount, StormLastOverwrites,
       SnapshotNoProposalMatches}
    [] candidate = SnapshotProjectsSidecar ->
      {SnapshotSidecarMatches}
    [] candidate = SnapshotProjectsNoProposal ->
      {SnapshotNoProposalMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\
          Bug = "reset_empty_keeps_sidecar" ->
      spec \ {ResetSidecarCounters}
    [] candidate = ResetEmpty /\
          Bug = "reset_empty_keeps_no_proposal" ->
      spec \ {ResetNoProposalCounters}
    [] candidate = ResetEmpty /\
          Bug = "reset_empty_keeps_storm_diagnostics" ->
      spec \ {ResetNoProposalDiagnostics}
    [] candidate = SidecarQuarantineRecord /\
          Bug = "sidecar_quarantine_not_counted" ->
      spec \ {IncrementSidecarQuarantine}
    [] candidate = SidecarFinalDropRecord /\
          Bug = "sidecar_final_drop_not_counted" ->
      spec \ {IncrementSidecarFinalDrop}
    [] candidate = SidecarRecoveryTriggerRecord /\
          Bug = "sidecar_recovery_trigger_not_counted" ->
      spec \ {IncrementSidecarRecoveryTrigger}
    [] candidate = NoProposalStormRecord /\
          Bug = "no_proposal_storm_not_counted" ->
      spec \ {IncrementNoProposalStorm}
    [] candidate = SidecarQuarantineRecord /\
          Bug = "sidecar_quarantine_counts_final_drop" ->
      (spec \ {IncrementSidecarQuarantine}) \cup
        {IncrementSidecarFinalDrop}
    [] candidate = NoProposalStormRecord /\
          Bug = "no_proposal_storm_counts_sidecar_recovery" ->
      (spec \ {IncrementNoProposalStorm}) \cup
        {IncrementSidecarRecoveryTrigger}
    [] candidate = RepeatedSidecarQuarantineAccumulates /\
          Bug = "repeated_sidecar_quarantine_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotSidecarMatches}
    [] candidate = RepeatedNoProposalStormAccumulates /\
          Bug = "repeated_no_proposal_storm_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotNoProposalMatches}
    [] candidate = ObserveStormSetsFields /\
          Bug = "storm_state_height_not_set" ->
      spec \ {SetStormLastHeight}
    [] candidate = ObserveStormSetsFields /\
          Bug = "storm_state_count_not_set" ->
      spec \ {SetStormLastCount}
    [] candidate = ObserveStormSetsFields /\
          Bug = "storm_state_max_not_set" ->
      spec \ {SetStormMaxFromObservation}
    [] candidate = ObserveStormFloorsMaxToCount /\
          Bug = "storm_state_max_not_floored_to_count" ->
      spec \ {SetStormMaxFromObservation, StormMaxFloorsToCount,
              SnapshotNoProposalMatches}
    [] candidate = ObserveStormPreservesHigherMax /\
          Bug = "storm_state_max_overwrites_higher" ->
      (spec \ {StormMaxPreservesHigher}) \cup
        {SetStormMaxFromObservation}
    [] candidate = ObserveStormRaisesMax /\
          Bug = "storm_state_max_not_raised" ->
      spec \ {SetStormMaxFromObservation, StormMaxRaisesOnHigher,
              SnapshotNoProposalMatches}
    [] candidate = ObserveStormOverwritesLast /\
          Bug = "storm_state_last_not_overwritten" ->
      spec \ {SetStormLastHeight, SetStormLastCount, StormLastOverwrites,
              SnapshotNoProposalMatches}
    [] candidate = SnapshotProjectsSidecar /\
          Bug = "snapshot_sidecar_mismatch" ->
      spec \ {SnapshotSidecarMatches}
    [] candidate = SnapshotProjectsNoProposal /\
          Bug = "snapshot_no_proposal_mismatch" ->
      spec \ {SnapshotNoProposalMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_sidecar" ->
      spec \ {ResetSidecarCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_no_proposal" ->
      spec \ {ResetNoProposalCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_storm_diagnostics" ->
      spec \ {ResetNoProposalDiagnostics}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 15
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..15

Safety ==
  /\ ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)
  /\ ImplementationActions(SidecarQuarantineRecord) =
       SpecActions(SidecarQuarantineRecord)
  /\ ImplementationActions(SidecarFinalDropRecord) =
       SpecActions(SidecarFinalDropRecord)
  /\ ImplementationActions(SidecarRecoveryTriggerRecord) =
       SpecActions(SidecarRecoveryTriggerRecord)
  /\ ImplementationActions(NoProposalStormRecord) =
       SpecActions(NoProposalStormRecord)
  /\ ImplementationActions(RepeatedSidecarQuarantineAccumulates) =
       SpecActions(RepeatedSidecarQuarantineAccumulates)
  /\ ImplementationActions(RepeatedNoProposalStormAccumulates) =
       SpecActions(RepeatedNoProposalStormAccumulates)
  /\ ImplementationActions(ObserveStormSetsFields) =
       SpecActions(ObserveStormSetsFields)
  /\ ImplementationActions(ObserveStormFloorsMaxToCount) =
       SpecActions(ObserveStormFloorsMaxToCount)
  /\ ImplementationActions(ObserveStormPreservesHigherMax) =
       SpecActions(ObserveStormPreservesHigherMax)
  /\ ImplementationActions(ObserveStormRaisesMax) =
       SpecActions(ObserveStormRaisesMax)
  /\ ImplementationActions(ObserveStormOverwritesLast) =
       SpecActions(ObserveStormOverwritesLast)
  /\ ImplementationActions(SnapshotProjectsSidecar) =
       SpecActions(SnapshotProjectsSidecar)
  /\ ImplementationActions(SnapshotProjectsNoProposal) =
       SpecActions(SnapshotProjectsNoProposal)
  /\ ImplementationActions(ResetAfterRecordsClears) =
       SpecActions(ResetAfterRecordsClears)

SidecarNoProposalStatusExactness ==
  Safety

SidecarNoProposalStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ SidecarNoProposalStatusExactness

BugResetEmptyKeepsSidecar ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsNoProposal ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsStormDiagnostics ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugSidecarQuarantineNotCounted ==
  ImplementationActions(SidecarQuarantineRecord) =
    SpecActions(SidecarQuarantineRecord)

BugSidecarFinalDropNotCounted ==
  ImplementationActions(SidecarFinalDropRecord) =
    SpecActions(SidecarFinalDropRecord)

BugSidecarRecoveryTriggerNotCounted ==
  ImplementationActions(SidecarRecoveryTriggerRecord) =
    SpecActions(SidecarRecoveryTriggerRecord)

BugNoProposalStormNotCounted ==
  ImplementationActions(NoProposalStormRecord) =
    SpecActions(NoProposalStormRecord)

BugSidecarQuarantineCountsFinalDrop ==
  ImplementationActions(SidecarQuarantineRecord) =
    SpecActions(SidecarQuarantineRecord)

BugNoProposalStormCountsSidecarRecovery ==
  ImplementationActions(NoProposalStormRecord) =
    SpecActions(NoProposalStormRecord)

BugRepeatedSidecarQuarantineOverwritesCount ==
  ImplementationActions(RepeatedSidecarQuarantineAccumulates) =
    SpecActions(RepeatedSidecarQuarantineAccumulates)

BugRepeatedNoProposalStormOverwritesCount ==
  ImplementationActions(RepeatedNoProposalStormAccumulates) =
    SpecActions(RepeatedNoProposalStormAccumulates)

BugStormStateHeightNotSet ==
  ImplementationActions(ObserveStormSetsFields) =
    SpecActions(ObserveStormSetsFields)

BugStormStateCountNotSet ==
  ImplementationActions(ObserveStormSetsFields) =
    SpecActions(ObserveStormSetsFields)

BugStormStateMaxNotSet ==
  ImplementationActions(ObserveStormSetsFields) =
    SpecActions(ObserveStormSetsFields)

BugStormStateMaxNotFlooredToCount ==
  ImplementationActions(ObserveStormFloorsMaxToCount) =
    SpecActions(ObserveStormFloorsMaxToCount)

BugStormStateMaxOverwritesHigher ==
  ImplementationActions(ObserveStormPreservesHigherMax) =
    SpecActions(ObserveStormPreservesHigherMax)

BugStormStateMaxNotRaised ==
  ImplementationActions(ObserveStormRaisesMax) =
    SpecActions(ObserveStormRaisesMax)

BugStormStateLastNotOverwritten ==
  ImplementationActions(ObserveStormOverwritesLast) =
    SpecActions(ObserveStormOverwritesLast)

BugSnapshotSidecarMismatch ==
  ImplementationActions(SnapshotProjectsSidecar) =
    SpecActions(SnapshotProjectsSidecar)

BugSnapshotNoProposalMismatch ==
  ImplementationActions(SnapshotProjectsNoProposal) =
    SpecActions(SnapshotProjectsNoProposal)

BugResetAfterRecordsKeepsSidecar ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsNoProposal ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsStormDiagnostics ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
