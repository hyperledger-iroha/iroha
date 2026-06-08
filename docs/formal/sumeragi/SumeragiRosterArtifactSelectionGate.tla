---- MODULE SumeragiRosterArtifactSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for roster artifact selection.

This slice captures the non-cryptographic selection policy in
`roster_artifact_selection_view(...)` and the final decision block of
`selection_from_roster_artifacts(...)` from `main_loop.rs`. Commit-QC and
validator-checkpoint validation are abstracted to "validated artifact present"
inputs. The model pins observable behavior: view selection prefers the commit
certificate, then the checkpoint, then the block view; no validated artifact
returns no selection; a validated commit certificate owns the selected roster
and commit sidecar; checkpoint-only selections use the checkpoint roster; when
both artifacts validate, the commit certificate is preferred and the checkpoint
is attached only when its view and state roots match the certificate; roster
mismatch alone does not drop a consistent checkpoint; stake snapshots resolve
against the selected roster in direct, cert-input, checkpoint-input order; and
epoch/input/root/genesis-stub helper choices feed checkpoint validation exactly
as the implementation does.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ViewCommitPriority == 1
ViewCheckpointFallback == 2
ViewBlockFallback == 3
ViewNoneWhenNoInputs == 4
NoValidatedArtifactsNone == 5
CertOnlySelection == 6
CheckpointOnlySelection == 7
BothMatchingKeepsBoth == 8
BothViewMismatchDropsCheckpoint == 9
BothRootsMismatchDropsCheckpoint == 10
RosterMismatchUsesCertRoster == 11
RosterMismatchKeepsCheckpointWhenConsistent == 12
StakeDirectPreferred == 13
StakeCertInputsFallback == 14
StakeCheckpointInputsFallback == 15
StakeNoMatchNone == 16
StakeResolvedAgainstCertRoster == 17
StakeResolvedAgainstCheckpointRoster == 18
CheckpointReusesCertInputsSameSet == 19
CheckpointUsesOwnInputsDifferentSet == 20
CheckpointRootsPreferCert == 21
CheckpointRootsFallbackHistory == 22
NposEpochFromCert == 23
NposEpochExpectedWithoutCert == 24
PermissionedEpochZero == 25
GenesisStubAllowed == 26
GenesisStubDeniedBySource == 27
GenesisStubDeniedByHeightOrView == 28

Candidates == 1..28

ViewSelectionCases == {
  ViewCommitPriority,
  ViewCheckpointFallback,
  ViewBlockFallback,
  ViewNoneWhenNoInputs
}

ArtifactAttachmentCases == {
  NoValidatedArtifactsNone,
  CertOnlySelection,
  CheckpointOnlySelection,
  BothMatchingKeepsBoth
}

CheckpointCompatibilityCases == {
  BothViewMismatchDropsCheckpoint,
  BothRootsMismatchDropsCheckpoint,
  RosterMismatchUsesCertRoster,
  RosterMismatchKeepsCheckpointWhenConsistent
}

StakePriorityCases == {
  StakeDirectPreferred,
  StakeCertInputsFallback,
  StakeCheckpointInputsFallback,
  StakeNoMatchNone
}

StakeRosterCases == {
  StakeResolvedAgainstCertRoster,
  StakeResolvedAgainstCheckpointRoster
}

CheckpointValidationInputCases == {
  CheckpointReusesCertInputsSameSet,
  CheckpointUsesOwnInputsDifferentSet,
  CheckpointRootsPreferCert,
  CheckpointRootsFallbackHistory
}

EpochSelectionCases == {
  NposEpochFromCert,
  NposEpochExpectedWithoutCert,
  PermissionedEpochZero
}

GenesisStubCases == {
  GenesisStubAllowed,
  GenesisStubDeniedBySource,
  GenesisStubDeniedByHeightOrView
}

