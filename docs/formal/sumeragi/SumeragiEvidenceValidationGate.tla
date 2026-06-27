---- MODULE SumeragiEvidenceValidationGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for `validate_evidence(...)`.

This slice pins the evidence validation result lattice:
- evidence kind must match the payload variant before payload-specific checks,
- double-vote evidence checks signature presence/length, phase shape, height,
  epoch, signer identity through the view topology, block/root conflict,
  evidence-kind agreement, and finally cryptographic vote signatures,
- invalid-proposal evidence must advance beyond the referenced QC height and
  must name the QC subject as its parent, while proposal view numbers reset per
  height and are intentionally ignored,
- censorship evidence requires non-empty receipts, matching transaction hashes,
  validator signers, valid receipt signatures, and an f + 1 unique-signer
  quorum; duplicate receipts cannot inflate quorum.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Ok == "Ok"
KindPayloadMismatch == "KindPayloadMismatch"
PhaseMismatch == "PhaseMismatch"
HeightMismatch == "HeightMismatch"
EpochMismatch == "EpochMismatch"
SignerMismatch == "SignerMismatch"
BlockHashMatch == "BlockHashMatch"
PhaseKindMismatch == "PhaseKindMismatch"
SignatureMissing == "SignatureMissing"
SignatureTruncated == "SignatureTruncated"
SignatureInvalid == "SignatureInvalid"
InvalidProposalHeight == "InvalidProposalHeight"
InvalidProposalParentMismatch == "InvalidProposalParentMismatch"
ReceiptMissing == "ReceiptMissing"
ReceiptTxHashMismatch == "ReceiptTxHashMismatch"
ReceiptSignerOutOfTopology == "ReceiptSignerOutOfTopology"
ReceiptSignatureInvalid == "ReceiptSignatureInvalid"
ReceiptQuorumMissing == "ReceiptQuorumMissing"

KindCases == {
  "invalid_qc_ok",
  "kind_mismatch_double_kind_invalid_qc_payload",
  "kind_mismatch_invalid_qc_kind_double_payload",
  "kind_mismatch_censorship_kind_proposal_payload"
}

DoubleCases == {
  "double_prepare_valid",
  "double_commit_block_valid",
  "double_commit_root_valid",
  "double_cross_phase_valid",
  "double_missing_signature",
  "double_truncated_signature",
  "double_bad_phase_pair",
  "double_height_mismatch",
  "double_epoch_mismatch",
  "double_signer_mismatch",
  "double_same_hash_prepare",
  "double_same_hash_commit_same_roots",
  "double_prepare_kind_for_commit",
  "double_commit_kind_for_prepare",
  "double_signature_invalid",
  "double_missing_signature_precedes_height",
  "double_truncated_signature_precedes_phase",
  "double_phase_precedes_height",
  "double_height_precedes_epoch",
  "double_epoch_precedes_signer",
  "double_signer_precedes_block"
}

ProposalCases == {
  "proposal_valid",
  "proposal_equal_height",
  "proposal_lower_height",
  "proposal_parent_mismatch",
  "proposal_height_parent_precedence",
  "proposal_view_reset_ignored"
}

CensorshipCases == {
  "censorship_valid_exact_quorum",
  "censorship_valid_extra_duplicate",
  "censorship_empty",
  "censorship_tx_mismatch",
  "censorship_signer_out",
  "censorship_bad_signature",
  "censorship_duplicate_below_quorum",
  "censorship_two_unique_below_quorum",
  "censorship_tx_mismatch_precedes_quorum",
  "censorship_outsider_precedes_signature",
  "censorship_signature_precedes_quorum"
}

AllCases == KindCases \cup DoubleCases \cup ProposalCases \cup CensorshipCases

