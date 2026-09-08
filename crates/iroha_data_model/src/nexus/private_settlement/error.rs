//! Structural validation errors shared by private-settlement records.

use thiserror::Error;

/// Structural validation failures for atomic private settlement wire objects.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PrivateSettlementValidationError {
    /// An unsupported wire version was supplied.
    #[error("unsupported atomic private settlement version {actual}")]
    UnsupportedVersion {
        /// Actual version.
        actual: u8,
    },
    /// Participant count is outside 2 through 255.
    #[error("atomic private settlement participant count {count} is outside 2..=255")]
    ParticipantCount {
        /// Actual participant count.
        count: usize,
    },
    /// Authority and expiry heights are invalid.
    #[error("atomic private settlement expiry must be after a non-zero authority context")]
    InvalidExpiry,
    /// Reimbursement leg ordinal is outside the participant set.
    #[error("atomic private settlement reimbursement leg is out of range")]
    InvalidReimbursementLeg,
    /// A leg ordinal differs from its canonical vector index.
    #[error("leg at index {index} has non-canonical ordinal {actual}")]
    NonCanonicalOrdinal {
        /// Vector index.
        index: usize,
        /// Supplied ordinal.
        actual: u8,
    },
    /// Routes are duplicated or not strictly ordered.
    #[error("private settlement routes must be strictly ordered and unique")]
    NonCanonicalRouteOrder,
    /// More than one leg routes to the same participant dataspace.
    #[error("private settlement permits exactly one leg per participant dataspace")]
    DuplicateDataspace,
    /// A cryptographic commitment or identifier used the reserved zero value.
    #[error("private settlement contains a reserved zero commitment")]
    ZeroCommitment,
    /// Stable bundle identifier does not match public intent material.
    #[error("private settlement bundle id does not match public intent material")]
    BundleIdMismatch,
    /// Exact public fee intent does not match its committed digest.
    #[error("private settlement public fee intent digest mismatch")]
    FeeIntentDigestMismatch,
    /// Exact public fee intent is structurally invalid.
    #[error("private settlement public fee intent is invalid")]
    InvalidFeeIntent,
    /// Canonical encoding unexpectedly failed.
    #[error("private settlement canonical encoding failed")]
    CanonicalEncoding,
    /// A root or key epoch is zero, non-contiguous, or otherwise invalid.
    #[error("private settlement epoch transition is invalid")]
    InvalidEpoch,
    /// Nullifiers or output commitments are duplicated.
    #[error("private settlement fixed state slots contain duplicates")]
    DuplicateStateItem,
    /// Fixed input/output vectors do not have the closed protocol lengths.
    #[error(
        "private settlement fixed slots require 2 nullifiers and 3 outputs, got {nullifiers} and {outputs}"
    )]
    InvalidFixedSlotCount {
        /// Actual nullifier count.
        nullifiers: usize,
        /// Actual output or encrypted-output count.
        outputs: usize,
    },
    /// Fixed delta fields differ from the proof statement.
    #[error("private settlement delta does not match its proof statement")]
    DeltaStatementMismatch,
    /// One encrypted output is malformed or misaligned.
    #[error("private settlement encrypted output {index} is invalid")]
    InvalidEncryptedOutput {
        /// Fixed output index.
        index: usize,
    },
    /// Restricted-DA certificate has an invalid body, quorum, or wire shape.
    #[error("private settlement restricted-DA availability certificate is invalid")]
    InvalidAvailabilityCertificate,
    /// One provisional availability share is malformed or unauthenticated.
    #[error("private settlement restricted-DA availability share is invalid")]
    InvalidAvailabilityShare,
    /// A node auditor-view attestation has malformed or reserved fields.
    #[error("private settlement auditor view attestation is invalid")]
    InvalidAuditorViewAttestation,
    /// A node approval-acknowledgement attestation is malformed or reserved.
    #[error("private settlement audit approval acknowledgement attestation is invalid")]
    InvalidAuditApprovalAcknowledgementAttestation,
    /// Auditor-only plaintext has an invalid fixed shape, value balance, or dummy slot.
    #[error("private settlement auditor plaintext is invalid")]
    InvalidAuditPlaintext,
    /// Auditor-only plaintext differs from its exact public manifest leg.
    #[error("private settlement auditor plaintext binding mismatch")]
    AuditPlaintextBindingMismatch,
    /// Designated reimbursement plaintext does not open the public terms commitment.
    #[error("private settlement reimbursement terms commitment mismatch")]
    ReimbursementTermsMismatch,
    /// Auditor policy lifecycle fields are invalid.
    #[error("private settlement audit policy lifecycle is invalid")]
    InvalidAuditPolicyLifecycle,
    /// Auditor threshold or roster size is invalid.
    #[error("private settlement audit threshold is invalid")]
    InvalidAuditThreshold,
    /// Auditors are duplicated or not strictly ordered.
    #[error("private settlement auditors must be strictly ordered and unique")]
    NonCanonicalAuditorOrder,
    /// Purpose-specific auditor keys are reused.
    #[error("private settlement auditor keys must be unique")]
    DuplicateAuditorKey,
    /// Hybrid public encryption key is malformed.
    #[error("private settlement auditor hybrid public key is invalid")]
    InvalidHybridPublicKey,
    /// Policy self-digest is invalid.
    #[error("private settlement audit policy digest mismatch")]
    AuditPolicyDigestMismatch,
    /// Restricted pool route is universal or has a reserved incarnation.
    #[error("private settlement pool governance route is invalid")]
    InvalidPoolGovernanceRoute,
    /// Restricted pool governance revision or activation interval is invalid.
    #[error("private settlement pool governance lifecycle is invalid")]
    InvalidPoolGovernanceLifecycle,
    /// A required pool, salt, commitment, policy, or key-epoch field is reserved.
    #[error("private settlement pool governance binding is invalid")]
    InvalidPoolGovernanceBinding,
    /// Exact route, pool, asset, or salt does not open the governed commitment.
    #[error("private settlement pool governance asset binding mismatch")]
    PoolGovernanceAssetBindingMismatch,
    /// Restricted pool mapping names the wrong audit policy or key epoch.
    #[error("private settlement pool governance audit policy mismatch")]
    PoolGovernancePolicyMismatch,
    /// Restricted pool governance self-digest is invalid.
    #[error("private settlement pool governance digest mismatch")]
    PoolGovernanceDigestMismatch,
    /// Restricted pool mapping or its audit policy is inactive at the requested height.
    #[error("private settlement pool governance mapping is stale")]
    StalePoolGovernance,
    /// Capsule authenticated data does not match its policy.
    #[error("private settlement audit capsule binding mismatch")]
    AuditCapsuleBindingMismatch,
    /// Capsule ciphertext does not match its fixed padding class.
    #[error("private settlement audit capsule ciphertext is invalid")]
    InvalidAuditCapsuleCiphertext,
    /// Capsule recipients differ from the exact policy roster.
    #[error("private settlement audit capsule recipients do not match policy")]
    AuditCapsuleRecipientMismatch,
    /// One wrapped DEK has malformed KEM or AEAD material.
    #[error("private settlement wrapped DEK is invalid")]
    InvalidWrappedDek,
    /// Proof bytes are empty or exceed the profile bound.
    #[error("private settlement proof size is invalid")]
    InvalidProofSize,
    /// Proof statement ordinal does not identify a manifest leg.
    #[error("private settlement payload references an unknown leg")]
    UnknownLeg,
    /// Restricted payload does not match the public manifest or local policy.
    #[error("private settlement payload does not match manifest")]
    ManifestPayloadMismatch,
    /// Capsule digest differs from the proof statement.
    #[error("private settlement audit capsule digest mismatch")]
    AuditCapsuleDigestMismatch,
    /// Proof-byte digest differs from the committed delta.
    #[error("private settlement proof digest mismatch")]
    ProofDigestMismatch,
    /// Sidecar ticket, retention, length, or certificate does not match.
    #[error("private settlement sidecar availability mismatch")]
    SidecarAvailabilityMismatch,
    /// Auditor approval is outside the policy or approval validity interval.
    #[error("private settlement audit approval is stale or policy-inconsistent")]
    StaleAuditApproval,
    /// Approval signer is not in the governed local policy.
    #[error("private settlement approval signer is not an authorized auditor")]
    UnauthorizedAuditor,
    /// Auditor signature verification failed.
    #[error("private settlement auditor signature is invalid")]
    InvalidAuditSignature,
    /// Approval set is duplicated or not strictly ordered.
    #[error("private settlement approvals must be strictly ordered and unique")]
    NonCanonicalApprovalOrder,
    /// Approval threshold was not met.
    #[error("private settlement has {actual} approvals but requires {required}")]
    InsufficientAuditApprovals {
        /// Actual approval count.
        actual: usize,
        /// Governed threshold.
        required: u8,
    },
    /// Approval body differs from the exact proof, capsule, delta, or roots.
    #[error("private settlement audit approval binding mismatch")]
    AuditApprovalBindingMismatch,
    /// Four-validator authority record is malformed.
    #[error("private settlement committee authority is invalid")]
    InvalidCommitteeAuthority,
    /// Compact authority catalog is malformed or non-canonical.
    #[error("private settlement authority catalog is invalid")]
    InvalidAuthorityCatalog,
    /// Phase certificate bitmap or signature shape is malformed.
    #[error("private settlement phase certificate is invalid")]
    InvalidPhaseCertificate,
    /// One participant phase vote is malformed.
    #[error("private settlement phase vote is invalid")]
    InvalidPhaseVote,
    /// The complete all-Prepare barrier is missing or misaligned.
    #[error("private settlement Prepare barrier is invalid")]
    InvalidPrepareBarrier,
    /// Receipt count, height, or canonical structure is invalid.
    #[error("private settlement receipt shape is invalid")]
    InvalidReceiptShape,
    /// Receipt leg, authority, delta, or phase body does not match the manifest.
    #[error("private settlement receipt binding mismatch")]
    ReceiptBindingMismatch,
    /// Public receipt exceeds the carrier byte budget.
    #[error("private settlement receipt size {bytes} exceeds the protocol limit")]
    ReceiptTooLarge {
        /// Actual canonical receipt size.
        bytes: usize,
    },
    /// Abort receipt is malformed.
    #[error("private settlement abort receipt is invalid")]
    InvalidAbortReceipt,
}
