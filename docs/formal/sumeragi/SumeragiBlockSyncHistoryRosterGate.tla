---- MODULE SumeragiBlockSyncHistoryRosterGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for block-sync historical roster selection.

This slice captures the non-cryptographic control policy in
`block_sync_history_roster_for_block(...)`. Histories, QCs, checkpoints, and
precommit signer records are collapsed into representative actions while
preserving observable behavior: consensus mode selects the matching mode tag;
exact precommit records are filtered by block hash, height, optional view,
mode tag, and expected epoch, with the greatest view selected; commit-QC and
validator-checkpoint histories are filtered to the requested block hash and
heights no greater than the requested height, with greatest height then view
selected; fresh non-empty commit-QC history suppresses precommit-derived QC
reconstruction; absent, stale, or empty-aggregate commit-QC history attempts
derivation from an exact precommit record; derived QCs use the
PrecommitSignerHistory source; failed derivation falls back directly to the
precommit signer record only when no same-height checkpoint is available; no
cert/checkpoint evidence returns no selection; cert/checkpoint sources, roster
height/view adjustment, checkpoint height filtering, stake-snapshot forwarding,
and post-validation precommit fallback follow the Rust helper.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ModeTagPermissioned == 1
ModeTagNpos == 2
PrecommitExactFilters == 3
PrecommitOptionalViewAllowsAny == 4
PrecommitChoosesMaxView == 5
CertHistoryFilters == 6
CertHistoryChoosesMaxHeightThenView == 7
CheckpointHistoryFilters == 8
CheckpointHistoryChoosesMaxHeightThenView == 9
FreshCertSuppressesDerive == 10
MissingCertDerivesFromPrecommit == 11
StaleCertDerivesFromPrecommit == 12
EmptyAggregateCertDerivesFromPrecommit == 13
DerivedCertUsesPrecommitSource == 14
DerivedFailsNoCheckpointFallsBackPrecommit == 15
DerivedFailsOldCheckpointFallsBackPrecommit == 16
DerivedFailsCurrentCheckpointUsesCheckpoint == 17
NoCertNoCheckpointReturnsNone == 18
CertSourceQcHistory == 19
OnlyCheckpointSourceCheckpointHistory == 20
CertHeightMismatchUsesCertHeightView == 21
CertHeightMismatchFiltersCheckpoint == 22
OnlyCheckpointOlderUsesCheckpointHeight == 23
SelectionFailureFallsBackPrecommit == 24
SelectionFailureNoPrecommitReturnsNone == 25
PrecommitStakeSnapshotForwarded == 26

Candidates == 1..26

ModeTagPermissionedAction == 1
ModeTagNposAction == 2
FilterPrecommitBlockHash == 3
FilterPrecommitHeight == 4
FilterPrecommitOptionalView == 5
FilterPrecommitModeTag == 6
FilterPrecommitExpectedEpoch == 7
ChoosePrecommitMaxView == 8
FilterCertSubjectHash == 9
FilterCertHeightLeRequested == 10
ChooseCertMaxHeightThenView == 11
FilterCheckpointBlockHash == 12
FilterCheckpointHeightLeRequested == 13
ChooseCheckpointMaxHeightThenView == 14
SuppressDerive == 15
AttemptDerive == 16
DeriveReasonMissingCert == 17
DeriveReasonStaleCert == 18
DeriveReasonEmptyAggregate == 19
SourceQcHistory == 20
SourcePrecommitSignerHistory == 21
SourceValidatorCheckpointHistory == 22
FallbackPrecommitSelection == 23
ProceedCheckpointArtifact == 24
ReturnNone == 25
CallRosterArtifactSelection == 26
RosterHeightRequested == 27
RosterHeightCert == 28
RosterHeightCheckpoint == 29
RosterViewCert == 30
FilterCheckpointToCertHeight == 31
ForwardPrecommitStakeSnapshot == 32
ArtifactFailureFallbackPrecommit == 33
ArtifactFailureNone == 34

Actions == 1..34

