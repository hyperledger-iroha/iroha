---- MODULE SumeragiQcValidationReasonGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for the exact QC validation reason projection used by
`QcValidationError::telemetry_reason()`, `qc_validation_reason(...)`, and
`qc_validation_error_to_evidence(...)`.

Every QC validation error has a stable telemetry label. Hard failures also emit
`InvalidQc` evidence, and the emitted evidence must carry the same label as the
reason helper. Soft failures keep their label for telemetry but do not emit
evidence.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoEvidenceReason == "none"
WrongReason == "wrong_reason"

BitmapLengthMismatch == "BitmapLengthMismatch"
SignerOutOfBounds == "SignerOutOfBounds"
InsufficientSigners == "InsufficientSigners"
MissingVotes == "MissingVotes"
DuplicateSigners == "DuplicateSigners"
ModeTagMismatch == "ModeTagMismatch"
PhaseMismatch == "PhaseMismatch"
ValidatorSetMismatch == "ValidatorSetMismatch"
ViewMismatch == "ViewMismatch"
AggregateMismatch == "AggregateMismatch"
SubjectMismatch == "SubjectMismatch"
RootsMismatch == "RootsMismatch"
HighestQcMismatch == "HighestQcMismatch"
InvalidSignature == "InvalidSignature"
SignerMissingFromBlock == "SignerMissingFromBlock"
StakeSnapshotUnavailable == "StakeSnapshotUnavailable"
StakeQuorumMissing == "StakeQuorumMissing"

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

Cases == HardErrors \cup SoftErrors

ReasonLabels == {
  "bitmap_length_mismatch",
  "signer_out_of_bounds",
  "insufficient_signers",
  "missing_votes",
  "duplicate_signers",
  "mode_tag_mismatch",
  "phase_mismatch",
  "validator_set_mismatch",
  "view_mismatch",
  "aggregate_mismatch",
  "subject_mismatch",
  "roots_mismatch",
  "highest_qc_mismatch",
  "invalid_signature",
  "signer_missing_from_block",
  "stake_snapshot_unavailable",
  "stake_quorum_missing"
}

AllReasonValues == ReasonLabels \cup {NoEvidenceReason, WrongReason}

\* @type: Str => Str;
SpecReason(c) ==
  CASE c = BitmapLengthMismatch -> "bitmap_length_mismatch"
    [] c = SignerOutOfBounds -> "signer_out_of_bounds"
    [] c = InsufficientSigners -> "insufficient_signers"
    [] c = MissingVotes -> "missing_votes"
    [] c = DuplicateSigners -> "duplicate_signers"
    [] c = ModeTagMismatch -> "mode_tag_mismatch"
    [] c = PhaseMismatch -> "phase_mismatch"
    [] c = ValidatorSetMismatch -> "validator_set_mismatch"
    [] c = ViewMismatch -> "view_mismatch"
    [] c = AggregateMismatch -> "aggregate_mismatch"
    [] c = SubjectMismatch -> "subject_mismatch"
    [] c = RootsMismatch -> "roots_mismatch"
    [] c = HighestQcMismatch -> "highest_qc_mismatch"
    [] c = InvalidSignature -> "invalid_signature"
    [] c = SignerMissingFromBlock -> "signer_missing_from_block"
    [] c = StakeSnapshotUnavailable -> "stake_snapshot_unavailable"
    [] c = StakeQuorumMissing -> "stake_quorum_missing"
    [] OTHER -> WrongReason

\* @type: Str => Bool;
SpecEmits(c) ==
  c \in HardErrors

\* @type: Str => Bool;
ActualEmits(c) ==
  CASE Bug = "insufficient_signers_emits"
       /\ c = InsufficientSigners -> TRUE
    [] Bug = "missing_votes_emits"
       /\ c = MissingVotes -> TRUE
    [] Bug = "duplicate_signers_no_evidence"
       /\ c = DuplicateSigners -> FALSE
    [] Bug = "phase_no_evidence"
       /\ c = PhaseMismatch -> FALSE
    [] Bug = "aggregate_no_evidence"
       /\ c = AggregateMismatch -> FALSE
    [] Bug = "signer_missing_no_evidence"
       /\ c = SignerMissingFromBlock -> FALSE
    [] Bug = "stake_snapshot_emits"
       /\ c = StakeSnapshotUnavailable -> TRUE
    [] Bug = "stake_quorum_emits"
       /\ c = StakeQuorumMissing -> TRUE
    [] OTHER -> SpecEmits(c)

