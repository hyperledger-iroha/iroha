---- MODULE SumeragiNexusEconomicsStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi Nexus economics status accounting.

This slice captures `record_nexus_fee_event(...)`,
`record_public_lane_bonded_delta(...)`,
`record_public_lane_pending_unbond_delta(...)`,
`record_public_lane_slash(...)`, `nexus_fee_snapshot()`,
`nexus_staking_snapshot()`, `reset_nexus_economics_for_tests()`,
`snapshot().nexus_fee`, `snapshot().nexus_staking`, and
`StatusSnapshot::strip_lane_details()` clearing Nexus economics details when
Nexus lane status is disabled.
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
ChargedPayer == 2
ChargedSponsor == 3
ChargedClearsLastError == 4
SponsorDisabled == 5
SponsorUnauthorized == 6
SponsorCapExceeded == 7
TransferFailed == 8
ConfigInvalid == 9
FeeSnapshotMatches == 10
StatusSnapshotProjectsFee == 11
BondedIncrease == 12
BondedDecrease == 13
BondedZeroNoop == 14
BondedUnderflowClamp == 15
BondedOverflowClamp == 16
PendingUnbondIncrease == 17
PendingUnbondDecrease == 18
SlashRecords == 19
RepeatedSlashAccumulates == 20
LaneEntryPreservesId == 21
StakingSnapshotSorted == 22
StakingSnapshotMatches == 23
StatusSnapshotProjectsStaking == 24
StripClearsNexusEconomics == 25
ResetAfterRecordsClearsFee == 26
ResetAfterRecordsClearsStaking == 27

Candidates == 1..27

ResetFeeCounters == 1
ResetFeeLastFields == 2
ResetStakingLanes == 3
ChargedTotalIncrement == 4
ChargedViaPayerIncrement == 5
ChargedViaSponsorIncrement == 6
LastPayerStored == 7
LastPayerIdStored == 8
LastAmountStored == 9
LastAssetStored == 10
LastErrorCleared == 11
PriorErrorReplaced == 12
SponsorDisabledIncrement == 13
SponsorDisabledPayerSponsor == 14
SponsorDisabledErrorStored == 15
SponsorUnauthorizedIncrement == 16
SponsorUnauthorizedSponsorIdStored == 17
SponsorUnauthorizedAuthorityInError == 18
SponsorCapIncrement == 19
SponsorCapAttemptedFeeStored == 20
SponsorCapMaxFeeInError == 21
TransferFailureIncrement == 22
TransferFailurePayerKindStored == 23
TransferFailureAssetStored == 24
TransferFailureReasonStored == 25
ConfigErrorIncrement == 26
ConfigErrorStored == 27
ConfigInvalidPreservesLastPayer == 28
FeeSnapshotReadsCounters == 29
FeeSnapshotReadsLastFields == 30
StatusSnapshotFeeField == 31
BondedAddDelta == 32
BondedSubDelta == 33
NumericZeroDeltaNoop == 34
NumericUnderflowClampsZero == 35
NumericOverflowClampsZero == 36
PendingUnbondAddDelta == 37
PendingUnbondSubDelta == 38
SlashIncrement == 39
SlashAccumulates == 40
LaneEntryCreated == 41
LaneIdStored == 42
StakingSnapshotReadsLane == 43
StakingSnapshotSortsByLane == 44
StatusSnapshotStakingField == 45
StripClearsNexusFee == 46
StripClearsNexusStaking == 47

ResetFeeActions == {ResetFeeCounters, ResetFeeLastFields}
ResetStakingActions == {ResetStakingLanes}
ChargedLastActions ==
  {LastPayerStored, LastPayerIdStored, LastAmountStored, LastAssetStored}