SpecValidate(c) ==
  CASE c \in {
         "invalid_qc_ok",
         "double_prepare_valid",
         "double_commit_block_valid",
         "double_commit_root_valid",
         "double_cross_phase_valid",
         "proposal_valid",
         "proposal_view_reset_ignored",
         "censorship_valid_exact_quorum",
         "censorship_valid_extra_duplicate"
       } -> Ok
    [] c \in {
         "kind_mismatch_double_kind_invalid_qc_payload",
         "kind_mismatch_invalid_qc_kind_double_payload",
         "kind_mismatch_censorship_kind_proposal_payload"
       } -> KindPayloadMismatch
    [] c = "double_missing_signature" -> SignatureMissing
    [] c = "double_truncated_signature" -> SignatureTruncated
    [] c = "double_bad_phase_pair" -> PhaseMismatch
    [] c = "double_height_mismatch" -> HeightMismatch
    [] c = "double_epoch_mismatch" -> EpochMismatch
    [] c = "double_signer_mismatch" -> SignerMismatch
    [] c \in {
         "double_same_hash_prepare",
         "double_same_hash_commit_same_roots"
       } -> BlockHashMatch
    [] c \in {
         "double_prepare_kind_for_commit",
         "double_commit_kind_for_prepare"
       } -> PhaseKindMismatch
    [] c = "double_signature_invalid" -> SignatureInvalid
    [] c = "double_missing_signature_precedes_height" -> SignatureMissing
    [] c = "double_truncated_signature_precedes_phase" -> SignatureTruncated
    [] c = "double_phase_precedes_height" -> PhaseMismatch
    [] c = "double_height_precedes_epoch" -> HeightMismatch
    [] c = "double_epoch_precedes_signer" -> EpochMismatch
    [] c = "double_signer_precedes_block" -> SignerMismatch
    [] c \in {
         "proposal_equal_height",
         "proposal_lower_height",
         "proposal_height_parent_precedence"
       } -> InvalidProposalHeight
    [] c = "proposal_parent_mismatch" -> InvalidProposalParentMismatch
    [] c = "censorship_empty" -> ReceiptMissing
    [] c \in {
         "censorship_tx_mismatch",
         "censorship_tx_mismatch_precedes_quorum"
       } -> ReceiptTxHashMismatch
    [] c \in {
         "censorship_signer_out",
         "censorship_outsider_precedes_signature"
       } -> ReceiptSignerOutOfTopology
    [] c \in {
         "censorship_bad_signature",
         "censorship_signature_precedes_quorum"
       } -> ReceiptSignatureInvalid
    [] c \in {
         "censorship_duplicate_below_quorum",
         "censorship_two_unique_below_quorum"
       } -> ReceiptQuorumMissing
    [] OTHER -> "Unknown"