ViewCommit == 1
ViewCheckpoint == 2
ViewBlock == 3
ViewNone == 4
ResultNone == 5
ResultSome == 6
RosterFromCert == 7
RosterFromCheckpoint == 8
CommitAttached == 9
CommitAbsent == 10
CheckpointAttached == 11
CheckpointAbsent == 12
SourcePreserved == 13
DropCheckpointOnViewMismatch == 14
DropCheckpointOnRootsMismatch == 15
KeepCheckpointOnRosterMismatch == 16
PreferCertOverCheckpointRoster == 17
StakeFromDirect == 18
StakeFromCertInputs == 19
StakeFromCheckpointInputs == 20
StakeAbsent == 21
StakeMatchSelectedRoster == 22
StakeIgnoreNonMatching == 23
ReuseCertInputs == 24
UseCheckpointInputs == 25
ValidateCheckpointIfPresent == 26
RootsFromCert == 27
RootsFromHistory == 28
EpochCert == 29
EpochExpected == 30
EpochZero == 31
AllowGenesis == 32
DenyGenesis == 33

Actions == 1..33

SpecActions(candidate) ==
  CASE candidate = ViewCommitPriority ->
      {ViewCommit}
    [] candidate = ViewCheckpointFallback ->
      {ViewCheckpoint}
    [] candidate = ViewBlockFallback ->
      {ViewBlock}
    [] candidate = ViewNoneWhenNoInputs ->
      {ViewNone}
    [] candidate = NoValidatedArtifactsNone ->
      {ResultNone, CommitAbsent, CheckpointAbsent, StakeAbsent}
    [] candidate = CertOnlySelection ->
      {ResultSome, RosterFromCert, CommitAttached, CheckpointAbsent, SourcePreserved}
    [] candidate = CheckpointOnlySelection ->
      {ResultSome, RosterFromCheckpoint, CommitAbsent, CheckpointAttached, SourcePreserved}
    [] candidate = BothMatchingKeepsBoth ->
      {ResultSome, RosterFromCert, CommitAttached, CheckpointAttached, SourcePreserved}
    [] candidate = BothViewMismatchDropsCheckpoint ->
      {ResultSome, RosterFromCert, CommitAttached, CheckpointAbsent,
       DropCheckpointOnViewMismatch}
    [] candidate = BothRootsMismatchDropsCheckpoint ->
      {ResultSome, RosterFromCert, CommitAttached, CheckpointAbsent,
       DropCheckpointOnRootsMismatch}
    [] candidate = RosterMismatchUsesCertRoster ->
      {ResultSome, RosterFromCert, PreferCertOverCheckpointRoster}
    [] candidate = RosterMismatchKeepsCheckpointWhenConsistent ->
      {ResultSome, CommitAttached, CheckpointAttached,
       KeepCheckpointOnRosterMismatch}
    [] candidate = StakeDirectPreferred ->
      {StakeFromDirect, StakeMatchSelectedRoster}
    [] candidate = StakeCertInputsFallback ->
      {StakeFromCertInputs, StakeMatchSelectedRoster, StakeIgnoreNonMatching}
    [] candidate = StakeCheckpointInputsFallback ->
      {StakeFromCheckpointInputs, StakeMatchSelectedRoster, StakeIgnoreNonMatching}
    [] candidate = StakeNoMatchNone ->
      {StakeAbsent, StakeIgnoreNonMatching}
    [] candidate = StakeResolvedAgainstCertRoster ->
      {RosterFromCert, StakeMatchSelectedRoster, StakeIgnoreNonMatching}
    [] candidate = StakeResolvedAgainstCheckpointRoster ->
      {RosterFromCheckpoint, StakeMatchSelectedRoster, StakeIgnoreNonMatching}
    [] candidate = CheckpointReusesCertInputsSameSet ->
      {ReuseCertInputs, ValidateCheckpointIfPresent}
    [] candidate = CheckpointUsesOwnInputsDifferentSet ->
      {UseCheckpointInputs, ValidateCheckpointIfPresent}
    [] candidate = CheckpointRootsPreferCert ->
      {RootsFromCert}
    [] candidate = CheckpointRootsFallbackHistory ->
      {RootsFromHistory}
    [] candidate = NposEpochFromCert ->
      {EpochCert}
    [] candidate = NposEpochExpectedWithoutCert ->
      {EpochExpected}
    [] candidate = PermissionedEpochZero ->
      {EpochZero}
    [] candidate = GenesisStubAllowed ->
      {AllowGenesis}
    [] candidate = GenesisStubDeniedBySource ->
      {DenyGenesis}
    [] candidate = GenesisStubDeniedByHeightOrView ->
      {DenyGenesis}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ViewCommitPriority /\ Bug = "view_commit_uses_checkpoint" ->
      (spec \ {ViewCommit}) \cup {ViewCheckpoint}
    [] candidate = ViewCheckpointFallback /\ Bug = "view_checkpoint_uses_block" ->
      (spec \ {ViewCheckpoint}) \cup {ViewBlock}
    [] candidate = ViewBlockFallback /\ Bug = "view_block_returns_none" ->
      (spec \ {ViewBlock}) \cup {ViewNone}
    [] candidate = ViewNoneWhenNoInputs /\ Bug = "view_none_returns_block" ->
      (spec \ {ViewNone}) \cup {ViewBlock}
    [] candidate = NoValidatedArtifactsNone /\ Bug = "no_artifacts_selects_empty" ->
      (spec \ {ResultNone}) \cup {ResultSome}
    [] candidate = CertOnlySelection /\ Bug = "cert_only_drops_cert" ->
      (spec \ {CommitAttached, RosterFromCert}) \cup {CommitAbsent}
    [] candidate = CheckpointOnlySelection /\ Bug = "checkpoint_only_drops_checkpoint" ->
      (spec \ {CheckpointAttached, RosterFromCheckpoint}) \cup {CheckpointAbsent}
    [] candidate = BothMatchingKeepsBoth /\ Bug = "both_matching_drops_checkpoint" ->
      (spec \ {CheckpointAttached}) \cup {CheckpointAbsent}
    [] candidate = BothViewMismatchDropsCheckpoint /\
          Bug = "view_mismatch_keeps_checkpoint" ->
      (spec \ {CheckpointAbsent, DropCheckpointOnViewMismatch}) \cup
        {CheckpointAttached}
    [] candidate = BothRootsMismatchDropsCheckpoint /\
          Bug = "roots_mismatch_keeps_checkpoint" ->
      (spec \ {CheckpointAbsent, DropCheckpointOnRootsMismatch}) \cup
        {CheckpointAttached}
    [] candidate = RosterMismatchUsesCertRoster /\
          Bug = "roster_mismatch_uses_checkpoint_roster" ->
      (spec \ {RosterFromCert, PreferCertOverCheckpointRoster}) \cup
        {RosterFromCheckpoint}
    [] candidate = RosterMismatchKeepsCheckpointWhenConsistent /\
          Bug = "roster_mismatch_drops_checkpoint" ->
      (spec \ {CheckpointAttached, KeepCheckpointOnRosterMismatch}) \cup
        {CheckpointAbsent}
    [] candidate = StakeDirectPreferred /\ Bug = "stake_direct_not_preferred" ->
      (spec \ {StakeFromDirect}) \cup {StakeFromCertInputs}
    [] candidate = StakeCertInputsFallback /\ Bug = "stake_cert_fallback_skipped" ->
      (spec \ {StakeFromCertInputs}) \cup {StakeAbsent}
    [] candidate = StakeCheckpointInputsFallback /\
          Bug = "stake_checkpoint_fallback_skipped" ->
      (spec \ {StakeFromCheckpointInputs}) \cup {StakeAbsent}
    [] candidate = StakeNoMatchNone /\ Bug = "stake_no_match_attaches" ->
      (spec \ {StakeAbsent, StakeIgnoreNonMatching}) \cup {StakeFromDirect}
    [] candidate = StakeResolvedAgainstCertRoster /\
          Bug = "stake_uses_checkpoint_roster_with_cert" ->
      (spec \ {RosterFromCert}) \cup {RosterFromCheckpoint}
    [] candidate = StakeResolvedAgainstCheckpointRoster /\
          Bug = "checkpoint_only_stake_uses_cert_roster" ->
      (spec \ {RosterFromCheckpoint}) \cup {RosterFromCert}
    [] candidate = CheckpointReusesCertInputsSameSet /\
          Bug = "checkpoint_recomputes_inputs_for_same_set" ->
      (spec \ {ReuseCertInputs}) \cup {UseCheckpointInputs}
    [] candidate = CheckpointUsesOwnInputsDifferentSet /\
          Bug = "checkpoint_reuses_inputs_for_different_set" ->
      (spec \ {UseCheckpointInputs}) \cup {ReuseCertInputs}
    [] candidate = CheckpointRootsPreferCert /\
          Bug = "checkpoint_roots_ignore_cert" ->
      (spec \ {RootsFromCert}) \cup {RootsFromHistory}
    [] candidate = CheckpointRootsFallbackHistory /\
          Bug = "checkpoint_roots_ignore_history" ->
      spec \ {RootsFromHistory}
    [] candidate = NposEpochFromCert /\ Bug = "npos_epoch_ignores_cert" ->
      (spec \ {EpochCert}) \cup {EpochExpected}
    [] candidate = NposEpochExpectedWithoutCert /\
          Bug = "npos_epoch_uses_zero_without_cert" ->
      (spec \ {EpochExpected}) \cup {EpochZero}
    [] candidate = PermissionedEpochZero /\
          Bug = "permissioned_epoch_uses_schedule" ->
      (spec \ {EpochZero}) \cup {EpochExpected}
    [] candidate = GenesisStubAllowed /\
          Bug = "genesis_stub_denies_allowed" ->
      (spec \ {AllowGenesis}) \cup {DenyGenesis}
    [] candidate = GenesisStubDeniedBySource /\
          Bug = "genesis_stub_ignores_source" ->
      (spec \ {DenyGenesis}) \cup {AllowGenesis}
    [] candidate = GenesisStubDeniedByHeightOrView /\
          Bug = "genesis_stub_ignores_height_or_view" ->
      (spec \ {DenyGenesis}) \cup {AllowGenesis}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "view_commit_uses_checkpoint",
       "view_checkpoint_uses_block",
       "view_block_returns_none",
       "view_none_returns_block",
       "no_artifacts_selects_empty",
       "cert_only_drops_cert",
       "checkpoint_only_drops_checkpoint",
       "both_matching_drops_checkpoint",
       "view_mismatch_keeps_checkpoint",
       "roots_mismatch_keeps_checkpoint",
       "roster_mismatch_uses_checkpoint_roster",
       "roster_mismatch_drops_checkpoint",
       "stake_direct_not_preferred",
       "stake_cert_fallback_skipped",
       "stake_checkpoint_fallback_skipped",
       "stake_no_match_attaches",
       "stake_uses_checkpoint_roster_with_cert",
       "checkpoint_only_stake_uses_cert_roster",
       "checkpoint_recomputes_inputs_for_same_set",
       "checkpoint_reuses_inputs_for_different_set",
       "checkpoint_roots_ignore_cert",
       "checkpoint_roots_ignore_history",
       "npos_epoch_ignores_cert",
       "npos_epoch_uses_zero_without_cert",
       "permissioned_epoch_uses_schedule",
       "genesis_stub_denies_allowed",
       "genesis_stub_ignores_source",
       "genesis_stub_ignores_height_or_view"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionViewExact ==
  \A c \in ViewSelectionCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionAttachmentExact ==
  \A c \in ArtifactAttachmentCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionCheckpointCompatibilityExact ==
  \A c \in CheckpointCompatibilityCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionStakePriorityExact ==
  \A c \in StakePriorityCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionStakeRosterExact ==
  \A c \in StakeRosterCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionCheckpointInputExact ==
  \A c \in CheckpointValidationInputCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionEpochExact ==
  \A c \in EpochSelectionCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionGenesisStubExact ==
  \A c \in GenesisStubCases:
    ImplementationActions(c) = SpecActions(c)

