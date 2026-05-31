---- MODULE SumeragiBlockSyncRosterEvidenceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for block-sync roster evidence helpers.

This slice captures `apply_roster_selection_to_block_sync_update(...)`,
`classify_block_sync_roster_evidence(...)`, and
`block_sync_update_has_roster(...)` from `main_loop.rs`. Commit QCs,
validator checkpoints, stake snapshots, and unrelated update fields are
collapsed into representative flags while preserving observable behavior:
a block-sync update is missing commit proof unless it carries either a commit
QC or a validator checkpoint; missing commit proof has priority over NPoS stake
checks; Permissioned updates with any commit proof are verifiable without a
stake snapshot; NPoS updates with any commit proof require a stake snapshot;
`block_sync_update_has_roster(...)` is true exactly for verifiable evidence; and
applying a selection overwrites the update commit QC, validator checkpoint, and
stake snapshot lanes while preserving unrelated update fields.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoProofPermissioned == 1
NoProofNpos == 2
QcOnlyPermissioned == 3
CheckpointOnlyPermissioned == 4
BothProofPermissioned == 5
QcOnlyNposNoStake == 6
CheckpointOnlyNposNoStake == 7
BothProofNposNoStake == 8
QcOnlyNposWithStake == 9
CheckpointOnlyNposWithStake == 10
BothProofNposWithStake == 11
HasRosterForVerifiable == 12
HasRosterForMissingCommitProof == 13
HasRosterForMissingStake == 14
ApplyCopiesCommitQc == 15
ApplyCopiesCheckpoint == 16
ApplyCopiesStakeSnapshot == 17
ApplyClearsAbsentCommitQc == 18
ApplyClearsAbsentCheckpoint == 19
ApplyClearsAbsentStakeSnapshot == 20
ApplyPreservesUnrelatedFields == 21

Candidates == 1..21

MissingCommitProof == 1
MissingStakeSnapshot == 2
Verifiable == 3
HasRosterTrue == 4
HasRosterFalse == 5
CommitProofViaQc == 6
CommitProofViaCheckpoint == 7
PermissionedMode == 8
NposMode == 9
StakeRequired == 10
StakePresent == 11
StakeMissing == 12
MissingProofPriority == 13
CommitQcCloned == 14
CheckpointCloned == 15
StakeSnapshotCloned == 16
CommitQcCleared == 17
CheckpointCleared == 18
StakeSnapshotCleared == 19
UnrelatedFieldsPreserved == 20

Actions == 1..20