SpecActions(candidate) ==
  CASE candidate = ModeTagPermissioned ->
      {ModeTagPermissionedAction}
    [] candidate = ModeTagNpos ->
      {ModeTagNposAction}
    [] candidate = PrecommitExactFilters ->
      {FilterPrecommitBlockHash, FilterPrecommitHeight,
       FilterPrecommitOptionalView, FilterPrecommitModeTag,
       FilterPrecommitExpectedEpoch}
    [] candidate = PrecommitOptionalViewAllowsAny ->
      {FilterPrecommitOptionalView}
    [] candidate = PrecommitChoosesMaxView ->
      {ChoosePrecommitMaxView}
    [] candidate = CertHistoryFilters ->
      {FilterCertSubjectHash, FilterCertHeightLeRequested}
    [] candidate = CertHistoryChoosesMaxHeightThenView ->
      {ChooseCertMaxHeightThenView}
    [] candidate = CheckpointHistoryFilters ->
      {FilterCheckpointBlockHash, FilterCheckpointHeightLeRequested}
    [] candidate = CheckpointHistoryChoosesMaxHeightThenView ->
      {ChooseCheckpointMaxHeightThenView}
    [] candidate = FreshCertSuppressesDerive ->
      {SuppressDerive, SourceQcHistory}
    [] candidate = MissingCertDerivesFromPrecommit ->
      {AttemptDerive, DeriveReasonMissingCert}
    [] candidate = StaleCertDerivesFromPrecommit ->
      {AttemptDerive, DeriveReasonStaleCert}
    [] candidate = EmptyAggregateCertDerivesFromPrecommit ->
      {AttemptDerive, DeriveReasonEmptyAggregate}
    [] candidate = DerivedCertUsesPrecommitSource ->
      {AttemptDerive, SourcePrecommitSignerHistory}
    [] candidate = DerivedFailsNoCheckpointFallsBackPrecommit ->
      {FallbackPrecommitSelection}
    [] candidate = DerivedFailsOldCheckpointFallsBackPrecommit ->
      {FallbackPrecommitSelection}
    [] candidate = DerivedFailsCurrentCheckpointUsesCheckpoint ->
      {ProceedCheckpointArtifact, SourceValidatorCheckpointHistory}
    [] candidate = NoCertNoCheckpointReturnsNone ->
      {ReturnNone}
    [] candidate = CertSourceQcHistory ->
      {SourceQcHistory, CallRosterArtifactSelection}
    [] candidate = OnlyCheckpointSourceCheckpointHistory ->
      {SourceValidatorCheckpointHistory, CallRosterArtifactSelection}
    [] candidate = CertHeightMismatchUsesCertHeightView ->
      {RosterHeightCert, RosterViewCert}
    [] candidate = CertHeightMismatchFiltersCheckpoint ->
      {FilterCheckpointToCertHeight}
    [] candidate = OnlyCheckpointOlderUsesCheckpointHeight ->
      {RosterHeightCheckpoint}
    [] candidate = SelectionFailureFallsBackPrecommit ->
      {ArtifactFailureFallbackPrecommit, FallbackPrecommitSelection}
    [] candidate = SelectionFailureNoPrecommitReturnsNone ->
      {ArtifactFailureNone, ReturnNone}
    [] candidate = PrecommitStakeSnapshotForwarded ->
      {ForwardPrecommitStakeSnapshot}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ModeTagPermissioned /\ Bug = "permissioned_uses_npos_tag" ->
      (spec \ {ModeTagPermissionedAction}) \cup {ModeTagNposAction}
    [] candidate = ModeTagNpos /\ Bug = "npos_uses_permissioned_tag" ->
      (spec \ {ModeTagNposAction}) \cup {ModeTagPermissionedAction}
    [] candidate = PrecommitExactFilters /\
          Bug = "precommit_filter_ignores_hash" ->
      spec \ {FilterPrecommitBlockHash}
    [] candidate = PrecommitExactFilters /\
          Bug = "precommit_filter_ignores_epoch" ->
      spec \ {FilterPrecommitExpectedEpoch}
    [] candidate = PrecommitOptionalViewAllowsAny /\
          Bug = "precommit_requires_view_when_absent" ->
      spec \ {FilterPrecommitOptionalView}
    [] candidate = PrecommitChoosesMaxView /\
          Bug = "precommit_chooses_lowest_view" ->
      spec \ {ChoosePrecommitMaxView}
    [] candidate = CertHistoryFilters /\ Bug = "cert_filter_allows_future" ->
      spec \ {FilterCertHeightLeRequested}
    [] candidate = CertHistoryChoosesMaxHeightThenView /\
          Bug = "cert_chooses_lowest_height" ->
      spec \ {ChooseCertMaxHeightThenView}
    [] candidate = CheckpointHistoryFilters /\
          Bug = "checkpoint_filter_allows_wrong_hash" ->
      spec \ {FilterCheckpointBlockHash}
    [] candidate = CheckpointHistoryChoosesMaxHeightThenView /\
          Bug = "checkpoint_chooses_lowest_view" ->
      spec \ {ChooseCheckpointMaxHeightThenView}
    [] candidate = FreshCertSuppressesDerive /\
          Bug = "fresh_cert_derives_anyway" ->
      (spec \ {SuppressDerive}) \cup {AttemptDerive}
    [] candidate = MissingCertDerivesFromPrecommit /\
          Bug = "missing_cert_skips_derive" ->
      spec \ {AttemptDerive}
    [] candidate = StaleCertDerivesFromPrecommit /\
          Bug = "stale_cert_skips_derive" ->
      spec \ {AttemptDerive}
    [] candidate = EmptyAggregateCertDerivesFromPrecommit /\
          Bug = "empty_aggregate_skips_derive" ->
      spec \ {AttemptDerive}
    [] candidate = DerivedCertUsesPrecommitSource /\
          Bug = "derived_cert_keeps_qc_history_source" ->
      (spec \ {SourcePrecommitSignerHistory}) \cup {SourceQcHistory}
    [] candidate = DerivedFailsNoCheckpointFallsBackPrecommit /\
          Bug = "derive_fail_no_checkpoint_returns_none" ->
      (spec \ {FallbackPrecommitSelection}) \cup {ReturnNone}
    [] candidate = DerivedFailsOldCheckpointFallsBackPrecommit /\
          Bug = "derive_fail_old_checkpoint_uses_checkpoint" ->
      (spec \ {FallbackPrecommitSelection}) \cup {ProceedCheckpointArtifact}
    [] candidate = DerivedFailsCurrentCheckpointUsesCheckpoint /\
          Bug = "derive_fail_current_checkpoint_falls_back" ->
      (spec \ {ProceedCheckpointArtifact}) \cup {FallbackPrecommitSelection}
    [] candidate = NoCertNoCheckpointReturnsNone /\
          Bug = "no_cert_no_checkpoint_calls_selection" ->
      (spec \ {ReturnNone}) \cup {CallRosterArtifactSelection}
    [] candidate = CertSourceQcHistory /\ Bug = "cert_source_checkpoint" ->
      (spec \ {SourceQcHistory}) \cup {SourceValidatorCheckpointHistory}
    [] candidate = OnlyCheckpointSourceCheckpointHistory /\
          Bug = "checkpoint_source_qc_history" ->
      (spec \ {SourceValidatorCheckpointHistory}) \cup {SourceQcHistory}
    [] candidate = CertHeightMismatchUsesCertHeightView /\
          Bug = "cert_mismatch_keeps_requested_height" ->
      (spec \ {RosterHeightCert, RosterViewCert}) \cup {RosterHeightRequested}
    [] candidate = CertHeightMismatchFiltersCheckpoint /\
          Bug = "cert_mismatch_keeps_any_checkpoint" ->
      spec \ {FilterCheckpointToCertHeight}
    [] candidate = OnlyCheckpointOlderUsesCheckpointHeight /\
          Bug = "old_checkpoint_keeps_requested_height" ->
      (spec \ {RosterHeightCheckpoint}) \cup {RosterHeightRequested}
    [] candidate = SelectionFailureFallsBackPrecommit /\
          Bug = "selection_failure_no_precommit_fallback" ->
      spec \ {FallbackPrecommitSelection}
    [] candidate = SelectionFailureNoPrecommitReturnsNone /\
          Bug = "selection_failure_without_precommit_returns_some" ->
      (spec \ {ReturnNone}) \cup {CallRosterArtifactSelection}
    [] candidate = PrecommitStakeSnapshotForwarded /\
          Bug = "precommit_stake_snapshot_not_forwarded" ->
      spec \ {ForwardPrecommitStakeSnapshot}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "permissioned_uses_npos_tag",
       "npos_uses_permissioned_tag",
       "precommit_filter_ignores_hash",
       "precommit_filter_ignores_epoch",
       "precommit_requires_view_when_absent",
       "precommit_chooses_lowest_view",
       "cert_filter_allows_future",
       "cert_chooses_lowest_height",
       "checkpoint_filter_allows_wrong_hash",
       "checkpoint_chooses_lowest_view",
       "fresh_cert_derives_anyway",
       "missing_cert_skips_derive",
       "stale_cert_skips_derive",
       "empty_aggregate_skips_derive",
       "derived_cert_keeps_qc_history_source",
       "derive_fail_no_checkpoint_returns_none",
       "derive_fail_old_checkpoint_uses_checkpoint",
       "derive_fail_current_checkpoint_falls_back",
       "no_cert_no_checkpoint_calls_selection",
       "cert_source_checkpoint",
       "checkpoint_source_qc_history",
       "cert_mismatch_keeps_requested_height",
       "cert_mismatch_keeps_any_checkpoint",
       "old_checkpoint_keeps_requested_height",
       "selection_failure_no_precommit_fallback",
       "selection_failure_without_precommit_returns_some",
       "precommit_stake_snapshot_not_forwarded"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BugPermissionedUsesNposTag ==
  ImplementationActions(ModeTagPermissioned) = SpecActions(ModeTagPermissioned)