RosterArtifactSelectionExactness ==
  /\ RosterArtifactSelectionViewExact
  /\ RosterArtifactSelectionAttachmentExact
  /\ RosterArtifactSelectionCheckpointCompatibilityExact
  /\ RosterArtifactSelectionStakePriorityExact
  /\ RosterArtifactSelectionStakeRosterExact
  /\ RosterArtifactSelectionCheckpointInputExact
  /\ RosterArtifactSelectionEpochExact
  /\ RosterArtifactSelectionGenesisStubExact

BugViewCommitUsesCheckpoint ==
  ImplementationActions(ViewCommitPriority) = SpecActions(ViewCommitPriority)

BugViewCheckpointUsesBlock ==
  ImplementationActions(ViewCheckpointFallback) =
    SpecActions(ViewCheckpointFallback)

BugViewBlockReturnsNone ==
  ImplementationActions(ViewBlockFallback) = SpecActions(ViewBlockFallback)

BugViewNoneReturnsBlock ==
  ImplementationActions(ViewNoneWhenNoInputs) = SpecActions(ViewNoneWhenNoInputs)

BugNoArtifactsSelectsEmpty ==
  ImplementationActions(NoValidatedArtifactsNone) =
    SpecActions(NoValidatedArtifactsNone)

BugCertOnlyDropsCert ==
  ImplementationActions(CertOnlySelection) = SpecActions(CertOnlySelection)

