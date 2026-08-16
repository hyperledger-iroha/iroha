//! Authoritative SoraFS proof-of-personhood issuer and registry records.
//!
//! Private credential bodies stay in holder/issuer storage. The ledger keeps only issuer-attested
//! credential and revocation-nonce commitments together with the exact signed public root and
//! revocation-list publications needed by verifiers.
use crate::account::AccountId;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;
/// First-release schema version for issuer policies.
pub const POP_ISSUER_POLICY_VERSION_V1: u16 = 1;
/// First-release schema version for credential commitment batches.
pub const POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1: u16 = 1;
/// Hard ceiling for credentials committed atomically in one publication.
pub const POP_CREDENTIAL_COMMITMENTS_MAX_V1: usize = 256;
/// Hard ceiling for revocations newly admitted by one publication.
pub const POP_REVOCATIONS_PER_PUBLICATION_MAX_V1: u32 = 4_096;
/// Maximum exact signed commitment-root payload size.
pub const POP_COMMITMENT_ROOT_PAYLOAD_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum exact signed revocation-list payload size.
pub const POP_REVOCATION_LIST_PAYLOAD_MAX_BYTES_V1: usize = 512 * 1024;
/// Maximum issuer identifier length in UTF-8 bytes.
pub const POP_ISSUER_ID_MAX_BYTES_V1: usize = 256;
/// Maximum credential lifetime allowed by first-release policy.
pub const POP_CREDENTIAL_LIFETIME_MAX_SECS_V1: u64 = 10 * 365 * 24 * 60 * 60;
/// Maximum future publication skew allowed by first-release policy.
pub const POP_PUBLICATION_CLOCK_SKEW_MAX_SECS_V1: u64 = 60 * 60;
/// Domain separator for issuer policy digests.
pub const POP_ISSUER_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.pop.issuer-policy.v1";
/// Domain separator for private revocation-nonce commitments.
pub const POP_REVOCATION_NONCE_COMMITMENT_DOMAIN_V1: &[u8] =
    b"sorafs.pop.revocation-nonce-commitment.v1";
/// Domain separator for exact signed credential payload commitments.
pub const POP_CREDENTIAL_PAYLOAD_COMMITMENT_DOMAIN_V1: &[u8] =
    b"sorafs.pop.credential-payload-commitment.v1";
