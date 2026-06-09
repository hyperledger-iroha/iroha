---- MODULE SumeragiQcValidationEvidenceGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `qc_validation_error_to_evidence(...)` and the
evidence-returning part of `validate_qc_with_evidence(...)`.

Hard QC validation failures prove a malformed or Byzantine certificate and must
emit `InvalidQc` evidence that clones the QC and carries the exact telemetry
reason. Soft failures caused by missing local context or insufficient quorum
must not emit evidence. Successful validation must return `Ok` with no
evidence side effect.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

OkResult == 0
ErrResult == 1

NoKind == 0
InvalidQcKind == 1
InvalidProposalKind == 2

NoCertificate == 0
QcClone == 1
WrongCertificate == 2

NoReason == 0
WrongReason == 99

Success == 0
BitmapLengthMismatch == 1
SignerOutOfBounds == 2
InsufficientSigners == 3
MissingVotes == 4
DuplicateSigners == 5
ModeTagMismatch == 6
PhaseMismatch == 7
ValidatorSetMismatch == 8
ViewMismatch == 9
AggregateMismatch == 10
SubjectMismatch == 11
RootsMismatch == 12
HighestQcMismatch == 13
InvalidSignature == 14
SignerMissingFromBlock == 15
StakeSnapshotUnavailable == 16
StakeQuorumMissing == 17

HardErrors == {
  BitmapLengthMismatch,
  SignerOutOfBounds,
  DuplicateSigners,
  ModeTagMismatch,
  PhaseMismatch,
  ValidatorSetMismatch,
  ViewMismatch,
  AggregateMismatch,
  SubjectMismatch,
  RootsMismatch,
  HighestQcMismatch,
  InvalidSignature,
  SignerMissingFromBlock
}

SoftErrors == {
  InsufficientSigners,
  MissingVotes,
  StakeSnapshotUnavailable,
  StakeQuorumMissing
}

AllCases == {Success} \cup HardErrors \cup SoftErrors

SpecResult(c) ==
  IF c = Success THEN OkResult ELSE ErrResult

ActualResult(c) ==
  CASE Bug = "validation_error_returns_ok"
       /\ c = SignerOutOfBounds -> OkResult
    [] Bug = "validation_ok_returns_err"
       /\ c = Success -> ErrResult
    [] OTHER -> SpecResult(c)

SpecEmits(c) ==
  c \in HardErrors

ActualEmits(c) ==
  CASE Bug = "bitmap_length_no_evidence"
       /\ c = BitmapLengthMismatch -> FALSE
    [] Bug = "signer_out_no_evidence"
       /\ c = SignerOutOfBounds -> FALSE
    [] Bug = "duplicate_signers_no_evidence"
       /\ c = DuplicateSigners -> FALSE
    [] Bug = "mode_tag_no_evidence"
       /\ c = ModeTagMismatch -> FALSE
    [] Bug = "phase_no_evidence"
       /\ c = PhaseMismatch -> FALSE
    [] Bug = "validator_set_no_evidence"
       /\ c = ValidatorSetMismatch -> FALSE
    [] Bug = "view_no_evidence"
       /\ c = ViewMismatch -> FALSE
    [] Bug = "aggregate_no_evidence"
       /\ c = AggregateMismatch -> FALSE
    [] Bug = "subject_no_evidence"
       /\ c = SubjectMismatch -> FALSE
    [] Bug = "roots_no_evidence"
       /\ c = RootsMismatch -> FALSE
    [] Bug = "highest_qc_no_evidence"
       /\ c = HighestQcMismatch -> FALSE
    [] Bug = "invalid_signature_no_evidence"
       /\ c = InvalidSignature -> FALSE
    [] Bug = "signer_missing_no_evidence"
       /\ c = SignerMissingFromBlock -> FALSE
    [] Bug = "insufficient_signers_emits"
       /\ c = InsufficientSigners -> TRUE
    [] Bug = "missing_votes_emits"
       /\ c = MissingVotes -> TRUE
    [] Bug = "stake_snapshot_emits"
       /\ c = StakeSnapshotUnavailable -> TRUE
    [] Bug = "stake_quorum_emits"
       /\ c = StakeQuorumMissing -> TRUE
    [] Bug = "validation_success_emits_evidence"
       /\ c = Success -> TRUE
    [] OTHER -> SpecEmits(c)

SpecKind(c) ==
  IF SpecEmits(c) THEN InvalidQcKind ELSE NoKind

