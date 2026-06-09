---- MODULE SumeragiValidationRejectReasonLabelGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for validation-reject reason labels.

This slice pins `validation_reject_reason_label(...)` and
`vnext_validation_reject_reason_label(...)` from `main_loop.rs`. The validation
failure finalization gate samples representative rejection paths, while the
validation-reject status gate proves counter accounting for labels that it is
given. This companion gate fixes the complete classifier boundary between
prev-hash, prev-height, topology, execution, and stateless rejection buckets.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

HasCommittedTransactions == "HasCommittedTransactions"
EmptyBlock == "EmptyBlock"
DuplicateTransactions == "DuplicateTransactions"
SccpCommitmentRootMismatch == "SccpCommitmentRootMismatch"
PrevBlockHashMismatch == "PrevBlockHashMismatch"
PrevBlockHeightMismatch == "PrevBlockHeightMismatch"
MerkleRootMismatch == "MerkleRootMismatch"
ExecutionContextInvalid == "ExecutionContextInvalid"
TransactionAccept == "TransactionAccept"
TopologyMismatch == "TopologyMismatch"
SignatureVerification == "SignatureVerification"
InvalidGenesis == "InvalidGenesis"
BlockInThePast == "BlockInThePast"
BlockInTheFuture == "BlockInTheFuture"
TransactionInTheFuture == "TransactionInTheFuture"
ConfidentialFeaturesMismatch == "ConfidentialFeaturesMismatch"
ProofPolicyHashMismatch == "ProofPolicyHashMismatch"
PreviousRosterEvidenceInvalid == "PreviousRosterEvidenceInvalid"
DaShardCursor == "DaShardCursor"
AxtEnvelopeValidationFailed == "AxtEnvelopeValidationFailed"
NposEffectsInvalid == "NposEffectsInvalid"

ValidationCases == {
  HasCommittedTransactions,
  EmptyBlock,
  DuplicateTransactions,
  SccpCommitmentRootMismatch,
  PrevBlockHashMismatch,
  PrevBlockHeightMismatch,
  MerkleRootMismatch,
  ExecutionContextInvalid,
  TransactionAccept,
  TopologyMismatch,
  SignatureVerification,
  InvalidGenesis,
  BlockInThePast,
  BlockInTheFuture,
  TransactionInTheFuture,
  ConfidentialFeaturesMismatch,
  ProofPolicyHashMismatch,
  PreviousRosterEvidenceInvalid,
  DaShardCursor,
  AxtEnvelopeValidationFailed,
  NposEffectsInvalid
}

ExecutionCases == {
  HasCommittedTransactions,
  EmptyBlock,
  DuplicateTransactions,
  SccpCommitmentRootMismatch,
  MerkleRootMismatch,
  ExecutionContextInvalid,
  TransactionAccept,
  SignatureVerification,
  DaShardCursor,
  AxtEnvelopeValidationFailed
}

StatelessCases == {
  ConfidentialFeaturesMismatch,
  ProofPolicyHashMismatch,
  InvalidGenesis,
  BlockInThePast,
  BlockInTheFuture,
  TransactionInTheFuture,
  PreviousRosterEvidenceInvalid,
  NposEffectsInvalid
}

KnownVNextLabels == {
  "stateless",
  "execution",
  "prev_hash",
  "prev_height",
  "topology",
  "validation_roots_missing",
  "pending_block_invalid",
  "other_rejection"
}

ReasonLabels == {"stateless", "execution", "prev_hash", "prev_height", "topology"}

SpecValidationLabel(c) ==
  CASE c = PrevBlockHashMismatch -> "prev_hash"
    [] c = PrevBlockHeightMismatch -> "prev_height"
    [] c = TopologyMismatch -> "topology"
    [] c \in ExecutionCases -> "execution"
    [] c \in StatelessCases -> "stateless"
    [] OTHER -> "stateless"