SpecActions(candidate) ==
  CASE candidate = NoProofPermissioned ->
      {PermissionedMode, MissingCommitProof, HasRosterFalse}
    [] candidate = NoProofNpos ->
      {NposMode, MissingCommitProof, MissingProofPriority, HasRosterFalse}
    [] candidate = QcOnlyPermissioned ->
      {PermissionedMode, CommitProofViaQc, Verifiable, HasRosterTrue}
    [] candidate = CheckpointOnlyPermissioned ->
      {PermissionedMode, CommitProofViaCheckpoint, Verifiable, HasRosterTrue}
    [] candidate = BothProofPermissioned ->
      {PermissionedMode, CommitProofViaQc, CommitProofViaCheckpoint,
       Verifiable, HasRosterTrue}
    [] candidate = QcOnlyNposNoStake ->
      {NposMode, CommitProofViaQc, StakeRequired, StakeMissing,
       MissingStakeSnapshot, HasRosterFalse}
    [] candidate = CheckpointOnlyNposNoStake ->
      {NposMode, CommitProofViaCheckpoint, StakeRequired, StakeMissing,
       MissingStakeSnapshot, HasRosterFalse}
    [] candidate = BothProofNposNoStake ->
      {NposMode, CommitProofViaQc, CommitProofViaCheckpoint, StakeRequired,
       StakeMissing, MissingStakeSnapshot, HasRosterFalse}
    [] candidate = QcOnlyNposWithStake ->
      {NposMode, CommitProofViaQc, StakeRequired, StakePresent,
       Verifiable, HasRosterTrue}
    [] candidate = CheckpointOnlyNposWithStake ->
      {NposMode, CommitProofViaCheckpoint, StakeRequired, StakePresent,
       Verifiable, HasRosterTrue}
    [] candidate = BothProofNposWithStake ->
      {NposMode, CommitProofViaQc, CommitProofViaCheckpoint, StakeRequired,
       StakePresent, Verifiable, HasRosterTrue}
    [] candidate = HasRosterForVerifiable ->
      {Verifiable, HasRosterTrue}
    [] candidate = HasRosterForMissingCommitProof ->
      {MissingCommitProof, HasRosterFalse}
    [] candidate = HasRosterForMissingStake ->
      {MissingStakeSnapshot, HasRosterFalse}
    [] candidate = ApplyCopiesCommitQc ->
      {CommitQcCloned}
    [] candidate = ApplyCopiesCheckpoint ->
      {CheckpointCloned}
    [] candidate = ApplyCopiesStakeSnapshot ->
      {StakeSnapshotCloned}
    [] candidate = ApplyClearsAbsentCommitQc ->
      {CommitQcCleared}
    [] candidate = ApplyClearsAbsentCheckpoint ->
      {CheckpointCleared}
    [] candidate = ApplyClearsAbsentStakeSnapshot ->
      {StakeSnapshotCleared}
    [] candidate = ApplyPreservesUnrelatedFields ->
      {UnrelatedFieldsPreserved}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = NoProofPermissioned /\
          Bug = "no_proof_permissioned_verifiable" ->
      (spec \ {MissingCommitProof, HasRosterFalse}) \cup
        {Verifiable, HasRosterTrue}
    [] candidate = NoProofNpos /\ Bug = "no_proof_npos_missing_stake" ->
      (spec \ {MissingCommitProof, MissingProofPriority}) \cup
        {MissingStakeSnapshot}
    [] candidate = QcOnlyPermissioned /\
          Bug = "qc_only_permissioned_missing_commit_proof" ->
      (spec \ {Verifiable, HasRosterTrue}) \cup
        {MissingCommitProof, HasRosterFalse}
    [] candidate = CheckpointOnlyPermissioned /\
          Bug = "checkpoint_only_permissioned_missing_commit_proof" ->
      (spec \ {Verifiable, HasRosterTrue}) \cup
        {MissingCommitProof, HasRosterFalse}
    [] candidate = BothProofPermissioned /\
          Bug = "both_permissioned_missing_commit_proof" ->
      (spec \ {Verifiable, HasRosterTrue}) \cup
        {MissingCommitProof, HasRosterFalse}
    [] candidate = QcOnlyNposNoStake /\
          Bug = "qc_only_npos_verifiable_without_stake" ->
      (spec \ {MissingStakeSnapshot, HasRosterFalse}) \cup
        {Verifiable, HasRosterTrue}
    [] candidate = CheckpointOnlyNposNoStake /\
          Bug = "checkpoint_only_npos_verifiable_without_stake" ->
      (spec \ {MissingStakeSnapshot, HasRosterFalse}) \cup
        {Verifiable, HasRosterTrue}
    [] candidate = BothProofNposNoStake /\
          Bug = "both_npos_verifiable_without_stake" ->
      (spec \ {MissingStakeSnapshot, HasRosterFalse}) \cup
        {Verifiable, HasRosterTrue}
    [] candidate = QcOnlyNposWithStake /\
          Bug = "qc_npos_with_stake_missing_stake" ->
      (spec \ {Verifiable, HasRosterTrue}) \cup
        {MissingStakeSnapshot, HasRosterFalse}
    [] candidate = CheckpointOnlyNposWithStake /\
          Bug = "checkpoint_npos_with_stake_missing_stake" ->
      (spec \ {Verifiable, HasRosterTrue}) \cup
        {MissingStakeSnapshot, HasRosterFalse}
    [] candidate = BothProofNposWithStake /\
          Bug = "both_npos_with_stake_missing_stake" ->
      (spec \ {Verifiable, HasRosterTrue}) \cup
        {MissingStakeSnapshot, HasRosterFalse}
    [] candidate = HasRosterForVerifiable /\
          Bug = "has_roster_false_for_verifiable" ->
      (spec \ {HasRosterTrue}) \cup {HasRosterFalse}
    [] candidate = HasRosterForMissingCommitProof /\
          Bug = "has_roster_true_for_missing_commit_proof" ->
      (spec \ {HasRosterFalse}) \cup {HasRosterTrue}
    [] candidate = HasRosterForMissingStake /\
          Bug = "has_roster_true_for_missing_stake" ->
      (spec \ {HasRosterFalse}) \cup {HasRosterTrue}
    [] candidate = ApplyCopiesCommitQc /\ Bug = "apply_skips_commit_qc" ->
      spec \ {CommitQcCloned}
    [] candidate = ApplyCopiesCheckpoint /\ Bug = "apply_skips_checkpoint" ->
      spec \ {CheckpointCloned}
    [] candidate = ApplyCopiesStakeSnapshot /\ Bug = "apply_skips_stake_snapshot" ->
      spec \ {StakeSnapshotCloned}
    [] candidate = ApplyClearsAbsentCommitQc /\
          Bug = "apply_keeps_old_commit_qc_when_absent" ->
      (spec \ {CommitQcCleared}) \cup {CommitQcCloned}
    [] candidate = ApplyClearsAbsentCheckpoint /\
          Bug = "apply_keeps_old_checkpoint_when_absent" ->
      (spec \ {CheckpointCleared}) \cup {CheckpointCloned}
    [] candidate = ApplyClearsAbsentStakeSnapshot /\
          Bug = "apply_keeps_old_stake_when_absent" ->
      (spec \ {StakeSnapshotCleared}) \cup {StakeSnapshotCloned}
    [] candidate = ApplyPreservesUnrelatedFields /\
          Bug = "apply_changes_unrelated_fields" ->
      spec \ {UnrelatedFieldsPreserved}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "no_proof_permissioned_verifiable",
       "no_proof_npos_missing_stake",
       "qc_only_permissioned_missing_commit_proof",
       "checkpoint_only_permissioned_missing_commit_proof",
       "both_permissioned_missing_commit_proof",
       "qc_only_npos_verifiable_without_stake",
       "checkpoint_only_npos_verifiable_without_stake",
       "both_npos_verifiable_without_stake",
       "qc_npos_with_stake_missing_stake",
       "checkpoint_npos_with_stake_missing_stake",
       "both_npos_with_stake_missing_stake",
       "has_roster_false_for_verifiable",
       "has_roster_true_for_missing_commit_proof",
       "has_roster_true_for_missing_stake",
       "apply_skips_commit_qc",
       "apply_skips_checkpoint",
       "apply_skips_stake_snapshot",
       "apply_keeps_old_commit_qc_when_absent",
       "apply_keeps_old_checkpoint_when_absent",
       "apply_keeps_old_stake_when_absent",
       "apply_changes_unrelated_fields"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BugNoProofPermissionedVerifiable ==
  ImplementationActions(NoProofPermissioned) = SpecActions(NoProofPermissioned)