FeeSnapshotActions == {FeeSnapshotReadsCounters, FeeSnapshotReadsLastFields}
LaneIdentityActions == {LaneEntryCreated, LaneIdStored}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      ResetFeeActions \cup ResetStakingActions
    [] candidate = ChargedPayer ->
      {ChargedTotalIncrement, ChargedViaPayerIncrement, LastErrorCleared} \cup
        ChargedLastActions
    [] candidate = ChargedSponsor ->
      {ChargedTotalIncrement, ChargedViaSponsorIncrement, LastErrorCleared} \cup
        ChargedLastActions
    [] candidate = ChargedClearsLastError ->
      {PriorErrorReplaced, LastErrorCleared}
    [] candidate = SponsorDisabled ->
      {SponsorDisabledIncrement, SponsorDisabledPayerSponsor,
       SponsorDisabledErrorStored, LastPayerIdStored}
    [] candidate = SponsorUnauthorized ->
      {SponsorUnauthorizedIncrement, SponsorUnauthorizedSponsorIdStored,
       SponsorUnauthorizedAuthorityInError, LastPayerStored, LastPayerIdStored}
    [] candidate = SponsorCapExceeded ->
      {SponsorCapIncrement, SponsorCapAttemptedFeeStored,
       SponsorCapMaxFeeInError, LastPayerStored, LastPayerIdStored}
    [] candidate = TransferFailed ->
      {TransferFailureIncrement, TransferFailurePayerKindStored,
       TransferFailureAssetStored, TransferFailureReasonStored,
       LastPayerIdStored, LastAmountStored}
    [] candidate = ConfigInvalid ->
      {ConfigErrorIncrement, ConfigErrorStored, ConfigInvalidPreservesLastPayer}
    [] candidate = FeeSnapshotMatches ->
      FeeSnapshotActions
    [] candidate = StatusSnapshotProjectsFee ->
      {StatusSnapshotFeeField} \cup FeeSnapshotActions
    [] candidate = BondedIncrease ->
      LaneIdentityActions \cup {BondedAddDelta}
    [] candidate = BondedDecrease ->
      LaneIdentityActions \cup {BondedSubDelta}
    [] candidate = BondedZeroNoop ->
      LaneIdentityActions \cup {NumericZeroDeltaNoop}
    [] candidate = BondedUnderflowClamp ->
      LaneIdentityActions \cup {NumericUnderflowClampsZero}
    [] candidate = BondedOverflowClamp ->
      LaneIdentityActions \cup {NumericOverflowClampsZero}
    [] candidate = PendingUnbondIncrease ->
      LaneIdentityActions \cup {PendingUnbondAddDelta}
    [] candidate = PendingUnbondDecrease ->
      LaneIdentityActions \cup {PendingUnbondSubDelta}
    [] candidate = SlashRecords ->
      LaneIdentityActions \cup {SlashIncrement}
    [] candidate = RepeatedSlashAccumulates ->
      LaneIdentityActions \cup {SlashIncrement, SlashAccumulates}
    [] candidate = LaneEntryPreservesId ->
      LaneIdentityActions
    [] candidate = StakingSnapshotSorted ->
      {StakingSnapshotReadsLane, StakingSnapshotSortsByLane}
    [] candidate = StakingSnapshotMatches ->
      LaneIdentityActions \cup
        {StakingSnapshotReadsLane, BondedAddDelta, PendingUnbondAddDelta,
         SlashIncrement}
    [] candidate = StatusSnapshotProjectsStaking ->
      {StatusSnapshotStakingField, StakingSnapshotReadsLane}
    [] candidate = StripClearsNexusEconomics ->
      {StripClearsNexusFee, StripClearsNexusStaking}
    [] candidate = ResetAfterRecordsClearsFee ->
      ResetFeeActions
    [] candidate = ResetAfterRecordsClearsStaking ->
      ResetStakingActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_keeps_fee" ->
      spec \ ResetFeeActions
    [] candidate = ResetEmpty /\ Bug = "reset_keeps_staking" ->
      spec \ ResetStakingActions
    [] candidate = ChargedPayer /\ Bug = "charged_not_counted" ->
      spec \ {ChargedTotalIncrement}
    [] candidate = ChargedPayer /\ Bug = "payer_charge_wrong_bucket" ->
      (spec \ {ChargedViaPayerIncrement}) \cup {ChargedViaSponsorIncrement}
    [] candidate = ChargedSponsor /\ Bug = "sponsor_charge_wrong_bucket" ->
      (spec \ {ChargedViaSponsorIncrement}) \cup {ChargedViaPayerIncrement}
    [] candidate = ChargedClearsLastError /\ Bug = "charged_keeps_last_error" ->
      spec \ {LastErrorCleared}
    [] candidate = SponsorDisabled /\ Bug = "sponsor_disabled_not_counted" ->
      spec \ {SponsorDisabledIncrement}
    [] candidate = SponsorDisabled /\ Bug = "sponsor_disabled_wrong_last_payer" ->
      spec \ {SponsorDisabledPayerSponsor}
    [] candidate = SponsorUnauthorized /\
          Bug = "sponsor_unauthorized_not_counted" ->
      spec \ {SponsorUnauthorizedIncrement}
    [] candidate = SponsorUnauthorized /\
          Bug = "sponsor_unauthorized_drops_authority" ->
      spec \ {SponsorUnauthorizedAuthorityInError}
    [] candidate = SponsorCapExceeded /\ Bug = "sponsor_cap_not_counted" ->
      spec \ {SponsorCapIncrement}
    [] candidate = SponsorCapExceeded /\
          Bug = "sponsor_cap_amount_not_attempted" ->
      spec \ {SponsorCapAttemptedFeeStored}
    [] candidate = TransferFailed /\ Bug = "transfer_failure_not_counted" ->
      spec \ {TransferFailureIncrement}
    [] candidate = TransferFailed /\ Bug = "transfer_failure_drops_asset" ->
      spec \ {TransferFailureAssetStored}
    [] candidate = ConfigInvalid /\ Bug = "config_invalid_not_counted" ->
      spec \ {ConfigErrorIncrement}
    [] candidate = ConfigInvalid /\ Bug = "config_invalid_overwrites_payer" ->
      spec \ {ConfigInvalidPreservesLastPayer}
    [] candidate = FeeSnapshotMatches /\ Bug = "fee_snapshot_mismatch" ->
      spec \ {FeeSnapshotReadsCounters}
    [] candidate = StatusSnapshotProjectsFee /\
          Bug = "status_snapshot_drops_fee" ->
      spec \ {StatusSnapshotFeeField}
    [] candidate = BondedIncrease /\ Bug = "bonded_delta_not_applied" ->
      spec \ {BondedAddDelta}
    [] candidate = PendingUnbondIncrease /\
          Bug = "pending_unbond_delta_not_applied" ->
      spec \ {PendingUnbondAddDelta}
    [] candidate = BondedZeroNoop /\ Bug = "zero_delta_mutates" ->
      (spec \ {NumericZeroDeltaNoop}) \cup {BondedAddDelta}
    [] candidate = BondedUnderflowClamp /\
          Bug = "decrease_underflow_not_clamped" ->
      (spec \ {NumericUnderflowClampsZero}) \cup {BondedSubDelta}
    [] candidate = BondedOverflowClamp /\ Bug = "overflow_not_clamped" ->
      (spec \ {NumericOverflowClampsZero}) \cup {BondedAddDelta}
    [] candidate = SlashRecords /\ Bug = "slash_not_counted" ->
      spec \ {SlashIncrement}
    [] candidate = RepeatedSlashAccumulates /\
          Bug = "repeated_slash_overwrites" ->
      spec \ {SlashAccumulates}
    [] candidate = LaneEntryPreservesId /\ Bug = "lane_entry_missing" ->
      spec \ {LaneEntryCreated}
    [] candidate = StakingSnapshotSorted /\ Bug = "lane_sorting_missing" ->
      spec \ {StakingSnapshotSortsByLane}
    [] candidate = StakingSnapshotMatches /\ Bug = "staking_snapshot_mismatch" ->
      spec \ {StakingSnapshotReadsLane}
    [] candidate = StatusSnapshotProjectsStaking /\
          Bug = "status_snapshot_drops_staking" ->
      spec \ {StatusSnapshotStakingField}
    [] candidate = StripClearsNexusEconomics /\
          Bug = "strip_keeps_nexus_fee" ->
      spec \ {StripClearsNexusFee}
    [] candidate = StripClearsNexusEconomics /\
          Bug = "strip_keeps_nexus_staking" ->
      spec \ {StripClearsNexusStaking}
    [] candidate = ResetAfterRecordsClearsFee /\
          Bug = "reset_after_records_keeps_fee" ->
      spec \ ResetFeeActions
    [] candidate = ResetAfterRecordsClearsStaking /\
          Bug = "reset_after_records_keeps_staking" ->
      spec \ ResetStakingActions
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 27
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..27

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