BugNposUsesPermissionedTag ==
  ImplementationActions(ModeTagNpos) = SpecActions(ModeTagNpos)

BugPrecommitFilterIgnoresHash ==
  ImplementationActions(PrecommitExactFilters) =
    SpecActions(PrecommitExactFilters)

BugPrecommitFilterIgnoresEpoch ==
  ImplementationActions(PrecommitExactFilters) =
    SpecActions(PrecommitExactFilters)

BugPrecommitRequiresViewWhenAbsent ==
  ImplementationActions(PrecommitOptionalViewAllowsAny) =
    SpecActions(PrecommitOptionalViewAllowsAny)

BugPrecommitChoosesLowestView ==
  ImplementationActions(PrecommitChoosesMaxView) =
    SpecActions(PrecommitChoosesMaxView)

BugCertFilterAllowsFuture ==
  ImplementationActions(CertHistoryFilters) = SpecActions(CertHistoryFilters)

BugCertChoosesLowestHeight ==
  ImplementationActions(CertHistoryChoosesMaxHeightThenView) =
    SpecActions(CertHistoryChoosesMaxHeightThenView)

BugCheckpointFilterAllowsWrongHash ==
  ImplementationActions(CheckpointHistoryFilters) =
    SpecActions(CheckpointHistoryFilters)