BugNoProofNposMissingStake ==
  ImplementationActions(NoProofNpos) = SpecActions(NoProofNpos)

BugQcOnlyPermissionedMissingCommitProof ==
  ImplementationActions(QcOnlyPermissioned) = SpecActions(QcOnlyPermissioned)

BugCheckpointOnlyPermissionedMissingCommitProof ==
  ImplementationActions(CheckpointOnlyPermissioned) =
    SpecActions(CheckpointOnlyPermissioned)

BugBothPermissionedMissingCommitProof ==
  ImplementationActions(BothProofPermissioned) =
    SpecActions(BothProofPermissioned)

BugQcOnlyNposVerifiableWithoutStake ==
  ImplementationActions(QcOnlyNposNoStake) = SpecActions(QcOnlyNposNoStake)

BugCheckpointOnlyNposVerifiableWithoutStake ==
  ImplementationActions(CheckpointOnlyNposNoStake) =
    SpecActions(CheckpointOnlyNposNoStake)

BugBothNposVerifiableWithoutStake ==
  ImplementationActions(BothProofNposNoStake) =
    SpecActions(BothProofNposNoStake)

BugQcNposWithStakeMissingStake ==
  ImplementationActions(QcOnlyNposWithStake) =
    SpecActions(QcOnlyNposWithStake)

BugCheckpointNposWithStakeMissingStake ==
  ImplementationActions(CheckpointOnlyNposWithStake) =
    SpecActions(CheckpointOnlyNposWithStake)

BugBothNposWithStakeMissingStake ==
  ImplementationActions(BothProofNposWithStake) =
    SpecActions(BothProofNposWithStake)

BugHasRosterFalseForVerifiable ==
  ImplementationActions(HasRosterForVerifiable) =
    SpecActions(HasRosterForVerifiable)

BugHasRosterTrueForMissingCommitProof ==
  ImplementationActions(HasRosterForMissingCommitProof) =
    SpecActions(HasRosterForMissingCommitProof)

BugHasRosterTrueForMissingStake ==
  ImplementationActions(HasRosterForMissingStake) =
    SpecActions(HasRosterForMissingStake)

BugApplySkipsCommitQc ==
  ImplementationActions(ApplyCopiesCommitQc) = SpecActions(ApplyCopiesCommitQc)

BugApplySkipsCheckpoint ==
  ImplementationActions(ApplyCopiesCheckpoint) =
    SpecActions(ApplyCopiesCheckpoint)

BugApplySkipsStakeSnapshot ==
  ImplementationActions(ApplyCopiesStakeSnapshot) =
    SpecActions(ApplyCopiesStakeSnapshot)

BugApplyKeepsOldCommitQcWhenAbsent ==
  ImplementationActions(ApplyClearsAbsentCommitQc) =
    SpecActions(ApplyClearsAbsentCommitQc)

BugApplyKeepsOldCheckpointWhenAbsent ==
  ImplementationActions(ApplyClearsAbsentCheckpoint) =
    SpecActions(ApplyClearsAbsentCheckpoint)

BugApplyKeepsOldStakeWhenAbsent ==
  ImplementationActions(ApplyClearsAbsentStakeSnapshot) =
    SpecActions(ApplyClearsAbsentStakeSnapshot)

BugApplyChangesUnrelatedFields ==
  ImplementationActions(ApplyPreservesUnrelatedFields) =
    SpecActions(ApplyPreservesUnrelatedFields)

====