\* @type: Str => Str;
ActualReason(c) ==
  CASE Bug = "bitmap_length_label_wrong"
       /\ c = BitmapLengthMismatch -> WrongReason
    [] Bug = "signer_out_label_wrong"
       /\ c = SignerOutOfBounds -> WrongReason
    [] Bug = "mode_tag_label_wrong"
       /\ c = ModeTagMismatch -> WrongReason
    [] Bug = "validator_set_label_wrong"
       /\ c = ValidatorSetMismatch -> WrongReason
    [] Bug = "view_label_wrong"
       /\ c = ViewMismatch -> WrongReason
    [] Bug = "subject_label_wrong"
       /\ c = SubjectMismatch -> WrongReason
    [] Bug = "roots_label_wrong"
       /\ c = RootsMismatch -> WrongReason
    [] Bug = "highest_qc_label_wrong"
       /\ c = HighestQcMismatch -> WrongReason
    [] Bug = "invalid_signature_label_wrong"
       /\ c = InvalidSignature -> WrongReason
    [] OTHER -> SpecReason(c)

\* @type: Str => Str;
SpecEvidenceReason(c) ==
  IF SpecEmits(c) THEN SpecReason(c) ELSE NoEvidenceReason

\* @type: Str => Str;
ActualEvidenceReason(c) ==
  IF ActualEmits(c) THEN ActualReason(c) ELSE NoEvidenceReason

\* @type: Str => <<Str, Bool, Str>>;
SpecOutput(c) ==
  <<SpecReason(c), SpecEmits(c), SpecEvidenceReason(c)>>

\* @type: Str => <<Str, Bool, Str>>;
ActualOutput(c) ==
  <<ActualReason(c), ActualEmits(c), ActualEvidenceReason(c)>>

Matches(c) ==
  ActualOutput(c) = SpecOutput(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "bitmap_length_label_wrong",
       "signer_out_label_wrong",
       "insufficient_signers_emits",
       "missing_votes_emits",
       "duplicate_signers_no_evidence",
       "mode_tag_label_wrong",
       "phase_no_evidence",
       "validator_set_label_wrong",
       "view_label_wrong",
       "aggregate_no_evidence",
       "subject_label_wrong",
       "roots_label_wrong",
       "highest_qc_label_wrong",
       "invalid_signature_label_wrong",
       "signer_missing_no_evidence",
       "stake_snapshot_emits",
       "stake_quorum_emits"
     }
  /\ checked = 0
  /\ \A c \in Cases: SpecReason(c) \in ReasonLabels
  /\ \A c \in Cases: SpecEvidenceReason(c) \in AllReasonValues
  /\ \A c \in Cases: ActualReason(c) \in AllReasonValues
  /\ \A c \in Cases: ActualEvidenceReason(c) \in AllReasonValues

QcValidationReasonMatchesSpec ==
  \A c \in Cases: Matches(c)

QcValidationReasonExactness ==
  /\ QcValidationReasonMatchesSpec

QcValidationReasonCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ QcValidationReasonExactness

SafetyFast ==
  QcValidationReasonExactness

BugBitmapLengthLabelWrong ==
  Matches(BitmapLengthMismatch)

BugSignerOutLabelWrong ==
  Matches(SignerOutOfBounds)

BugInsufficientSignersEmits ==
  Matches(InsufficientSigners)

BugMissingVotesEmits ==
  Matches(MissingVotes)

BugDuplicateSignersNoEvidence ==
  Matches(DuplicateSigners)

BugModeTagLabelWrong ==
  Matches(ModeTagMismatch)

BugPhaseNoEvidence ==
  Matches(PhaseMismatch)

BugValidatorSetLabelWrong ==
  Matches(ValidatorSetMismatch)

BugViewLabelWrong ==
  Matches(ViewMismatch)

BugAggregateNoEvidence ==
  Matches(AggregateMismatch)

BugSubjectLabelWrong ==
  Matches(SubjectMismatch)

BugRootsLabelWrong ==
  Matches(RootsMismatch)

BugHighestQcLabelWrong ==
  Matches(HighestQcMismatch)

BugInvalidSignatureLabelWrong ==
  Matches(InvalidSignature)

BugSignerMissingNoEvidence ==
  Matches(SignerMissingFromBlock)

BugStakeSnapshotEmits ==
  Matches(StakeSnapshotUnavailable)

BugStakeQuorumEmits ==
  Matches(StakeQuorumMissing)

====