ActualValidationLabel(c) ==
  CASE Bug = "prev_hash_as_stateless"
       /\ c = PrevBlockHashMismatch -> "stateless"
    [] Bug = "prev_height_as_execution"
       /\ c = PrevBlockHeightMismatch -> "execution"
    [] Bug = "topology_as_execution"
       /\ c = TopologyMismatch -> "execution"
    [] Bug = "committed_as_stateless"
       /\ c = HasCommittedTransactions -> "stateless"
    [] Bug = "empty_as_stateless"
       /\ c = EmptyBlock -> "stateless"
    [] Bug = "duplicate_as_stateless"
       /\ c = DuplicateTransactions -> "stateless"
    [] Bug = "sccp_as_stateless"
       /\ c = SccpCommitmentRootMismatch -> "stateless"
    [] Bug = "execution_context_as_stateless"
       /\ c = ExecutionContextInvalid -> "stateless"
    [] Bug = "transaction_accept_as_stateless"
       /\ c = TransactionAccept -> "stateless"
    [] Bug = "merkle_as_stateless"
       /\ c = MerkleRootMismatch -> "stateless"
    [] Bug = "signature_as_stateless"
       /\ c = SignatureVerification -> "stateless"
    [] Bug = "da_cursor_as_stateless"
       /\ c = DaShardCursor -> "stateless"
    [] Bug = "axt_as_stateless"
       /\ c = AxtEnvelopeValidationFailed -> "stateless"
    [] Bug = "confidential_as_execution"
       /\ c = ConfidentialFeaturesMismatch -> "execution"
    [] Bug = "proof_policy_as_execution"
       /\ c = ProofPolicyHashMismatch -> "execution"
    [] Bug = "invalid_genesis_as_execution"
       /\ c = InvalidGenesis -> "execution"
    [] Bug = "past_as_execution"
       /\ c = BlockInThePast -> "execution"
    [] Bug = "future_as_execution"
       /\ c = BlockInTheFuture -> "execution"
    [] Bug = "tx_future_as_execution"
       /\ c = TransactionInTheFuture -> "execution"
    [] Bug = "previous_roster_as_execution"
       /\ c = PreviousRosterEvidenceInvalid -> "execution"
    [] Bug = "npos_as_execution"
       /\ c = NposEffectsInvalid -> "execution"
    [] OTHER -> SpecValidationLabel(c)

SpecVNextLabel(label) ==
  CASE label \in {"stateless", "execution", "prev_hash", "prev_height",
                  "topology"} -> label
    [] label = "validation_roots_missing" -> "execution"
    [] label = "pending_block_invalid" -> "stateless"
    [] OTHER -> "stateless"

ActualVNextLabel(label) ==
  CASE Bug = "vnext_roots_missing_stateless"
       /\ label = "validation_roots_missing" -> "stateless"
    [] Bug = "vnext_pending_invalid_execution"
       /\ label = "pending_block_invalid" -> "execution"
    [] Bug = "vnext_unknown_execution"
       /\ label = "other_rejection" -> "execution"
    [] Bug = "vnext_prev_hash_stateless"
       /\ label = "prev_hash" -> "stateless"
    [] OTHER -> SpecVNextLabel(label)