ActualValidate(c) ==
  CASE Bug = "kind_mismatch_accepted"
       /\ c = "kind_mismatch_double_kind_invalid_qc_payload" -> Ok
    [] Bug = "invalid_qc_rejected"
       /\ c = "invalid_qc_ok" -> KindPayloadMismatch
    [] Bug = "double_prepare_rejected"
       /\ c = "double_prepare_valid" -> SignatureInvalid
    [] Bug = "double_commit_block_rejected"
       /\ c = "double_commit_block_valid" -> SignatureInvalid
    [] Bug = "double_commit_root_rejected"
       /\ c = "double_commit_root_valid" -> BlockHashMatch
    [] Bug = "double_cross_phase_rejected"
       /\ c = "double_cross_phase_valid" -> PhaseMismatch
    [] Bug = "double_missing_signature_accepted"
       /\ c = "double_missing_signature" -> Ok
    [] Bug = "double_truncated_signature_accepted"
       /\ c = "double_truncated_signature" -> Ok
    [] Bug = "double_bad_phase_accepted"
       /\ c = "double_bad_phase_pair" -> Ok
    [] Bug = "double_height_mismatch_accepted"
       /\ c = "double_height_mismatch" -> Ok
    [] Bug = "double_epoch_mismatch_accepted"
       /\ c = "double_epoch_mismatch" -> Ok
    [] Bug = "double_signer_mismatch_accepted"
       /\ c = "double_signer_mismatch" -> Ok
    [] Bug = "double_same_hash_prepare_accepted"
       /\ c = "double_same_hash_prepare" -> Ok
    [] Bug = "double_same_hash_commit_roots_accepted"
       /\ c = "double_same_hash_commit_same_roots" -> Ok
    [] Bug = "double_prepare_kind_for_commit_accepted"
       /\ c = "double_prepare_kind_for_commit" -> Ok
    [] Bug = "double_commit_kind_for_prepare_accepted"
       /\ c = "double_commit_kind_for_prepare" -> Ok
    [] Bug = "double_invalid_signature_accepted"
       /\ c = "double_signature_invalid" -> Ok
    [] Bug = "double_missing_precedence_uses_height"
       /\ c = "double_missing_signature_precedes_height" -> HeightMismatch
    [] Bug = "double_truncated_precedence_uses_phase"
       /\ c = "double_truncated_signature_precedes_phase" -> PhaseMismatch
    [] Bug = "double_phase_precedence_uses_height"
       /\ c = "double_phase_precedes_height" -> HeightMismatch
    [] Bug = "double_height_precedence_uses_epoch"
       /\ c = "double_height_precedes_epoch" -> EpochMismatch
    [] Bug = "double_epoch_precedence_uses_signer"
       /\ c = "double_epoch_precedes_signer" -> SignerMismatch
    [] Bug = "double_signer_precedence_uses_block"
       /\ c = "double_signer_precedes_block" -> BlockHashMatch
    [] Bug = "proposal_equal_height_accepted"
       /\ c = "proposal_equal_height" -> Ok
    [] Bug = "proposal_lower_height_accepted"
       /\ c = "proposal_lower_height" -> Ok
    [] Bug = "proposal_parent_mismatch_accepted"
       /\ c = "proposal_parent_mismatch" -> Ok
    [] Bug = "proposal_height_parent_precedence_parent"
       /\ c = "proposal_height_parent_precedence" -> InvalidProposalParentMismatch
    [] Bug = "proposal_view_reset_rejected"
       /\ c = "proposal_view_reset_ignored" -> InvalidProposalHeight
    [] Bug = "censorship_empty_accepted"
       /\ c = "censorship_empty" -> Ok
    [] Bug = "censorship_tx_mismatch_accepted"
       /\ c = "censorship_tx_mismatch" -> Ok
    [] Bug = "censorship_signer_out_accepted"
       /\ c = "censorship_signer_out" -> Ok
    [] Bug = "censorship_bad_signature_accepted"
       /\ c = "censorship_bad_signature" -> Ok
    [] Bug = "censorship_duplicate_quorum_counted"
       /\ c = "censorship_duplicate_below_quorum" -> Ok
    [] Bug = "censorship_below_quorum_accepted"
       /\ c = "censorship_two_unique_below_quorum" -> Ok
    [] Bug = "censorship_exact_quorum_rejected"
       /\ c = "censorship_valid_exact_quorum" -> ReceiptQuorumMissing
    [] Bug = "censorship_extra_duplicate_rejected"
       /\ c = "censorship_valid_extra_duplicate" -> ReceiptQuorumMissing
    [] Bug = "censorship_tx_precedence_uses_quorum"
       /\ c = "censorship_tx_mismatch_precedes_quorum" -> ReceiptQuorumMissing
    [] Bug = "censorship_outsider_precedence_uses_signature"
       /\ c = "censorship_outsider_precedes_signature" -> ReceiptSignatureInvalid
    [] Bug = "censorship_signature_precedence_uses_quorum"
       /\ c = "censorship_signature_precedes_quorum" -> ReceiptQuorumMissing
    [] OTHER -> SpecValidate(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "kind_mismatch_accepted",
       "invalid_qc_rejected",
       "double_prepare_rejected",
       "double_commit_block_rejected",
       "double_commit_root_rejected",
       "double_cross_phase_rejected",
       "double_missing_signature_accepted",
       "double_truncated_signature_accepted",
       "double_bad_phase_accepted",
       "double_height_mismatch_accepted",
       "double_epoch_mismatch_accepted",
       "double_signer_mismatch_accepted",
       "double_same_hash_prepare_accepted",
       "double_same_hash_commit_roots_accepted",
       "double_prepare_kind_for_commit_accepted",
       "double_commit_kind_for_prepare_accepted",
       "double_invalid_signature_accepted",
       "double_missing_precedence_uses_height",
       "double_truncated_precedence_uses_phase",
       "double_phase_precedence_uses_height",
       "double_height_precedence_uses_epoch",
       "double_epoch_precedence_uses_signer",
       "double_signer_precedence_uses_block",
       "proposal_equal_height_accepted",
       "proposal_lower_height_accepted",
       "proposal_parent_mismatch_accepted",
       "proposal_height_parent_precedence_parent",
       "proposal_view_reset_rejected",
       "censorship_empty_accepted",
       "censorship_tx_mismatch_accepted",
       "censorship_signer_out_accepted",
       "censorship_bad_signature_accepted",
       "censorship_duplicate_quorum_counted",
       "censorship_below_quorum_accepted",
       "censorship_exact_quorum_rejected",
       "censorship_extra_duplicate_rejected",
       "censorship_tx_precedence_uses_quorum",
       "censorship_outsider_precedence_uses_signature",
       "censorship_signature_precedence_uses_quorum"
     }
  /\ checked = 0