NexusEconomicsStatusExactness ==
  Safety

NexusEconomicsStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NexusEconomicsStatusExactness

BugResetKeepsFee ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetKeepsStaking ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugChargedNotCounted ==
  ImplementationActions(ChargedPayer) = SpecActions(ChargedPayer)

BugPayerChargeWrongBucket ==
  ImplementationActions(ChargedPayer) = SpecActions(ChargedPayer)

BugSponsorChargeWrongBucket ==
  ImplementationActions(ChargedSponsor) = SpecActions(ChargedSponsor)

BugChargedKeepsLastError ==
  ImplementationActions(ChargedClearsLastError) =
    SpecActions(ChargedClearsLastError)

BugSponsorDisabledNotCounted ==
  ImplementationActions(SponsorDisabled) = SpecActions(SponsorDisabled)

BugSponsorDisabledWrongLastPayer ==
  ImplementationActions(SponsorDisabled) = SpecActions(SponsorDisabled)

BugSponsorUnauthorizedNotCounted ==
  ImplementationActions(SponsorUnauthorized) = SpecActions(SponsorUnauthorized)

BugSponsorUnauthorizedDropsAuthority ==
  ImplementationActions(SponsorUnauthorized) = SpecActions(SponsorUnauthorized)

BugSponsorCapNotCounted ==
  ImplementationActions(SponsorCapExceeded) = SpecActions(SponsorCapExceeded)