BugSet == {
  "none",
  "prev_hash_as_stateless",
  "prev_height_as_execution",
  "topology_as_execution",
  "committed_as_stateless",
  "empty_as_stateless",
  "duplicate_as_stateless",
  "sccp_as_stateless",
  "execution_context_as_stateless",
  "transaction_accept_as_stateless",
  "merkle_as_stateless",
  "signature_as_stateless",
  "da_cursor_as_stateless",
  "axt_as_stateless",
  "confidential_as_execution",
  "proof_policy_as_execution",
  "invalid_genesis_as_execution",
  "past_as_execution",
  "future_as_execution",
  "tx_future_as_execution",
  "previous_roster_as_execution",
  "npos_as_execution",
  "vnext_roots_missing_stateless",
  "vnext_pending_invalid_execution",
  "vnext_unknown_execution",
  "vnext_prev_hash_stateless"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0
  /\ \A c \in ValidationCases: ActualValidationLabel(c) \in ReasonLabels
  /\ \A label \in KnownVNextLabels: ActualVNextLabel(label) \in ReasonLabels

ValidationLabelsExact ==
  \A c \in ValidationCases:
    ActualValidationLabel(c) = SpecValidationLabel(c)

VNextLabelsExact ==
  \A label \in KnownVNextLabels:
    ActualVNextLabel(label) = SpecVNextLabel(label)

DirectLabelsStable ==
  /\ ActualValidationLabel(PrevBlockHashMismatch) = "prev_hash"
  /\ ActualValidationLabel(PrevBlockHeightMismatch) = "prev_height"
  /\ ActualValidationLabel(TopologyMismatch) = "topology"

ExecutionCasesStable ==
  \A c \in ExecutionCases:
    ActualValidationLabel(c) = "execution"

StatelessCasesStable ==
  \A c \in StatelessCases:
    ActualValidationLabel(c) = "stateless"

VNextSpecialCasesStable ==
  /\ ActualVNextLabel("validation_roots_missing") = "execution"
  /\ ActualVNextLabel("pending_block_invalid") = "stateless"
  /\ ActualVNextLabel("other_rejection") = "stateless"
  /\ ActualVNextLabel("prev_hash") = "prev_hash"

DirectReasonLabelAnchors ==
  /\ ActualValidationLabel(PrevBlockHashMismatch) = "prev_hash"
  /\ ActualValidationLabel(PrevBlockHeightMismatch) = "prev_height"
  /\ ActualValidationLabel(TopologyMismatch) = "topology"

ExecutionBlockContentAnchors ==
  /\ ActualValidationLabel(HasCommittedTransactions) = "execution"
  /\ ActualValidationLabel(EmptyBlock) = "execution"
  /\ ActualValidationLabel(DuplicateTransactions) = "execution"
  /\ ActualValidationLabel(SccpCommitmentRootMismatch) = "execution"
  /\ ActualValidationLabel(MerkleRootMismatch) = "execution"

ExecutionRuntimeAnchors ==
  /\ ActualValidationLabel(ExecutionContextInvalid) = "execution"
  /\ ActualValidationLabel(TransactionAccept) = "execution"
  /\ ActualValidationLabel(SignatureVerification) = "execution"
  /\ ActualValidationLabel(DaShardCursor) = "execution"
  /\ ActualValidationLabel(AxtEnvelopeValidationFailed) = "execution"

StatelessPolicyAnchors ==
  /\ ActualValidationLabel(ConfidentialFeaturesMismatch) = "stateless"
  /\ ActualValidationLabel(ProofPolicyHashMismatch) = "stateless"
  /\ ActualValidationLabel(InvalidGenesis) = "stateless"
  /\ ActualValidationLabel(NposEffectsInvalid) = "stateless"

StatelessTemporalRosterAnchors ==
  /\ ActualValidationLabel(BlockInThePast) = "stateless"
  /\ ActualValidationLabel(BlockInTheFuture) = "stateless"
  /\ ActualValidationLabel(TransactionInTheFuture) = "stateless"
  /\ ActualValidationLabel(PreviousRosterEvidenceInvalid) = "stateless"

VNextPassThroughAnchors ==
  /\ ActualVNextLabel("stateless") = "stateless"
  /\ ActualVNextLabel("execution") = "execution"
  /\ ActualVNextLabel("prev_hash") = "prev_hash"
  /\ ActualVNextLabel("prev_height") = "prev_height"
  /\ ActualVNextLabel("topology") = "topology"

VNextNormalizationAnchors ==
  /\ ActualVNextLabel("validation_roots_missing") = "execution"
  /\ ActualVNextLabel("pending_block_invalid") = "stateless"
  /\ ActualVNextLabel("other_rejection") = "stateless"

ValidationRejectReasonLabelCoreSafety ==
  /\ ValidationLabelsExact
  /\ VNextLabelsExact
  /\ DirectLabelsStable
  /\ ExecutionCasesStable
  /\ StatelessCasesStable
  /\ VNextSpecialCasesStable
  /\ DirectReasonLabelAnchors
  /\ ExecutionBlockContentAnchors
  /\ ExecutionRuntimeAnchors
  /\ StatelessPolicyAnchors
  /\ StatelessTemporalRosterAnchors
  /\ VNextPassThroughAnchors
  /\ VNextNormalizationAnchors

SafetyFast ==
  ValidationRejectReasonLabelCoreSafety

====