BugCheckpointOnlyDropsCheckpoint ==
  ImplementationActions(CheckpointOnlySelection) =
    SpecActions(CheckpointOnlySelection)

BugBothMatchingDropsCheckpoint ==
  ImplementationActions(BothMatchingKeepsBoth) =
    SpecActions(BothMatchingKeepsBoth)

BugViewMismatchKeepsCheckpoint ==
  ImplementationActions(BothViewMismatchDropsCheckpoint) =
    SpecActions(BothViewMismatchDropsCheckpoint)

BugRootsMismatchKeepsCheckpoint ==
  ImplementationActions(BothRootsMismatchDropsCheckpoint) =
    SpecActions(BothRootsMismatchDropsCheckpoint)

BugRosterMismatchUsesCheckpointRoster ==
  ImplementationActions(RosterMismatchUsesCertRoster) =
    SpecActions(RosterMismatchUsesCertRoster)

BugRosterMismatchDropsCheckpoint ==
  ImplementationActions(RosterMismatchKeepsCheckpointWhenConsistent) =
    SpecActions(RosterMismatchKeepsCheckpointWhenConsistent)

BugStakeDirectNotPreferred ==
  ImplementationActions(StakeDirectPreferred) = SpecActions(StakeDirectPreferred)

BugStakeCertFallbackSkipped ==
  ImplementationActions(StakeCertInputsFallback) =
    SpecActions(StakeCertInputsFallback)