BugSponsorCapAmountNotAttempted ==
  ImplementationActions(SponsorCapExceeded) = SpecActions(SponsorCapExceeded)

BugTransferFailureNotCounted ==
  ImplementationActions(TransferFailed) = SpecActions(TransferFailed)

BugTransferFailureDropsAsset ==
  ImplementationActions(TransferFailed) = SpecActions(TransferFailed)

BugConfigInvalidNotCounted ==
  ImplementationActions(ConfigInvalid) = SpecActions(ConfigInvalid)

BugConfigInvalidOverwritesPayer ==
  ImplementationActions(ConfigInvalid) = SpecActions(ConfigInvalid)

BugFeeSnapshotMismatch ==
  ImplementationActions(FeeSnapshotMatches) = SpecActions(FeeSnapshotMatches)

BugStatusSnapshotDropsFee ==
  ImplementationActions(StatusSnapshotProjectsFee) =
    SpecActions(StatusSnapshotProjectsFee)

BugBondedDeltaNotApplied ==
  ImplementationActions(BondedIncrease) = SpecActions(BondedIncrease)

BugPendingUnbondDeltaNotApplied ==
  ImplementationActions(PendingUnbondIncrease) =
    SpecActions(PendingUnbondIncrease)

BugZeroDeltaMutates ==
  ImplementationActions(BondedZeroNoop) = SpecActions(BondedZeroNoop)

BugDecreaseUnderflowNotClamped ==
  ImplementationActions(BondedUnderflowClamp) =
    SpecActions(BondedUnderflowClamp)

BugOverflowNotClamped ==
  ImplementationActions(BondedOverflowClamp) = SpecActions(BondedOverflowClamp)

BugSlashNotCounted ==
  ImplementationActions(SlashRecords) = SpecActions(SlashRecords)

BugRepeatedSlashOverwrites ==
  ImplementationActions(RepeatedSlashAccumulates) =
    SpecActions(RepeatedSlashAccumulates)

BugLaneEntryMissing ==
  ImplementationActions(LaneEntryPreservesId) =
    SpecActions(LaneEntryPreservesId)

BugLaneSortingMissing ==
  ImplementationActions(StakingSnapshotSorted) =
    SpecActions(StakingSnapshotSorted)

BugStakingSnapshotMismatch ==
  ImplementationActions(StakingSnapshotMatches) =
    SpecActions(StakingSnapshotMatches)

BugStatusSnapshotDropsStaking ==
  ImplementationActions(StatusSnapshotProjectsStaking) =
    SpecActions(StatusSnapshotProjectsStaking)

BugStripKeepsNexusFee ==
  ImplementationActions(StripClearsNexusEconomics) =
    SpecActions(StripClearsNexusEconomics)

BugStripKeepsNexusStaking ==
  ImplementationActions(StripClearsNexusEconomics) =
    SpecActions(StripClearsNexusEconomics)

BugResetAfterRecordsKeepsFee ==
  ImplementationActions(ResetAfterRecordsClearsFee) =
    SpecActions(ResetAfterRecordsClearsFee)

BugResetAfterRecordsKeepsStaking ==
  ImplementationActions(ResetAfterRecordsClearsStaking) =
    SpecActions(ResetAfterRecordsClearsStaking)

=============================================================================
====