ActualKind(c) ==
  CASE Bug = "wrong_kind"
       /\ c = BitmapLengthMismatch -> InvalidProposalKind
    [] ActualEmits(c) -> InvalidQcKind
    [] OTHER -> NoKind

SpecCertificate(c) ==
  IF SpecEmits(c) THEN QcClone ELSE NoCertificate

ActualCertificate(c) ==
  CASE Bug = "drops_certificate"
       /\ c = BitmapLengthMismatch -> NoCertificate
    [] Bug = "wrong_certificate"
       /\ c = BitmapLengthMismatch -> WrongCertificate
    [] ActualEmits(c) -> QcClone
    [] OTHER -> NoCertificate

SpecReason(c) ==
  IF SpecEmits(c) THEN c ELSE NoReason

ActualReason(c) ==
  CASE Bug = "wrong_reason"
       /\ c = BitmapLengthMismatch -> WrongReason
    [] ActualEmits(c) /\ c # Success -> c
    [] OTHER -> NoReason

\* @type: Int => <<Int, Bool, Int, Int, Int>>;
SpecOutput(c) ==
  <<SpecResult(c), SpecEmits(c), SpecKind(c), SpecCertificate(c), SpecReason(c)>>

\* @type: Int => <<Int, Bool, Int, Int, Int>>;
ActualOutput(c) ==
  <<ActualResult(c), ActualEmits(c), ActualKind(c), ActualCertificate(c),
    ActualReason(c)>>

Matches(c) ==
  ActualOutput(c) = SpecOutput(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "bitmap_length_no_evidence",
       "signer_out_no_evidence",
       "duplicate_signers_no_evidence",
       "mode_tag_no_evidence",
       "phase_no_evidence",
       "validator_set_no_evidence",
       "view_no_evidence",
       "aggregate_no_evidence",
       "subject_no_evidence",
       "roots_no_evidence",
       "highest_qc_no_evidence",
       "invalid_signature_no_evidence",
       "signer_missing_no_evidence",
       "insufficient_signers_emits",
       "missing_votes_emits",
       "stake_snapshot_emits",
       "stake_quorum_emits",
       "wrong_kind",
       "drops_certificate",
       "wrong_certificate",
       "wrong_reason",
       "validation_success_emits_evidence",
       "validation_error_returns_ok",
       "validation_ok_returns_err"
     }
  /\ checked = 0

QcValidationEvidenceMatchesSpec ==
  \A c \in AllCases: Matches(c)

SafetyFast ==
  QcValidationEvidenceMatchesSpec

BugBitmapLengthNoEvidence ==
  Matches(BitmapLengthMismatch)

BugSignerOutNoEvidence ==
  Matches(SignerOutOfBounds)

BugDuplicateSignersNoEvidence ==
  Matches(DuplicateSigners)

BugModeTagNoEvidence ==
  Matches(ModeTagMismatch)

BugPhaseNoEvidence ==
  Matches(PhaseMismatch)

BugValidatorSetNoEvidence ==
  Matches(ValidatorSetMismatch)

BugViewNoEvidence ==
  Matches(ViewMismatch)

BugAggregateNoEvidence ==
  Matches(AggregateMismatch)

BugSubjectNoEvidence ==
  Matches(SubjectMismatch)

BugRootsNoEvidence ==
  Matches(RootsMismatch)

BugHighestQcNoEvidence ==
  Matches(HighestQcMismatch)

BugInvalidSignatureNoEvidence ==
  Matches(InvalidSignature)

BugSignerMissingNoEvidence ==
  Matches(SignerMissingFromBlock)

BugInsufficientSignersEmits ==
  Matches(InsufficientSigners)

BugMissingVotesEmits ==
  Matches(MissingVotes)

BugStakeSnapshotEmits ==
  Matches(StakeSnapshotUnavailable)

BugStakeQuorumEmits ==
  Matches(StakeQuorumMissing)

BugWrongKind ==
  Matches(BitmapLengthMismatch)

BugDropsCertificate ==
  Matches(BitmapLengthMismatch)

BugWrongCertificate ==
  Matches(BitmapLengthMismatch)

BugWrongReason ==
  Matches(BitmapLengthMismatch)

BugValidationSuccessEmitsEvidence ==
  Matches(Success)

BugValidationErrorReturnsOk ==
  Matches(SignerOutOfBounds)

BugValidationOkReturnsErr ==
  Matches(Success)

====