Matches(c) ==
  ActualValidate(c) = SpecValidate(c)

EvidenceValidationMatchesSpec ==
  /\ Matches("invalid_qc_ok")
  /\ Matches("kind_mismatch_double_kind_invalid_qc_payload")
  /\ Matches("kind_mismatch_invalid_qc_kind_double_payload")
  /\ Matches("kind_mismatch_censorship_kind_proposal_payload")
  /\ Matches("double_prepare_valid")
  /\ Matches("double_commit_block_valid")
  /\ Matches("double_commit_root_valid")
  /\ Matches("double_cross_phase_valid")
  /\ Matches("double_missing_signature")
  /\ Matches("double_truncated_signature")
  /\ Matches("double_bad_phase_pair")
  /\ Matches("double_height_mismatch")
  /\ Matches("double_epoch_mismatch")
  /\ Matches("double_signer_mismatch")
  /\ Matches("double_same_hash_prepare")
  /\ Matches("double_same_hash_commit_same_roots")
  /\ Matches("double_prepare_kind_for_commit")
  /\ Matches("double_commit_kind_for_prepare")
  /\ Matches("double_signature_invalid")
  /\ Matches("double_missing_signature_precedes_height")
  /\ Matches("double_truncated_signature_precedes_phase")
  /\ Matches("double_phase_precedes_height")
  /\ Matches("double_height_precedes_epoch")
  /\ Matches("double_epoch_precedes_signer")
  /\ Matches("double_signer_precedes_block")
  /\ Matches("proposal_valid")
  /\ Matches("proposal_equal_height")
  /\ Matches("proposal_lower_height")
  /\ Matches("proposal_parent_mismatch")
  /\ Matches("proposal_height_parent_precedence")
  /\ Matches("proposal_view_reset_ignored")
  /\ Matches("censorship_valid_exact_quorum")
  /\ Matches("censorship_valid_extra_duplicate")
  /\ Matches("censorship_empty")
  /\ Matches("censorship_tx_mismatch")
  /\ Matches("censorship_signer_out")
  /\ Matches("censorship_bad_signature")
  /\ Matches("censorship_duplicate_below_quorum")
  /\ Matches("censorship_two_unique_below_quorum")
  /\ Matches("censorship_tx_mismatch_precedes_quorum")
  /\ Matches("censorship_outsider_precedes_signature")
  /\ Matches("censorship_signature_precedes_quorum")

EvidenceValidationExactness ==
  EvidenceValidationMatchesSpec

EvidenceValidationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EvidenceValidationExactness

SafetyFast ==
  EvidenceValidationExactness

BugKindMismatchAccepted ==
  ActualValidate("kind_mismatch_double_kind_invalid_qc_payload") =
    SpecValidate("kind_mismatch_double_kind_invalid_qc_payload")

BugInvalidQcRejected ==
  ActualValidate("invalid_qc_ok") = SpecValidate("invalid_qc_ok")

BugDoublePrepareRejected ==
  ActualValidate("double_prepare_valid") = SpecValidate("double_prepare_valid")

BugDoubleCommitBlockRejected ==
  ActualValidate("double_commit_block_valid") = SpecValidate("double_commit_block_valid")

BugDoubleCommitRootRejected ==
  ActualValidate("double_commit_root_valid") = SpecValidate("double_commit_root_valid")

BugDoubleCrossPhaseRejected ==
  ActualValidate("double_cross_phase_valid") = SpecValidate("double_cross_phase_valid")

BugDoubleMissingSignatureAccepted ==
  ActualValidate("double_missing_signature") = SpecValidate("double_missing_signature")

BugDoubleTruncatedSignatureAccepted ==
  ActualValidate("double_truncated_signature") = SpecValidate("double_truncated_signature")

BugDoubleBadPhaseAccepted ==
  ActualValidate("double_bad_phase_pair") = SpecValidate("double_bad_phase_pair")

BugDoubleHeightMismatchAccepted ==
  ActualValidate("double_height_mismatch") = SpecValidate("double_height_mismatch")

BugDoubleEpochMismatchAccepted ==
  ActualValidate("double_epoch_mismatch") = SpecValidate("double_epoch_mismatch")

BugDoubleSignerMismatchAccepted ==
  ActualValidate("double_signer_mismatch") = SpecValidate("double_signer_mismatch")