BugCheckpointChoosesLowestView ==
  ImplementationActions(CheckpointHistoryChoosesMaxHeightThenView) =
    SpecActions(CheckpointHistoryChoosesMaxHeightThenView)

BugFreshCertDerivesAnyway ==
  ImplementationActions(FreshCertSuppressesDerive) =
    SpecActions(FreshCertSuppressesDerive)

BugMissingCertSkipsDerive ==
  ImplementationActions(MissingCertDerivesFromPrecommit) =
    SpecActions(MissingCertDerivesFromPrecommit)

BugStaleCertSkipsDerive ==
  ImplementationActions(StaleCertDerivesFromPrecommit) =
    SpecActions(StaleCertDerivesFromPrecommit)

BugEmptyAggregateSkipsDerive ==
  ImplementationActions(EmptyAggregateCertDerivesFromPrecommit) =
    SpecActions(EmptyAggregateCertDerivesFromPrecommit)

BugDerivedCertKeepsQcHistorySource ==
  ImplementationActions(DerivedCertUsesPrecommitSource) =
    SpecActions(DerivedCertUsesPrecommitSource)

BugDeriveFailNoCheckpointReturnsNone ==
  ImplementationActions(DerivedFailsNoCheckpointFallsBackPrecommit) =
    SpecActions(DerivedFailsNoCheckpointFallsBackPrecommit)

BugDeriveFailOldCheckpointUsesCheckpoint ==
  ImplementationActions(DerivedFailsOldCheckpointFallsBackPrecommit) =
    SpecActions(DerivedFailsOldCheckpointFallsBackPrecommit)

BugDeriveFailCurrentCheckpointFallsBack ==
  ImplementationActions(DerivedFailsCurrentCheckpointUsesCheckpoint) =
    SpecActions(DerivedFailsCurrentCheckpointUsesCheckpoint)

BugNoCertNoCheckpointCallsSelection ==
  ImplementationActions(NoCertNoCheckpointReturnsNone) =
    SpecActions(NoCertNoCheckpointReturnsNone)

BugCertSourceCheckpoint ==
  ImplementationActions(CertSourceQcHistory) = SpecActions(CertSourceQcHistory)

BugCheckpointSourceQcHistory ==
  ImplementationActions(OnlyCheckpointSourceCheckpointHistory) =
    SpecActions(OnlyCheckpointSourceCheckpointHistory)

BugCertMismatchKeepsRequestedHeight ==
  ImplementationActions(CertHeightMismatchUsesCertHeightView) =
    SpecActions(CertHeightMismatchUsesCertHeightView)

BugCertMismatchKeepsAnyCheckpoint ==
  ImplementationActions(CertHeightMismatchFiltersCheckpoint) =
    SpecActions(CertHeightMismatchFiltersCheckpoint)

BugOldCheckpointKeepsRequestedHeight ==
  ImplementationActions(OnlyCheckpointOlderUsesCheckpointHeight) =
    SpecActions(OnlyCheckpointOlderUsesCheckpointHeight)

BugSelectionFailureNoPrecommitFallback ==
  ImplementationActions(SelectionFailureFallsBackPrecommit) =
    SpecActions(SelectionFailureFallsBackPrecommit)

BugSelectionFailureWithoutPrecommitReturnsSome ==
  ImplementationActions(SelectionFailureNoPrecommitReturnsNone) =
    SpecActions(SelectionFailureNoPrecommitReturnsNone)

BugPrecommitStakeSnapshotNotForwarded ==
  ImplementationActions(PrecommitStakeSnapshotForwarded) =
    SpecActions(PrecommitStakeSnapshotForwarded)

====
