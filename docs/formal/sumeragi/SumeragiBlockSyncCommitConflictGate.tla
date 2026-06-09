---- MODULE SumeragiBlockSyncCommitConflictGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the committed-height conflict branch in
`handle_block_sync_update(...)`.

When Kura already has a block at the incoming height, the live path compares
that committed hash with the incoming BlockSyncUpdate hash. Matching hashes and
heights that cannot be looked up continue to the normal block-sync path.
Conflicting hashes never overwrite finality:

- conflicts without a commit QC are dropped and clear the missing-block request,
- conflicts with an invalid commit QC are dropped and clear the request,
- conflicts with a valid commit QC emit `InvalidQc` evidence carrying the same
  certificate and the `commit_conflict_finality` reason, then drop and clear,
- evidence broadcast errors do not re-admit the conflicting update, and
- the validation call for conflicting QCs carries the block identity, expected
  epoch, consensus mode/tag, stake snapshot, and the genesis-stub allowance.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "height_zero_skips",
  "committed_absent",
  "committed_same_hash",
  "conflict_no_qc",
  "conflict_invalid_qc",
  "conflict_valid_qc",
  "conflict_valid_qc_evidence_error",
  "conflict_valid_qc_with_stake",
  "conflict_valid_qc_npos",
  "conflict_valid_qc_genesis_stub"
}

HeightConvertible(c) ==
  c # "height_zero_skips"

NonZeroHeight(c) ==
  c # "height_zero_skips"

CommittedPresent(c) ==
  c # "committed_absent" /\ NonZeroHeight(c)

CommittedHashMatches(c) ==
  c = "committed_same_hash"

Conflict(c) ==
  /\ HeightConvertible(c)
  /\ NonZeroHeight(c)
  /\ CommittedPresent(c)
  /\ ~CommittedHashMatches(c)

IncomingQc(c) ==
  c \in {
    "conflict_invalid_qc",
    "conflict_valid_qc",
    "conflict_valid_qc_evidence_error",
    "conflict_valid_qc_with_stake",
    "conflict_valid_qc_npos",
    "conflict_valid_qc_genesis_stub"
  }

QcValid(c) ==
  c \in {
    "conflict_valid_qc",
    "conflict_valid_qc_evidence_error",
    "conflict_valid_qc_with_stake",
    "conflict_valid_qc_npos",
    "conflict_valid_qc_genesis_stub"
  }

EvidenceBroadcastErrors(c) ==
  c = "conflict_valid_qc_evidence_error"

StakeSnapshotPresent(c) ==
  c = "conflict_valid_qc_with_stake"

ConsensusMode(c) ==
  IF c = "conflict_valid_qc_npos" THEN "Npos" ELSE "Permissioned"

ModeTag(c) ==
  IF ConsensusMode(c) = "Npos" THEN "NPOS_TAG" ELSE "PERMISSIONED_TAG"

Height(c) ==
  IF c = "conflict_valid_qc_genesis_stub" THEN 1 ELSE 4

View(c) ==
  IF c = "conflict_valid_qc_genesis_stub" THEN 0 ELSE 2

ExpectedEpoch(c) ==
  IF c = "conflict_valid_qc_genesis_stub" THEN 1 ELSE 7

AllowGenesisStub(c) ==
  Height(c) = 1 /\ View(c) = 0

SpecValidateCalled(c) ==
  Conflict(c) /\ IncomingQc(c)

SpecValidationArgsBound(c) ==
  SpecValidateCalled(c)

SpecValidationUsesStake(c) ==
  IF SpecValidateCalled(c) THEN StakeSnapshotPresent(c) ELSE FALSE

SpecValidationMode(c) ==
  IF SpecValidateCalled(c) THEN ConsensusMode(c) ELSE "none"

SpecValidationModeTag(c) ==
  IF SpecValidateCalled(c) THEN ModeTag(c) ELSE "none"

SpecValidationAllowGenesisStub(c) ==
  IF SpecValidateCalled(c) THEN AllowGenesisStub(c) ELSE FALSE

SpecDrop(c) ==
  Conflict(c)

SpecClearMissing(c) ==
  Conflict(c)

SpecRecordKind(c) ==
  IF Conflict(c) THEN "BlockSyncUpdate" ELSE "none"

SpecRecordOutcome(c) ==
  IF Conflict(c) THEN "Dropped" ELSE "none"

SpecRecordReason(c) ==
  IF Conflict(c) THEN "CommitConflict" ELSE "none"

SpecEvidenceEmitted(c) ==
  Conflict(c) /\ IncomingQc(c) /\ QcValid(c)

SpecEvidenceKind(c) ==
  IF SpecEvidenceEmitted(c) THEN "InvalidQc" ELSE "none"

SpecEvidenceReason(c) ==
  IF SpecEvidenceEmitted(c) THEN "commit_conflict_finality" ELSE "none"