BugDoubleSameHashPrepareAccepted ==
  ActualValidate("double_same_hash_prepare") = SpecValidate("double_same_hash_prepare")

BugDoubleSameHashCommitRootsAccepted ==
  ActualValidate("double_same_hash_commit_same_roots") =
    SpecValidate("double_same_hash_commit_same_roots")

BugDoublePrepareKindForCommitAccepted ==
  ActualValidate("double_prepare_kind_for_commit") =
    SpecValidate("double_prepare_kind_for_commit")

BugDoubleCommitKindForPrepareAccepted ==
  ActualValidate("double_commit_kind_for_prepare") =
    SpecValidate("double_commit_kind_for_prepare")

BugDoubleInvalidSignatureAccepted ==
  ActualValidate("double_signature_invalid") = SpecValidate("double_signature_invalid")

BugDoubleMissingPrecedenceUsesHeight ==
  ActualValidate("double_missing_signature_precedes_height") =
    SpecValidate("double_missing_signature_precedes_height")

BugDoubleTruncatedPrecedenceUsesPhase ==
  ActualValidate("double_truncated_signature_precedes_phase") =
    SpecValidate("double_truncated_signature_precedes_phase")

BugDoublePhasePrecedenceUsesHeight ==
  ActualValidate("double_phase_precedes_height") = SpecValidate("double_phase_precedes_height")

BugDoubleHeightPrecedenceUsesEpoch ==
  ActualValidate("double_height_precedes_epoch") = SpecValidate("double_height_precedes_epoch")

BugDoubleEpochPrecedenceUsesSigner ==
  ActualValidate("double_epoch_precedes_signer") = SpecValidate("double_epoch_precedes_signer")

BugDoubleSignerPrecedenceUsesBlock ==
  ActualValidate("double_signer_precedes_block") = SpecValidate("double_signer_precedes_block")

BugProposalEqualHeightAccepted ==
  ActualValidate("proposal_equal_height") = SpecValidate("proposal_equal_height")

BugProposalLowerHeightAccepted ==
  ActualValidate("proposal_lower_height") = SpecValidate("proposal_lower_height")

BugProposalParentMismatchAccepted ==
  ActualValidate("proposal_parent_mismatch") = SpecValidate("proposal_parent_mismatch")

BugProposalHeightParentPrecedenceParent ==
  ActualValidate("proposal_height_parent_precedence") =
    SpecValidate("proposal_height_parent_precedence")

BugProposalViewResetRejected ==
  ActualValidate("proposal_view_reset_ignored") = SpecValidate("proposal_view_reset_ignored")

BugCensorshipEmptyAccepted ==
  ActualValidate("censorship_empty") = SpecValidate("censorship_empty")

BugCensorshipTxMismatchAccepted ==
  ActualValidate("censorship_tx_mismatch") = SpecValidate("censorship_tx_mismatch")

BugCensorshipSignerOutAccepted ==
  ActualValidate("censorship_signer_out") = SpecValidate("censorship_signer_out")

BugCensorshipBadSignatureAccepted ==
  ActualValidate("censorship_bad_signature") = SpecValidate("censorship_bad_signature")

BugCensorshipDuplicateQuorumCounted ==
  ActualValidate("censorship_duplicate_below_quorum") =
    SpecValidate("censorship_duplicate_below_quorum")

BugCensorshipBelowQuorumAccepted ==
  ActualValidate("censorship_two_unique_below_quorum") =
    SpecValidate("censorship_two_unique_below_quorum")

BugCensorshipExactQuorumRejected ==
  ActualValidate("censorship_valid_exact_quorum") = SpecValidate("censorship_valid_exact_quorum")

BugCensorshipExtraDuplicateRejected ==
  ActualValidate("censorship_valid_extra_duplicate") =
    SpecValidate("censorship_valid_extra_duplicate")

BugCensorshipTxPrecedenceUsesQuorum ==
  ActualValidate("censorship_tx_mismatch_precedes_quorum") =
    SpecValidate("censorship_tx_mismatch_precedes_quorum")

BugCensorshipOutsiderPrecedenceUsesSignature ==
  ActualValidate("censorship_outsider_precedes_signature") =
    SpecValidate("censorship_outsider_precedes_signature")

BugCensorshipSignaturePrecedenceUsesQuorum ==
  ActualValidate("censorship_signature_precedes_quorum") =
    SpecValidate("censorship_signature_precedes_quorum")

====