BugStakeCheckpointFallbackSkipped ==
  ImplementationActions(StakeCheckpointInputsFallback) =
    SpecActions(StakeCheckpointInputsFallback)

BugStakeNoMatchAttaches ==
  ImplementationActions(StakeNoMatchNone) = SpecActions(StakeNoMatchNone)

BugStakeUsesCheckpointRosterWithCert ==
  ImplementationActions(StakeResolvedAgainstCertRoster) =
    SpecActions(StakeResolvedAgainstCertRoster)

BugCheckpointOnlyStakeUsesCertRoster ==
  ImplementationActions(StakeResolvedAgainstCheckpointRoster) =
    SpecActions(StakeResolvedAgainstCheckpointRoster)

BugCheckpointRecomputesInputsForSameSet ==
  ImplementationActions(CheckpointReusesCertInputsSameSet) =
    SpecActions(CheckpointReusesCertInputsSameSet)

BugCheckpointReusesInputsForDifferentSet ==
  ImplementationActions(CheckpointUsesOwnInputsDifferentSet) =
    SpecActions(CheckpointUsesOwnInputsDifferentSet)

BugCheckpointRootsIgnoreCert ==
  ImplementationActions(CheckpointRootsPreferCert) =
    SpecActions(CheckpointRootsPreferCert)

BugCheckpointRootsIgnoreHistory ==
  ImplementationActions(CheckpointRootsFallbackHistory) =
    SpecActions(CheckpointRootsFallbackHistory)

BugNposEpochIgnoresCert ==
  ImplementationActions(NposEpochFromCert) = SpecActions(NposEpochFromCert)

BugNposEpochUsesZeroWithoutCert ==
  ImplementationActions(NposEpochExpectedWithoutCert) =
    SpecActions(NposEpochExpectedWithoutCert)

BugPermissionedEpochUsesSchedule ==
  ImplementationActions(PermissionedEpochZero) =
    SpecActions(PermissionedEpochZero)

BugGenesisStubDeniesAllowed ==
  ImplementationActions(GenesisStubAllowed) =
    SpecActions(GenesisStubAllowed)

BugGenesisStubIgnoresSource ==
  ImplementationActions(GenesisStubDeniedBySource) =
    SpecActions(GenesisStubDeniedBySource)

BugGenesisStubIgnoresHeightOrView ==
  ImplementationActions(GenesisStubDeniedByHeightOrView) =
    SpecActions(GenesisStubDeniedByHeightOrView)

====