SpecEvidenceCertificate(c) ==
  IF SpecEvidenceEmitted(c) THEN "incoming_qc" ELSE "none"

SpecFallsThrough(c) ==
  ~Conflict(c)

SpecReturnOk(c) ==
  TRUE

ActualValidateCalled(c) ==
  CASE Bug = "valid_qc_skips_validation"
       /\ c = "conflict_valid_qc" -> FALSE
    [] Bug = "no_qc_validates"
       /\ c = "conflict_no_qc" -> TRUE
    [] OTHER -> SpecValidateCalled(c)

ActualValidationArgsBound(c) ==
  IF ~ActualValidateCalled(c) THEN FALSE
  ELSE CASE Bug = "validation_uses_wrong_subject"
            /\ c = "conflict_valid_qc" -> FALSE
         [] OTHER -> TRUE

ActualValidationUsesStake(c) ==
  IF ~ActualValidateCalled(c) THEN FALSE
  ELSE CASE Bug = "validation_drops_stake"
            /\ c = "conflict_valid_qc_with_stake" -> FALSE
         [] OTHER -> StakeSnapshotPresent(c)

ActualValidationMode(c) ==
  IF ~ActualValidateCalled(c) THEN "none"
  ELSE CASE Bug = "validation_uses_permissioned_for_npos"
            /\ c = "conflict_valid_qc_npos" -> "Permissioned"
         [] OTHER -> ConsensusMode(c)

ActualValidationModeTag(c) ==
  IF ~ActualValidateCalled(c) THEN "none"
  ELSE CASE Bug = "validation_uses_wrong_mode_tag"
            /\ c = "conflict_valid_qc_npos" -> "PERMISSIONED_TAG"
         [] OTHER -> ModeTag(c)

ActualValidationAllowGenesisStub(c) ==
  IF ~ActualValidateCalled(c) THEN FALSE
  ELSE CASE Bug = "genesis_stub_not_allowed"
            /\ c = "conflict_valid_qc_genesis_stub" -> FALSE
         [] OTHER -> AllowGenesisStub(c)

ActualDrop(c) ==
  CASE Bug = "zero_height_drops"
       /\ c = "height_zero_skips" -> TRUE
    [] Bug = "absent_committed_drops"
       /\ c = "committed_absent" -> TRUE
    [] Bug = "same_hash_drops"
       /\ c = "committed_same_hash" -> TRUE
    [] Bug = "no_qc_falls_through"
       /\ c = "conflict_no_qc" -> FALSE
    [] Bug = "invalid_qc_falls_through"
       /\ c = "conflict_invalid_qc" -> FALSE
    [] Bug = "valid_qc_accepts_conflict"
       /\ c = "conflict_valid_qc" -> FALSE
    [] Bug = "evidence_error_aborts_drop"
       /\ c = "conflict_valid_qc_evidence_error" -> FALSE
    [] OTHER -> SpecDrop(c)

ActualClearMissing(c) ==
  IF ~ActualDrop(c) THEN FALSE
  ELSE CASE Bug = "no_qc_no_clear"
            /\ c = "conflict_no_qc" -> FALSE
         [] Bug = "invalid_qc_no_clear"
            /\ c = "conflict_invalid_qc" -> FALSE
         [] Bug = "valid_qc_no_clear"
            /\ c = "conflict_valid_qc" -> FALSE
         [] OTHER -> Conflict(c)

ActualRecordKind(c) ==
  IF ~ActualDrop(c) THEN "none"
  ELSE "BlockSyncUpdate"

ActualRecordOutcome(c) ==
  IF ~ActualDrop(c) THEN "none"
  ELSE "Dropped"

ActualRecordReason(c) ==
  IF ~ActualDrop(c) THEN "none"
  ELSE CASE Bug = "wrong_drop_reason"
            /\ c = "conflict_no_qc" -> "FutureWindow"
         [] OTHER -> "CommitConflict"

ActualEvidenceEmitted(c) ==
  CASE Bug = "invalid_qc_emits_evidence"
       /\ c = "conflict_invalid_qc" -> TRUE
    [] Bug = "valid_qc_missing_evidence"
       /\ c = "conflict_valid_qc" -> FALSE
    [] Bug # "valid_qc_missing_evidence"
       /\ ActualDrop(c)
       /\ IncomingQc(c)
       /\ QcValid(c) -> TRUE
    [] OTHER -> FALSE

ActualEvidenceKind(c) ==
  IF ~ActualEvidenceEmitted(c) THEN "none"
  ELSE CASE Bug = "valid_qc_wrong_evidence_kind"
            /\ c = "conflict_valid_qc" -> "InvalidProposal"
         [] OTHER -> "InvalidQc"