/// Domain separator for registry audit-chain digests.
pub const POP_REGISTRY_AUDIT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-audit.v1";
/// Domain separator for exact registry operation payload digests.
pub const POP_REGISTRY_PAYLOAD_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-payload.v1";
/// Governance-controlled issuer identity and bounded admission policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopIssuerPolicyV1 {
    /// Schema version; must equal [`POP_ISSUER_POLICY_VERSION_V1`].
    pub version: u16,
    /// Monotonic policy revision beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding policy, absent only at revision one.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_policy_digest: Option<[u8; 32]>,
    /// Canonical public issuer label (`pop-issuer-*`).
    pub issuer_id: String,
    /// Exact universal account authorised to publish issuer state.
    pub issuer_account: AccountId,
    /// Raw Ed25519 public key required on signed public publications.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub issuer_public_key: [u8; 32],
    /// Maximum credentials accepted in one atomic commitment batch.
    pub max_credentials_per_batch: u16,
    /// Maximum new revocations accepted in one list publication.
    pub max_revocations_per_publication: u32,
    /// Maximum credential validity interval.
    pub max_credential_lifetime_secs: u64,
    /// Maximum tolerated future skew for signed publication timestamps.
    pub max_future_clock_skew_secs: u64,
    /// Whether issuer publications are paused.
    pub paused: bool,
}
impl PopIssuerPolicyV1 {
    /// Validate the complete first-release issuer policy.
    ///
    /// # Errors
    ///
    /// Returns an error when the version, revision chain, issuer identity or
    /// key, batch limits, credential lifetime, or clock skew is invalid.
    pub fn validate(&self) -> Result<(), PopIssuerPolicyValidationError> {
        if self.version != POP_ISSUER_POLICY_VERSION_V1 {
            return Err(PopIssuerPolicyValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.revision == 0 {
            return Err(PopIssuerPolicyValidationError::ZeroRevision);
        }
        match (self.revision, self.predecessor_policy_digest) {
            (1, None) => {}
            (1, Some(_)) => return Err(PopIssuerPolicyValidationError::UnexpectedPredecessor),
            (_, Some(digest)) if digest != [0; 32] => {}
            _ => return Err(PopIssuerPolicyValidationError::MissingPredecessor),
        }
        validate_issuer_id(&self.issuer_id)?;
        if self.issuer_public_key == [0; 32]
            || PublicKey::from_bytes(Algorithm::Ed25519, &self.issuer_public_key).is_err()
        {
            return Err(PopIssuerPolicyValidationError::InvalidIssuerPublicKey);
        }
        if !(1..=u16::try_from(POP_CREDENTIAL_COMMITMENTS_MAX_V1)
            .expect("credential batch ceiling fits u16"))
            .contains(&self.max_credentials_per_batch)
        {
            return Err(
                PopIssuerPolicyValidationError::InvalidCredentialBatchLimit {
                    found: self.max_credentials_per_batch,
                },
            );
        }
        if !(1..=POP_REVOCATIONS_PER_PUBLICATION_MAX_V1)
            .contains(&self.max_revocations_per_publication)
        {
            return Err(
                PopIssuerPolicyValidationError::InvalidRevocationBatchLimit {
                    found: self.max_revocations_per_publication,
                },
            );
        }
        if !(1..=POP_CREDENTIAL_LIFETIME_MAX_SECS_V1).contains(&self.max_credential_lifetime_secs) {
            return Err(PopIssuerPolicyValidationError::InvalidCredentialLifetime {
                found: self.max_credential_lifetime_secs,
            });
        }
        if self.max_future_clock_skew_secs > POP_PUBLICATION_CLOCK_SKEW_MAX_SECS_V1 {
            return Err(PopIssuerPolicyValidationError::InvalidClockSkew {
                found: self.max_future_clock_skew_secs,
            });
        }
        Ok(())
    }
    /// Compute the canonical domain-separated policy digest.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito cannot encode the policy.
    pub fn digest(&self) -> Result<[u8; 32], norito::Error> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(POP_ISSUER_POLICY_DIGEST_DOMAIN_V1);
        norito::core::write_canonical_to_writer(self, &mut Blake3Writer(&mut hasher))?;
        Ok(*hasher.finalize().as_bytes())
    }
}
struct Blake3Writer<'a>(&'a mut blake3::Hasher);
impl std::io::Write for Blake3Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn validate_issuer_id(issuer_id: &str) -> Result<(), PopIssuerPolicyValidationError> {
    const NON_PRODUCTION_MARKERS: &[&str] =
        &["demo", "dev", "local", "mock", "sandbox", "staging", "test"];
    if issuer_id.len() > POP_ISSUER_ID_MAX_BYTES_V1
        || !issuer_id.strip_prefix("pop-issuer-").is_some_and(|suffix| {
            !suffix.is_empty()
                && !suffix.starts_with('-')
                && !suffix.ends_with('-')
                && !suffix.contains("--")
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !suffix
                    .split('-')
                    .any(|segment| NON_PRODUCTION_MARKERS.contains(&segment))
        })
    {
        return Err(PopIssuerPolicyValidationError::InvalidIssuerId);
    }
    Ok(())
}
/// Issuer-policy validation failures.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PopIssuerPolicyValidationError {
    /// Unsupported schema version.
    #[error("unsupported PoP issuer policy version {found}")]
    UnsupportedVersion {
        /// Version supplied by the policy.
        found: u16,
    },
    /// Revision zero is invalid.
    #[error("PoP issuer policy revision must be non-zero")]
    ZeroRevision,
    /// Revision one unexpectedly carries a predecessor.
    #[error("PoP issuer policy revision one must not carry a predecessor")]
    UnexpectedPredecessor,
    /// A later revision lacks a non-zero predecessor digest.
    #[error("PoP issuer policy revisions after one require a non-zero predecessor")]
    MissingPredecessor,
    /// Issuer id is not a bounded canonical `pop-issuer-*` label.
    #[error("PoP issuer id must be a bounded canonical lowercase pop-issuer-* label")]
    InvalidIssuerId,
    /// Issuer key is inert or not a valid Ed25519 public key encoding.
    #[error("PoP issuer key must be a valid non-zero Ed25519 public key")]
    InvalidIssuerPublicKey,
    /// Credential batch limit is outside the first-release bound.
    #[error("invalid PoP credential batch limit {found}")]
    InvalidCredentialBatchLimit {
        /// Supplied limit.
        found: u16,
    },
    /// Revocation batch limit is outside the first-release bound.
    #[error("invalid PoP revocation batch limit {found}")]
    InvalidRevocationBatchLimit {
        /// Supplied limit.
        found: u32,
    },
    /// Credential lifetime is outside the first-release bound.
    #[error("invalid PoP maximum credential lifetime {found}")]
    InvalidCredentialLifetime {
        /// Supplied lifetime.
        found: u64,
    },
    /// Future skew is outside the first-release bound.
    #[error("invalid PoP publication clock skew {found}")]
    InvalidClockSkew {
        /// Supplied skew.
        found: u64,
    },
}
/// Payload-free issuer commitment to one private signed credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopCredentialCommitmentV1 {
    /// Domain-separated BLAKE3-256 commitment to exact canonical signed credential bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credential_commitment: [u8; 32],
    /// Domain-separated BLAKE3-256 commitment to the private revocation nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub revocation_nonce_commitment: [u8; 32],
    /// Public commitment root containing the credential.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commitment_root: [u8; 32],
    /// Commitment-tree version containing the credential.
    pub commitment_tree_version: u64,
    /// Revocation-list version observed at issuance.
    pub revocation_list_version: u64,
    /// Credential issuance epoch.
    pub issued_at_epoch: u64,
    /// Credential expiry epoch.
    pub expires_at_epoch: u64,
}
impl PopCredentialCommitmentV1 {
    /// Validate payload-free commitment invariants.
    ///
    /// # Errors
    ///
    /// Returns an error when a commitment is zero, a publication version is
    /// zero, or the credential validity window is invalid.
    pub fn validate(&self) -> Result<(), PopCredentialCommitmentValidationError> {
        if self.credential_commitment == [0; 32] {
            return Err(PopCredentialCommitmentValidationError::ZeroCredentialCommitment);
        }
        if self.revocation_nonce_commitment == [0; 32] {
            return Err(PopCredentialCommitmentValidationError::ZeroRevocationNonceCommitment);
        }
        if self.commitment_root == [0; 32] {
            return Err(PopCredentialCommitmentValidationError::ZeroCommitmentRoot);
        }
        if self.commitment_tree_version == 0 || self.revocation_list_version == 0 {
            return Err(PopCredentialCommitmentValidationError::ZeroPublicationVersion);
        }
        if self.issued_at_epoch == 0 || self.issued_at_epoch >= self.expires_at_epoch {
            return Err(PopCredentialCommitmentValidationError::InvalidValidityWindow);
        }
        Ok(())
    }
}
/// Credential-commitment validation failures.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PopCredentialCommitmentValidationError {
    /// Credential commitment is inert.
    #[error("PoP credential commitment must not be zero")]
    ZeroCredentialCommitment,
    /// Revocation-nonce commitment is inert.
    #[error("PoP revocation nonce commitment must not be zero")]
    ZeroRevocationNonceCommitment,
    /// Commitment root is inert.
    #[error("PoP credential commitment root must not be zero")]
    ZeroCommitmentRoot,
    /// A referenced publication version is zero.
    #[error("PoP credential publication versions must be non-zero")]
    ZeroPublicationVersion,
    /// Issuance/expiry interval is empty or inverted.
    #[error("PoP credential commitment validity window is invalid")]
    InvalidValidityWindow,
}
/// Atomic first-release credential commitment, root, and revocation snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopCredentialCommitmentBatchV1 {
    /// Schema version.
    pub version: u16,
    /// Exact active issuer-policy digest expected by the publisher.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub issuer_policy_digest: [u8; 32],
    /// Exact canonical signed `sorafs_manifest::PopCommitmentRootV1` bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub commitment_root_payload: Vec<u8>,
    /// Exact canonical signed `sorafs_manifest::PopRevocationListV1` bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub revocation_list_payload: Vec<u8>,
    /// Strictly credential-commitment-ordered payload-free issuer records.
    pub commitments: Vec<PopCredentialCommitmentV1>,
}
impl PopCredentialCommitmentBatchV1 {
    /// Validate hard resource bounds and canonical commitment ordering.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version, inert policy digest, invalid payload or batch
    /// bounds, invalid commitments, non-canonical ordering, or duplicate revocation commitments.
    pub fn validate(&self) -> Result<(), PopCredentialCommitmentBatchValidationError> {
        if self.version != POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1 {
            return Err(
                PopCredentialCommitmentBatchValidationError::UnsupportedVersion {
                    found: self.version,
                },
            );
        }
        if self.issuer_policy_digest == [0; 32] {
            return Err(PopCredentialCommitmentBatchValidationError::ZeroPolicyDigest);
        }
        if self.commitment_root_payload.is_empty()
            || self.commitment_root_payload.len() > POP_COMMITMENT_ROOT_PAYLOAD_MAX_BYTES_V1
        {
            return Err(
                PopCredentialCommitmentBatchValidationError::InvalidRootPayloadSize {
                    found: self.commitment_root_payload.len(),
                },
            );
        }
        if self.revocation_list_payload.is_empty()
            || self.revocation_list_payload.len() > POP_REVOCATION_LIST_PAYLOAD_MAX_BYTES_V1
        {
            return Err(
                PopCredentialCommitmentBatchValidationError::InvalidRevocationPayloadSize {
                    found: self.revocation_list_payload.len(),
                },
            );
        }
        if self.commitments.is_empty() || self.commitments.len() > POP_CREDENTIAL_COMMITMENTS_MAX_V1
        {
            return Err(
                PopCredentialCommitmentBatchValidationError::InvalidBatchSize {
                    found: self.commitments.len(),
                },
            );
        }
        let mut previous = None;
        let mut revocation_commitments = std::collections::BTreeSet::new();
        for commitment in &self.commitments {
            commitment.validate().map_err(|error| {
                PopCredentialCommitmentBatchValidationError::InvalidCommitment { error }
            })?;
            if previous.is_some_and(|value| value >= commitment.credential_commitment) {
                return Err(
                    PopCredentialCommitmentBatchValidationError::CommitmentsNotStrictlyOrdered,
                );
            }
            if !revocation_commitments.insert(commitment.revocation_nonce_commitment) {
                return Err(
                    PopCredentialCommitmentBatchValidationError::DuplicateRevocationCommitment,
                );
            }
            previous = Some(commitment.credential_commitment);
        }
        Ok(())
    }
}
/// Credential-batch validation failures.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PopCredentialCommitmentBatchValidationError {
    /// Unsupported batch schema version.
    #[error("unsupported PoP credential commitment batch version {found}")]
    UnsupportedVersion {
        /// Supplied version.
        found: u16,
    },
    /// Active policy digest binding is inert.
    #[error("PoP credential commitment batch policy digest must not be zero")]
    ZeroPolicyDigest,
    /// Signed root payload is empty or oversized.
    #[error("invalid PoP commitment-root payload size {found}")]
    InvalidRootPayloadSize {
        /// Supplied byte length.
        found: usize,
    },
    /// Signed revocation payload is empty or oversized.
    #[error("invalid PoP revocation-list payload size {found}")]
    InvalidRevocationPayloadSize {
        /// Supplied byte length.
        found: usize,
    },
    /// Batch is empty or oversized.
    #[error("invalid PoP credential commitment batch size {found}")]
    InvalidBatchSize {
        /// Supplied entry count.
        found: usize,
    },
    /// A commitment record is invalid.
    #[error("invalid PoP credential commitment: {error}")]
    InvalidCommitment {
        /// Underlying validation error.
        error: PopCredentialCommitmentValidationError,
    },
    /// Credential commitments are duplicated or not sorted.
    #[error("PoP credential commitments must be strictly ordered")]
    CommitmentsNotStrictlyOrdered,
    /// Two private nonce values map to the same advertised commitment.
    #[error("PoP credential batch contains a duplicate revocation-nonce commitment")]
    DuplicateRevocationCommitment,
}
/// Activated issuer policy with ledger provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopIssuerPolicyRecordV1 {
    /// Active policy.
    pub policy: PopIssuerPolicyV1,
    /// Canonical policy digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
    /// Activation block timestamp.
    pub activated_at_epoch: u64,
    /// Governance account that activated the policy.
    pub activated_by: AccountId,
    /// Audit-chain sequence of the activation.
    pub audit_sequence: u64,
    /// Audit-chain digest of the activation.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub audit_digest: [u8; 32],
}
/// Durable payload-free credential commitment record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopCredentialCommitmentRecordV1 {
    /// Issuer commitment body.
    pub commitment: PopCredentialCommitmentV1,
    /// Ledger timestamp at which the commitment became authoritative.
    pub committed_at_epoch: u64,
    /// Exact issuer account that committed the record.
    pub committed_by: AccountId,
    /// Issuer policy digest used for admission.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub admitted_policy_digest: [u8; 32],
    /// Audit-chain sequence of the atomic batch transition.
    pub audit_sequence: u64,
    /// Audit-chain digest of the atomic batch transition.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub audit_digest: [u8; 32],
}
/// Authoritative signed commitment-root publication record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopCommitmentRootRecordV1 {
    /// Root digest.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub root_digest: [u8; 32],
    /// Monotonic tree version.
    pub tree_version: u64,
    /// Number of leaves committed by the root.
    pub tree_size: u64,
    /// Exact canonical signed root publication bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub canonical_root_payload: Vec<u8>,
    /// Ledger timestamp at which the root became authoritative.
    pub recorded_at_epoch: u64,
    /// Exact issuer account that published the root.
    pub recorded_by: AccountId,
    /// Issuer policy digest used for admission.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub admitted_policy_digest: [u8; 32],
    /// Audit-chain sequence of the atomic publication.
    pub audit_sequence: u64,
    /// Audit-chain digest of the atomic publication.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub audit_digest: [u8; 32],
}
/// Authoritative signed revocation-list publication record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopRevocationPublicationRecordV1 {
    /// Monotonic list version.
    pub list_version: u64,
    /// Commitment root to which the list is bound.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commitment_root: [u8; 32],
    /// Sparse revocation root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub revocation_root: [u8; 32],
    /// Number of explicit revocation entries in the signed snapshot.
    pub entry_count: u32,
    /// Exact canonical signed revocation-list bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub canonical_revocation_list_payload: Vec<u8>,
    /// Ledger timestamp at which the list became authoritative.
    pub recorded_at_epoch: u64,
    /// Exact issuer account that published the list.
    pub recorded_by: AccountId,
    /// Issuer policy digest used for admission.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub admitted_policy_digest: [u8; 32],
    /// Audit-chain sequence of the atomic publication.
    pub audit_sequence: u64,
    /// Audit-chain digest of the atomic publication.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub audit_digest: [u8; 32],
}
/// Stable public reason recorded for a private nonce commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "reason", content = "value", rename_all = "snake_case")
)]
pub enum PopRegistryRevocationReasonV1 {
    /// Credential was rotated.
    Rotated,
    /// Holder requested withdrawal.
    HolderRequested,
    /// Enrollment evidence was invalidated.
    EnrollmentInvalid,
    /// Governance suspended the credential.
    GovernanceSuspension,
    /// Credential expired.
    Expired,
}
/// Durable payload-free revocation record keyed by nonce commitment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopRevocationRecordV1 {
    /// Domain-separated revocation-nonce commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub revocation_nonce_commitment: [u8; 32],
    /// Credential commitment bound to the revoked nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credential_commitment: [u8; 32],
    /// Signed list version that first introduced the revocation.
    pub list_version: u64,
    /// Commitment root to which that list was bound.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub commitment_root: [u8; 32],
    /// Issuer-authored revocation timestamp.
    pub revoked_at_epoch: u64,
    /// Stable reason code.
    pub reason: PopRegistryRevocationReasonV1,
    /// Ledger timestamp at which the revocation became authoritative.
    pub recorded_at_epoch: u64,
    /// Exact issuer account that published the revocation.
    pub recorded_by: AccountId,
    /// Issuer policy digest used for admission.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub admitted_policy_digest: [u8; 32],
    /// Audit-chain sequence of the list publication.
    pub audit_sequence: u64,
    /// Audit-chain digest of the list publication.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub audit_digest: [u8; 32],
}
/// Kind of transition committed into the registry audit chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum PopRegistryAuditEventKindV1 {
    /// Issuer policy was activated or rotated.
    PolicyActivated,
    /// Credential commitments and a new public root were committed atomically.
    CredentialBatchCommitted,
    /// A strict revocation-list extension was published.
    RevocationListPublished,
}
impl PopRegistryAuditEventKindV1 {
    /// Stable digest tag for the event kind.
    #[must_use]
    pub const fn digest_tag(self) -> u8 {
        match self {
            Self::PolicyActivated => 1,
            Self::CredentialBatchCommitted => 2,
            Self::RevocationListPublished => 3,
        }
    }
}
/// One link in the deterministic registry audit chain.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopRegistryAuditDigestRecordV1 {
    /// Monotonic event sequence beginning at one.
    pub sequence: u64,
    /// Transition kind.
    pub kind: PopRegistryAuditEventKindV1,
    /// Domain-separated digest of the exact canonical operation payload.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payload_digest: [u8; 32],
    /// Previous audit digest, absent only for sequence one.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub previous_audit_digest: Option<[u8; 32]>,
    /// Domain-separated digest of this audit link.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub audit_digest: [u8; 32],
    /// Ledger timestamp of the transition.
    pub recorded_at_epoch: u64,
    /// Account that authorised the transition.
    pub recorded_by: AccountId,
}
/// Constant-time authoritative registry counters and active anchors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PopRegistryStatusV1 {
    /// Active commitment root, absent before the first issuer batch.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub active_root_digest: Option<[u8; 32]>,
    /// Active commitment-tree version, or zero before first publication.
    pub active_tree_version: u64,
    /// Active revocation-list version, or zero before first publication.
    pub active_revocation_list_version: u64,
    /// Active sparse revocation root, absent before first publication.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub active_revocation_root: Option<[u8; 32]>,
    /// Total unique private credentials represented by payload-free commitments.
    pub credential_commitment_count: u64,
    /// Total unique revoked nonce commitments.
    pub revoked_credential_count: u64,
    /// Latest registry audit sequence.
    pub audit_sequence: u64,
    /// Latest registry audit digest.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub audit_head: Option<[u8; 32]>,
    /// Last authoritative ledger transition timestamp.
    pub updated_at_epoch: u64,
}
/// Commit a private nonce into the public registry without revealing it.
#[must_use]
pub fn pop_revocation_nonce_commitment_v1(nonce: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POP_REVOCATION_NONCE_COMMITMENT_DOMAIN_V1);
    hasher.update(&nonce);
    *hasher.finalize().as_bytes()
}
/// Commit exact canonical signed credential bytes without publishing them.
#[must_use]
pub fn pop_credential_payload_commitment_v1(canonical_credential: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POP_CREDENTIAL_PAYLOAD_COMMITMENT_DOMAIN_V1);
    hasher.update(canonical_credential);
    *hasher.finalize().as_bytes()
}
/// Digest an exact canonical registry operation payload.
#[must_use]
pub fn pop_registry_payload_digest_v1(payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POP_REGISTRY_PAYLOAD_DIGEST_DOMAIN_V1);
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    fn encode_with_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(value).expect("encode alternate-layout PoP registry value")
    }
    fn assert_canonical_norito_round_trip<T>(value: &T)
    where
        T: core::fmt::Debug + PartialEq + norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let encoded = norito::encode_canonical(value).expect("encode canonical PoP registry value");
        let decoded: T =
            norito::decode_canonical(&encoded).expect("decode canonical PoP registry value");
        assert_eq!(&decoded, value);
        let alternate = encode_with_alternate_norito_layout(value);
        assert_ne!(alternate, encoded);
        let alternate_decoded: T = norito::decode_from_bytes(&alternate)
            .expect("alternate-layout PoP registry value remains structurally decodable");
        assert_eq!(&alternate_decoded, value);
        assert!(matches!(
            norito::decode_canonical::<T>(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));
    }
    fn keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("valid test key")
    }
    fn account(seed: u8) -> AccountId {
        AccountId::new(keypair(seed).public_key().clone())
    }
    fn public_key_bytes(seed: u8) -> [u8; 32] {
        let keypair = keypair(seed);
        let (_, bytes) = keypair
            .public_key()
            .try_to_bytes()
            .expect("encode Ed25519 public key");
        bytes.try_into().expect("Ed25519 public key length")
    }
    fn policy() -> PopIssuerPolicyV1 {
        PopIssuerPolicyV1 {
            version: POP_ISSUER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issuer_account: account(7),
            issuer_public_key: public_key_bytes(7),
            max_credentials_per_batch: 16,
            max_revocations_per_publication: 32,
            max_credential_lifetime_secs: 365 * 24 * 60 * 60,
            max_future_clock_skew_secs: 30,
            paused: false,
        }
    }
    fn commitment(value: u8) -> PopCredentialCommitmentV1 {
        PopCredentialCommitmentV1 {
            credential_commitment: [value; 32],
            revocation_nonce_commitment: [value.wrapping_add(1); 32],
            commitment_root: [9; 32],
            commitment_tree_version: 1,
            revocation_list_version: 1,
            issued_at_epoch: 100,
            expires_at_epoch: 200,
        }
    }
    #[test]
    fn policy_validation_and_digest_are_deterministic() {
        let policy = policy();
        policy.validate().expect("valid policy");
        let digest = policy.digest().expect("digest policy");
        let canonical = norito::encode_canonical(&policy).expect("historical policy bytes");
        let mut historical = blake3::Hasher::new();
        historical.update(POP_ISSUER_POLICY_DIGEST_DOMAIN_V1);
        historical.update(&canonical);
        assert_eq!(digest, *historical.finalize().as_bytes());
        assert_eq!(digest, policy.digest().expect("repeat policy digest"));
        assert_canonical_norito_round_trip(&policy);
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let ambient_digest = policy
            .digest()
            .expect("digest policy under alternate ambient layout");
        drop(ambient);
        assert_eq!(
            ambient_digest, digest,
            "issuer-policy identity must ignore ambient Norito layout"
        );
        let mut invalid = policy.clone();
        invalid.issuer_id = "POP-ISSUER-NONCANONICAL".to_owned();
        assert_eq!(
            invalid.validate(),
            Err(PopIssuerPolicyValidationError::InvalidIssuerId)
        );
        invalid.issuer_id = "pop-issuer-dev".to_owned();
        assert_eq!(
            invalid.validate(),
            Err(PopIssuerPolicyValidationError::InvalidIssuerId)
        );
        invalid.issuer_id = policy.issuer_id;
        invalid.issuer_public_key = [0xFF; 32];
        assert_eq!(
            invalid.validate(),
            Err(PopIssuerPolicyValidationError::InvalidIssuerPublicKey)
        );
    }
    #[test]
    fn commitment_batch_rejects_duplicate_and_unsorted_entries() {
        let mut batch = PopCredentialCommitmentBatchV1 {
            version: POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
            issuer_policy_digest: [3; 32],
            commitment_root_payload: vec![1],
            revocation_list_payload: vec![2],
            commitments: vec![commitment(2), commitment(1)],
        };
        assert_eq!(
            batch.validate(),
            Err(PopCredentialCommitmentBatchValidationError::CommitmentsNotStrictlyOrdered)
        );
        batch.commitments = vec![commitment(1), commitment(2)];
        batch.commitments[1].revocation_nonce_commitment =
            batch.commitments[0].revocation_nonce_commitment;
        assert_eq!(
            batch.validate(),
            Err(PopCredentialCommitmentBatchValidationError::DuplicateRevocationCommitment)
        );
    }
    #[test]
    fn revocation_nonce_commitments_are_domain_separated_and_stable() {
        let nonce = [0xA5; 32];
        let commitment = pop_revocation_nonce_commitment_v1(nonce);
        assert_ne!(commitment, nonce);
        assert_eq!(commitment, pop_revocation_nonce_commitment_v1(nonce));
        assert_ne!(commitment, pop_revocation_nonce_commitment_v1([0xA4; 32]));
        let credential = pop_credential_payload_commitment_v1(b"canonical credential");
        assert_ne!(
            credential,
            *blake3::hash(b"canonical credential").as_bytes()
        );
        assert_eq!(
            credential,
            pop_credential_payload_commitment_v1(b"canonical credential")
        );
    }
    #[test]
    fn records_roundtrip_with_norito() {
        let record = PopRegistryStatusV1 {
            active_root_digest: Some([1; 32]),
            active_tree_version: 2,
            active_revocation_list_version: 3,
            active_revocation_root: Some([4; 32]),
            credential_commitment_count: 5,
            revoked_credential_count: 1,
            audit_sequence: 7,
            audit_head: Some([8; 32]),
            updated_at_epoch: 9,
        };
        assert_canonical_norito_round_trip(&record);
    }
}