ActualEvidenceReason(c) ==
  IF ~ActualEvidenceEmitted(c) THEN "none"
  ELSE CASE Bug = "valid_qc_wrong_evidence_reason"
            /\ c = "conflict_valid_qc" -> "wrong_reason"
         [] OTHER -> "commit_conflict_finality"

ActualEvidenceCertificate(c) ==
  IF ~ActualEvidenceEmitted(c) THEN "none"
  ELSE CASE Bug = "valid_qc_wrong_certificate"
            /\ c = "conflict_valid_qc" -> "other_qc"
         [] OTHER -> "incoming_qc"

ActualFallsThrough(c) ==
  ~ActualDrop(c)

ActualReturnOk(c) ==
  CASE Bug = "valid_qc_returns_error"
       /\ c = "conflict_valid_qc" -> FALSE
    [] OTHER -> TRUE

Matches(c) ==
  /\ ActualValidateCalled(c) = SpecValidateCalled(c)
  /\ ActualValidationArgsBound(c) = SpecValidationArgsBound(c)
  /\ ActualValidationUsesStake(c) = SpecValidationUsesStake(c)
  /\ ActualValidationMode(c) = SpecValidationMode(c)
  /\ ActualValidationModeTag(c) = SpecValidationModeTag(c)
  /\ ActualValidationAllowGenesisStub(c) = SpecValidationAllowGenesisStub(c)
  /\ ActualDrop(c) = SpecDrop(c)
  /\ ActualClearMissing(c) = SpecClearMissing(c)
  /\ ActualRecordKind(c) = SpecRecordKind(c)
  /\ ActualRecordOutcome(c) = SpecRecordOutcome(c)
  /\ ActualRecordReason(c) = SpecRecordReason(c)
  /\ ActualEvidenceEmitted(c) = SpecEvidenceEmitted(c)
  /\ ActualEvidenceKind(c) = SpecEvidenceKind(c)
  /\ ActualEvidenceReason(c) = SpecEvidenceReason(c)
  /\ ActualEvidenceCertificate(c) = SpecEvidenceCertificate(c)
  /\ ActualFallsThrough(c) = SpecFallsThrough(c)
  /\ ActualReturnOk(c) = SpecReturnOk(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "zero_height_drops",
       "absent_committed_drops",
       "same_hash_drops",
       "no_qc_falls_through",
       "no_qc_no_clear",
       "no_qc_validates",
       "invalid_qc_falls_through",
       "invalid_qc_no_clear",
       "invalid_qc_emits_evidence",
       "valid_qc_accepts_conflict",
       "valid_qc_skips_validation",
       "valid_qc_missing_evidence",
       "valid_qc_wrong_evidence_kind",
       "valid_qc_wrong_evidence_reason",
       "valid_qc_wrong_certificate",
       "valid_qc_no_clear",
       "valid_qc_returns_error",
       "evidence_error_aborts_drop",
       "validation_uses_wrong_subject",
       "validation_drops_stake",
       "validation_uses_permissioned_for_npos",
       "validation_uses_wrong_mode_tag",
       "genesis_stub_not_allowed",
       "wrong_drop_reason"
     }
  /\ checked = 0

CommitConflictMatchesSpec ==
  \A c \in Cases: Matches(c)

SafetyFast == CommitConflictMatchesSpec

ZeroHeightSkips ==
  Matches("height_zero_skips")

CommittedAbsentContinues ==
  Matches("committed_absent")

SameHashContinues ==
  Matches("committed_same_hash")

NoQcConflictDrops ==
  Matches("conflict_no_qc")

NoQcConflictClears ==
  Matches("conflict_no_qc")

NoQcConflictDoesNotValidate ==
  Matches("conflict_no_qc")

InvalidQcConflictDrops ==
  Matches("conflict_invalid_qc")

InvalidQcConflictClears ==
  Matches("conflict_invalid_qc")

InvalidQcConflictNoEvidence ==
  Matches("conflict_invalid_qc")

ValidQcConflictDrops ==
  Matches("conflict_valid_qc")

ValidQcConflictValidates ==
  Matches("conflict_valid_qc")

ValidQcConflictEvidence ==
  Matches("conflict_valid_qc")

ValidQcConflictClears ==
  Matches("conflict_valid_qc")

ValidQcConflictReturnsOk ==
  Matches("conflict_valid_qc")

EvidenceErrorStillDrops ==
  Matches("conflict_valid_qc_evidence_error")

ValidationBindsSubject ==
  Matches("conflict_valid_qc")

ValidationPassesStake ==
  Matches("conflict_valid_qc_with_stake")

ValidationUsesConsensusMode ==
  Matches("conflict_valid_qc_npos")

ValidationUsesModeTag ==
  Matches("conflict_valid_qc_npos")

GenesisStubAllowedAtHeightOneViewZero ==
  Matches("conflict_valid_qc_genesis_stub")

DropRecordUsesCommitConflict ==
  Matches("conflict_no_qc")

=============================================================================
====
