//! Norito schemas for the SoraFS Pin Registry contract.
//!
//! The types defined here represent the on-chain/state-machine structures
//! driving manifest lifecycle, alias bindings, replication orders, and
//! governance policy snapshots. Validation helpers ensure the records obey
//! canonical encoding and governance constraints before they are persisted.

use std::collections::HashSet;

use blake3::Hasher;
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::{
    Error as NoritoError,
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
};
use thiserror::Error;

use crate::{
    CouncilSignature, PinPolicy, chunker_registry,
    validation::{ManifestValidationError, PinPolicyConstraints, validate_pin_policy},
};

/// Canonical pin registry record for a manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PinRecordV1 {
    /// IPLD/CID of the manifest (binary multibase form).
    pub manifest_cid: Vec<u8>,
    /// SHA3-256 digest of the ordered chunk plan emitted during build.
    pub chunk_plan_digest: [u8; 32],
    /// Merkle root of the Proof-of-Retrievability transcript.
    pub por_root: [u8; 32],
    /// Canonical chunker profile handle (`namespace.name@semver`).
    pub profile_handle: String,
    /// Epoch when the manifest was approved (inclusive).
    pub approved_at: u64,
    /// Epoch (inclusive) until which the manifest must remain pinned.
    pub retention_epoch: u64,
    /// Governance-enforced pin policy captured at approval time.
    pub pin_policy: PinPolicy,
    /// Optional predecessor manifest CID for succession chains.
    #[norito(default)]
    pub successor_of: Option<Vec<u8>>,
    /// Optional digest of the council envelope that approved the manifest.
    #[norito(default)]
    pub governance_envelope_hash: Option<[u8; 32]>,
}

impl PinRecordV1 {
    /// Validates structural invariants for the pin record.
    pub fn validate(&self) -> Result<(), PinRecordValidationError> {
        if self.manifest_cid.is_empty() {
            return Err(PinRecordValidationError::EmptyManifestCid);
        }
        if self.chunk_plan_digest.iter().all(|&byte| byte == 0) {
            return Err(PinRecordValidationError::InvalidChunkPlanDigest);
        }
        if self.por_root.iter().all(|&byte| byte == 0) {
            return Err(PinRecordValidationError::InvalidPorRoot);
        }
        let descriptor =
            chunker_registry::lookup_by_handle(&self.profile_handle).ok_or_else(|| {
                PinRecordValidationError::UnknownProfileHandle {
                    handle: self.profile_handle.clone(),
                }
            })?;
        let canonical = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if self.profile_handle != canonical {
            return Err(PinRecordValidationError::NonCanonicalProfileHandle {
                provided: self.profile_handle.clone(),
                canonical,
            });
        }
        if self.retention_epoch < self.approved_at {
            return Err(PinRecordValidationError::RetentionBeforeApproval {
                approved_at: self.approved_at,
                retention_epoch: self.retention_epoch,
            });
        }
        validate_pin_policy(&self.pin_policy, &PinPolicyConstraints::default())
            .map_err(PinRecordValidationError::InvalidPinPolicy)?;
        if self
            .successor_of
            .as_ref()
            .is_some_and(|parent| parent.is_empty())
        {
            return Err(PinRecordValidationError::InvalidSuccessorCid);
        }
        if let Some(hash) = self.governance_envelope_hash
            && hash.iter().all(|&byte| byte == 0)
        {
            return Err(PinRecordValidationError::InvalidGovernanceEnvelopeHash);
        }
        Ok(())
    }
}

/// Errors produced when a [`PinRecordV1`] fails validation.
#[derive(Debug, Error)]
pub enum PinRecordValidationError {
    #[error("manifest CID must not be empty")]
    EmptyManifestCid,
    #[error("chunk plan digest must be non-zero")]
    InvalidChunkPlanDigest,
    #[error("PoR root must be non-zero")]
    InvalidPorRoot,
    #[error("unknown chunker profile handle `{handle}`")]
    UnknownProfileHandle { handle: String },
    #[error("non-canonical profile handle `{provided}` (expected `{canonical}`)")]
    NonCanonicalProfileHandle { provided: String, canonical: String },
    #[error("retention epoch {retention_epoch} precedes approval epoch {approved_at}")]
    RetentionBeforeApproval {
        approved_at: u64,
        retention_epoch: u64,
    },
    #[error("pin policy invalid: {0}")]
    InvalidPinPolicy(ManifestValidationError),
    #[error("successor manifest CID must not be empty")]
    InvalidSuccessorCid,
    #[error("governance envelope hash must be non-zero")]
    InvalidGovernanceEnvelopeHash,
}

/// Alias binding that maps a human-friendly alias to a manifest CID.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct AliasBindingV1 {
    /// Alias identifier (`namespace/name` style, lower-case ASCII).
    pub alias: String,
    /// Manifest CID bound to the alias at `bound_at`.
    pub manifest_cid: Vec<u8>,
    /// Epoch when the alias binding became active.
    pub bound_at: u64,
    /// Epoch (inclusive) when the alias expires unless renewed.
    pub expiry_epoch: u64,
}

impl AliasBindingV1 {
    /// Checks alias syntax and lifetime ordering.
    pub fn validate(&self) -> Result<(), AliasBindingValidationError> {
        validate_alias(&self.alias)?;
        if self.manifest_cid.is_empty() {
            return Err(AliasBindingValidationError::EmptyManifestCid);
        }
        if self.expiry_epoch < self.bound_at {
            return Err(AliasBindingValidationError::ExpiryBeforeBound {
                bound_at: self.bound_at,
                expiry_epoch: self.expiry_epoch,
            });
        }
        Ok(())
    }
}

fn validate_alias(alias: &str) -> Result<(), AliasBindingValidationError> {
    let trimmed = alias.trim();
    if trimmed.is_empty() {
        return Err(AliasBindingValidationError::EmptyAlias);
    }
    if trimmed.len() > 128 {
        return Err(AliasBindingValidationError::AliasTooLong {
            length: trimmed.len(),
        });
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_' | '/'))
    {
        return Err(AliasBindingValidationError::InvalidAliasCharacters {
            alias: trimmed.to_owned(),
        });
    }
    if trimmed != alias {
        return Err(AliasBindingValidationError::AliasHasWhitespace);
    }
    Ok(())
}

/// Errors raised while validating [`AliasBindingV1`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AliasBindingValidationError {
    #[error("alias must not be empty")]
    EmptyAlias,
    #[error("alias length {length} exceeds maximum of 128 characters")]
    AliasTooLong { length: usize },
    #[error("alias contains invalid characters: `{alias}`")]
    InvalidAliasCharacters { alias: String },
    #[error("alias must not include surrounding whitespace")]
    AliasHasWhitespace,
    #[error("manifest CID must not be empty")]
    EmptyManifestCid,
    #[error("alias expiry {expiry_epoch} precedes binding epoch {bound_at}")]
    ExpiryBeforeBound { bound_at: u64, expiry_epoch: u64 },
}

/// Alias proof bundle propagated alongside SoraFS responses.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[norito(decode_from_slice)]
pub struct AliasProofBundleV1 {
    /// Alias binding being attested.
    pub binding: AliasBindingV1,
    /// Merkle root of the alias registry tree.
    pub registry_root: [u8; 32],
    /// Registry height (epoch) associated with the root.
    pub registry_height: u64,
    /// Unix timestamp (seconds) when the bundle was produced.
    pub generated_at_unix: u64,
    /// Unix timestamp (seconds) when the bundle expires.
    pub expires_at_unix: u64,
    /// Merkle path proving inclusion of the alias binding.
    #[norito(default)]
    pub merkle_path: Vec<[u8; 32]>,
    /// Optional governance signatures binding the registry root.
    #[norito(default)]
    pub council_signatures: Vec<CouncilSignature>,
}

impl AliasProofBundleV1 {
    /// Validates structural invariants for the bundle.
    pub fn validate(&self) -> Result<(), AliasProofBundleValidationError> {
        self.binding
            .validate()
            .map_err(AliasProofBundleValidationError::InvalidAliasBinding)?;

        if self.registry_root.iter().all(|&byte| byte == 0) {
            return Err(AliasProofBundleValidationError::EmptyRegistryRoot);
        }

        if self.expires_at_unix < self.generated_at_unix {
            return Err(AliasProofBundleValidationError::GeneratedAfterExpiry {
                generated_at_unix: self.generated_at_unix,
                expires_at_unix: self.expires_at_unix,
            });
        }

        for (index, signature) in self.council_signatures.iter().enumerate() {
            if signature.signer.iter().all(|&byte| byte == 0) {
                return Err(AliasProofBundleValidationError::EmptyCouncilSigner { index });
            }
            if signature.signature.is_empty() {
                return Err(AliasProofBundleValidationError::EmptyCouncilSignature { index });
            }
        }

        Ok(())
    }
}

/// Errors produced when an [`AliasProofBundleV1`] fails validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AliasProofBundleValidationError {
    #[error("alias binding invalid: {0}")]
    InvalidAliasBinding(AliasBindingValidationError),
    #[error("registry root must not be zero")]
    EmptyRegistryRoot,
    #[error("expires_at_unix {expires_at_unix} precedes generated_at_unix {generated_at_unix}")]
    GeneratedAfterExpiry {
        generated_at_unix: u64,
        expires_at_unix: u64,
    },
    #[error("council signature at index {index} has a zeroed signer identifier")]
    EmptyCouncilSigner { index: usize },
    #[error("council signature at index {index} must not be empty")]
    EmptyCouncilSignature { index: usize },
}

/// Domain separator applied when hashing alias leaves.
const ALIAS_LEAF_DOMAIN: &[u8] = b"sorafs:alias:leaf:v1";
/// Domain separator applied when hashing alias parent nodes.
const ALIAS_PARENT_DOMAIN: &[u8] = b"sorafs:alias:parent:v1";
/// Domain separator applied when deriving the signature digest.
const ALIAS_SIGNATURE_DOMAIN: &[u8] = b"sorafs:alias:root:v1";

/// Errors produced when verifying an [`AliasProofBundleV1`].
#[derive(Debug, Error)]
pub enum AliasProofVerificationError {
    #[error("alias proof bundle invalid: {0}")]
    Validation(#[from] AliasProofBundleValidationError),
    #[error("failed to encode alias binding: {0}")]
    EncodeAliasBinding(#[source] NoritoError),
    #[error("alias proof Merkle root mismatch: bundle {expected_hex}, computed {computed_hex}")]
    MerkleRootMismatch {
        /// Hex-encoded root supplied by the bundle.
        expected_hex: String,
        /// Hex-encoded root recomputed from the alias binding and path.
        computed_hex: String,
    },
    #[error("alias proof is missing council signatures")]
    MissingCouncilSignatures,
    #[error("alias proof signature at index {index} must contain 64 bytes (found {length})")]
    InvalidSignatureLength {
        /// Signature position inside the bundle.
        index: usize,
        /// Actual signature length encountered.
        length: usize,
    },
    #[error("alias proof signer at index {index} invalid ({reason})")]
    InvalidSignerPublicKey {
        /// Signature position inside the bundle.
        index: usize,
        /// Failure reason (includes signer identifier).
        reason: String,
    },
    #[error("alias proof signature verification failed at index {index}: {reason}")]
    SignatureVerificationFailed {
        /// Signature position inside the bundle.
        index: usize,
        /// Verification failure reason (includes signer identifier).
        reason: String,
    },
}

fn alias_binding_leaf_hash(binding: &AliasBindingV1) -> Result<[u8; 32], NoritoError> {
    let bytes = norito::to_bytes(binding)?;
    let mut hasher = Hasher::new();
    hasher.update(ALIAS_LEAF_DOMAIN);
    hasher.update(&bytes);
    let mut output = [0u8; 32];
    output.copy_from_slice(hasher.finalize().as_bytes());
    Ok(output)
}

fn merkle_parent_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (a, b) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    let mut hasher = Hasher::new();
    hasher.update(ALIAS_PARENT_DOMAIN);
    hasher.update(a);
    hasher.update(b);
    let mut output = [0u8; 32];
    output.copy_from_slice(hasher.finalize().as_bytes());
    output
}

fn recompute_merkle_root(
    binding: &AliasBindingV1,
    path: &[[u8; 32]],
) -> Result<[u8; 32], NoritoError> {
    let mut node = alias_binding_leaf_hash(binding)?;
    for sibling in path {
        node = merkle_parent_hash(&node, sibling);
    }
    Ok(node)
}

#[must_use]
fn alias_signature_message(bundle: &AliasProofBundleV1) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(ALIAS_SIGNATURE_DOMAIN);
    hasher.update(&bundle.registry_root);
    hasher.update(&bundle.registry_height.to_le_bytes());
    hasher.update(&bundle.generated_at_unix.to_le_bytes());
    hasher.update(&bundle.expires_at_unix.to_le_bytes());
    let mut output = [0u8; 32];
    output.copy_from_slice(hasher.finalize().as_bytes());
    output
}

/// Recompute the Merkle root for an alias binding and its Merkle path.
///
/// # Errors
///
/// Returns [`AliasProofVerificationError::EncodeAliasBinding`] when the alias
/// binding fails to encode via Norito.
pub fn alias_merkle_root(
    binding: &AliasBindingV1,
    path: &[[u8; 32]],
) -> Result<[u8; 32], AliasProofVerificationError> {
    recompute_merkle_root(binding, path).map_err(AliasProofVerificationError::EncodeAliasBinding)
}

/// Compute the digest that council members sign for an alias proof bundle.
#[must_use]
pub fn alias_proof_signature_digest(bundle: &AliasProofBundleV1) -> [u8; 32] {
    alias_signature_message(bundle)
}

/// Perform cryptographic verification of an alias proof bundle.
///
/// # Errors
///
/// Returns [`AliasProofVerificationError`] when the bundle fails structural
/// validation, the Merkle root does not match, or council signatures fail to
/// verify.
pub fn verify_alias_proof_bundle(
    bundle: &AliasProofBundleV1,
) -> Result<(), AliasProofVerificationError> {
    bundle.validate()?;

    let computed_root = alias_merkle_root(&bundle.binding, &bundle.merkle_path)?;
    if computed_root != bundle.registry_root {
        return Err(AliasProofVerificationError::MerkleRootMismatch {
            expected_hex: hex::encode(bundle.registry_root),
            computed_hex: hex::encode(computed_root),
        });
    }

    if bundle.council_signatures.is_empty() {
        return Err(AliasProofVerificationError::MissingCouncilSignatures);
    }

    let message = alias_signature_message(bundle);
    for (index, signature) in bundle.council_signatures.iter().enumerate() {
        if signature.signature.len() != 64 {
            return Err(AliasProofVerificationError::InvalidSignatureLength {
                index,
                length: signature.signature.len(),
            });
        }
        let signer_hex = hex::encode(signature.signer);
        let public_key =
            PublicKey::from_bytes(Algorithm::Ed25519, &signature.signer).map_err(|err| {
                AliasProofVerificationError::InvalidSignerPublicKey {
                    index,
                    reason: format!("signer {signer_hex}: {err}"),
                }
            })?;
        let sig = Signature::from_bytes(&signature.signature);
        sig.verify(&public_key, message.as_ref()).map_err(|err| {
            AliasProofVerificationError::SignatureVerificationFailed {
                index,
                reason: format!("signer {signer_hex}: {err}"),
            }
        })?;
    }

    Ok(())
}

/// Replication order emitted by the pin registry for storage providers.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ReplicationOrderV1 {
    /// Governance-controlled order identifier (BLAKE3-256 digest).
    pub order_id: [u8; 32],
    /// Manifest CID providers must pin.
    pub manifest_cid: Vec<u8>,
    /// Provider identifiers targeted by the order.
    pub providers: Vec<[u8; 32]>,
    /// Required redundancy (minimum number of successful providers).
    pub redundancy: u16,
    /// Unix timestamp (seconds) when the order expires.
    pub deadline: u64,
    /// Governance policy digest in effect when the order was issued.
    pub policy_hash: [u8; 32],
}

impl ReplicationOrderV1 {
    /// Validates order structure and policy references.
    pub fn validate(&self) -> Result<(), ReplicationOrderValidationError> {
        if self.order_id.iter().all(|&byte| byte == 0) {
            return Err(ReplicationOrderValidationError::InvalidOrderId);
        }
        if self.manifest_cid.is_empty() {
            return Err(ReplicationOrderValidationError::EmptyManifestCid);
        }
        if self.providers.is_empty() {
            return Err(ReplicationOrderValidationError::MissingProviders);
        }
        let mut seen: HashSet<[u8; 32]> = HashSet::new();
        for provider in &self.providers {
            if provider.iter().all(|&byte| byte == 0) {
                return Err(ReplicationOrderValidationError::InvalidProviderId);
            }
            if !seen.insert(*provider) {
                return Err(ReplicationOrderValidationError::DuplicateProvider {
                    provider_id: *provider,
                });
            }
        }
        if self.redundancy == 0 {
            return Err(ReplicationOrderValidationError::ZeroRedundancy);
        }
        if usize::from(self.redundancy) > self.providers.len() {
            return Err(
                ReplicationOrderValidationError::RedundancyExceedsProviders {
                    redundancy: self.redundancy,
                    providers: self.providers.len() as u16,
                },
            );
        }
        if self.deadline == 0 {
            return Err(ReplicationOrderValidationError::InvalidDeadline);
        }
        if self.policy_hash.iter().all(|&byte| byte == 0) {
            return Err(ReplicationOrderValidationError::InvalidPolicyHash);
        }
        Ok(())
    }
}

/// Errors produced when a [`ReplicationOrderV1`] fails validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationOrderValidationError {
    #[error("order identifier must not be zero")]
    InvalidOrderId,
    #[error("manifest CID must not be empty")]
    EmptyManifestCid,
    #[error("replication order must target at least one provider")]
    MissingProviders,
    #[error("provider identifier must not be zero")]
    InvalidProviderId,
    #[error("duplicate provider entry in replication order")]
    DuplicateProvider { provider_id: [u8; 32] },
    #[error("redundancy must be at least one")]
    ZeroRedundancy,
    #[error("redundancy target {redundancy} exceeds provider list ({providers})")]
    RedundancyExceedsProviders { redundancy: u16, providers: u16 },
    #[error("deadline must be a positive unix timestamp")]
    InvalidDeadline,
    #[error("policy hash must be non-zero")]
    InvalidPolicyHash,
}

/// Provider acknowledgement for a replication order.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ReplicationReceiptV1 {
    /// Order identifier being acknowledged.
    pub order_id: [u8; 32],
    /// Provider responding to the order.
    pub provider_id: [u8; 32],
    /// Reported status for the replication attempt.
    pub status: ReplicationReceiptStatus,
    /// Unix timestamp (seconds) when the status was recorded.
    pub timestamp: u64,
    /// Optional digest of the PoR sample bundle.
    #[norito(default)]
    pub por_sample_digest: Option<[u8; 32]>,
}

impl ReplicationReceiptV1 {
    /// Validates structural invariants for the receipt.
    pub fn validate(&self) -> Result<(), ReplicationReceiptValidationError> {
        if self.order_id.iter().all(|&byte| byte == 0) {
            return Err(ReplicationReceiptValidationError::InvalidOrderId);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(ReplicationReceiptValidationError::InvalidProviderId);
        }
        if self.timestamp == 0 {
            return Err(ReplicationReceiptValidationError::InvalidTimestamp);
        }
        if matches!(self.status, ReplicationReceiptStatus::Completed)
            && self.por_sample_digest.is_none()
        {
            return Err(ReplicationReceiptValidationError::MissingPorSampleDigest);
        }
        if let Some(digest) = self.por_sample_digest
            && digest.iter().all(|&byte| byte == 0)
        {
            return Err(ReplicationReceiptValidationError::InvalidPorSampleDigest);
        }
        Ok(())
    }
}

/// Receipt status outcomes reported by providers.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum ReplicationReceiptStatus {
    /// Provider accepted the order and is processing ingestion.
    Accepted = 1,
    /// Provider completed ingestion and PoR sampling.
    Completed = 2,
    /// Provider rejected the order (capacity issues, policy mismatch, etc.).
    Rejected = 3,
}

/// Errors produced when a [`ReplicationReceiptV1`] fails validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationReceiptValidationError {
    #[error("order identifier must not be zero")]
    InvalidOrderId,
    #[error("provider identifier must not be zero")]
    InvalidProviderId,
    #[error("timestamp must be a positive unix timestamp")]
    InvalidTimestamp,
    #[error("completed receipts must include a PoR sample digest")]
    MissingPorSampleDigest,
    #[error("PoR sample digest must be non-zero when present")]
    InvalidPorSampleDigest,
}

/// Governance policy snapshot associated with the pin registry.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ManifestPolicyV1 {
    /// Minimum replica count required for approval.
    pub min_replicas: u16,
    /// Optional retention window cap enforced by governance.
    #[norito(default)]
    pub max_retention_epochs: Option<u64>,
    /// Allowed chunker profile handles for manifests.
    pub allowed_profiles: Vec<String>,
    /// Fee charged for pinning, expressed in basis points.
    pub pin_fee_basis_points: u16,
}

impl ManifestPolicyV1 {
    /// Validates governance policy structure and constraints.
    pub fn validate(&self) -> Result<(), ManifestPolicyValidationError> {
        if self.min_replicas == 0 {
            return Err(ManifestPolicyValidationError::ZeroReplicaMinimum);
        }
        if let Some(max_retention) = self.max_retention_epochs
            && max_retention == 0
        {
            return Err(ManifestPolicyValidationError::InvalidMaxRetention);
        }
        if self.allowed_profiles.is_empty() {
            return Err(ManifestPolicyValidationError::EmptyAllowedProfiles);
        }
        for handle in &self.allowed_profiles {
            let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
                ManifestPolicyValidationError::UnknownProfileHandle {
                    handle: handle.clone(),
                }
            })?;
            let canonical = format!(
                "{}.{}@{}",
                descriptor.namespace, descriptor.name, descriptor.semver
            );
            if handle != &canonical {
                return Err(ManifestPolicyValidationError::NonCanonicalProfileHandle {
                    provided: handle.clone(),
                    canonical,
                });
            }
        }
        if self.pin_fee_basis_points > 10_000 {
            return Err(ManifestPolicyValidationError::FeeOutOfRange {
                basis_points: self.pin_fee_basis_points,
            });
        }
        Ok(())
    }
}

/// Errors produced when a [`ManifestPolicyV1`] fails validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestPolicyValidationError {
    #[error("minimum replica count must be >= 1")]
    ZeroReplicaMinimum,
    #[error("max retention epoch must be > 0 when specified")]
    InvalidMaxRetention,
    #[error("allowed profile list must not be empty")]
    EmptyAllowedProfiles,
    #[error("unknown chunker profile handle `{handle}`")]
    UnknownProfileHandle { handle: String },
    #[error("non-canonical profile handle `{provided}` (expected `{canonical}`)")]
    NonCanonicalProfileHandle { provided: String, canonical: String },
    #[error("pin fee {basis_points} basis points exceeds 100%")]
    FeeOutOfRange { basis_points: u16 },
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
    use norito::{decode_from_bytes, to_bytes};

    use super::*;

    fn sample_alias_binding() -> AliasBindingV1 {
        AliasBindingV1 {
            alias: "docs/main".into(),
            manifest_cid: b"bafyexamplecid".to_vec(),
            bound_at: 1_700_000_000,
            expiry_epoch: 1_700_086_400,
        }
    }

    fn sample_pin_policy() -> PinPolicy {
        PinPolicy {
            min_replicas: 3,
            storage_class: crate::StorageClass::Hot,
            retention_epoch: 1_700_086_400,
        }
    }

    fn sample_pin_record() -> PinRecordV1 {
        PinRecordV1 {
            manifest_cid: b"bafyexamplecid".to_vec(),
            chunk_plan_digest: [0x11; 32],
            por_root: [0x22; 32],
            profile_handle: "sorafs.sf1@1.0.0".to_owned(),
            approved_at: 1_700_000_000,
            retention_epoch: 1_700_172_800,
            pin_policy: sample_pin_policy(),
            successor_of: None,
            governance_envelope_hash: Some([0x33; 32]),
        }
    }

    #[test]
    fn pin_record_roundtrip_and_validate() {
        let record = sample_pin_record();
        record.validate().expect("valid pin record");
        let bytes = to_bytes(&record).expect("encode");
        let decoded: PinRecordV1 = decode_from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, record);
    }

    #[test]
    fn pin_record_rejects_empty_cid() {
        let mut record = sample_pin_record();
        record.manifest_cid.clear();
        let err = record.validate().unwrap_err();
        assert!(matches!(err, PinRecordValidationError::EmptyManifestCid));
    }

    #[test]
    fn alias_binding_validation() {
        let binding = sample_alias_binding();
        binding.validate().expect("valid alias binding");
    }

    #[test]
    fn alias_binding_rejects_whitespace() {
        let binding = AliasBindingV1 {
            alias: " docs ".into(),
            manifest_cid: b"cid".to_vec(),
            bound_at: 1,
            expiry_epoch: 2,
        };
        let err = binding.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasBindingValidationError::AliasHasWhitespace
        ));
    }

    fn sample_alias_proof_bundle() -> AliasProofBundleV1 {
        AliasProofBundleV1 {
            binding: sample_alias_binding(),
            registry_root: [0xAA; 32],
            registry_height: 42,
            generated_at_unix: 1_700_000_010,
            expires_at_unix: 1_700_000_610,
            merkle_path: vec![[0xBB; 32], [0xCC; 32]],
            council_signatures: vec![crate::CouncilSignature {
                signer: [0xDD; 32],
                signature: vec![0x01, 0x02, 0x03],
            }],
        }
    }

    fn signed_alias_proof_bundle() -> (AliasProofBundleV1, KeyPair) {
        let mut bundle = AliasProofBundleV1 {
            binding: sample_alias_binding(),
            registry_root: [0u8; 32],
            registry_height: 42,
            generated_at_unix: 1_700_000_010,
            expires_at_unix: 1_700_000_610,
            merkle_path: vec![[0xBB; 32], [0xCC; 32]],
            council_signatures: Vec::new(),
        };
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x11; 32]).expect("seeded key");
        let keypair = KeyPair::from_private_key(private).expect("derive keypair");
        let root =
            alias_merkle_root(&bundle.binding, &bundle.merkle_path).expect("compute merkle root");
        bundle.registry_root = root;
        let digest = alias_proof_signature_digest(&bundle);
        let signature = Signature::new(keypair.private_key(), digest.as_ref());
        let signer_vec = keypair.public_key().to_bytes().1;
        let signer: [u8; 32] = signer_vec
            .try_into()
            .expect("ed25519 public key must be 32 bytes");
        bundle.council_signatures.push(crate::CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        });
        (bundle, keypair)
    }

    #[test]
    fn alias_proof_bundle_validate() {
        let bundle = sample_alias_proof_bundle();
        bundle.validate().expect("valid proof bundle");
    }

    #[test]
    fn alias_proof_bundle_rejects_zero_root() {
        let mut bundle = sample_alias_proof_bundle();
        bundle.registry_root = [0u8; 32];
        let err = bundle.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasProofBundleValidationError::EmptyRegistryRoot
        ));
    }

    #[test]
    fn alias_proof_bundle_rejects_generated_after_expiry() {
        let mut bundle = sample_alias_proof_bundle();
        bundle.expires_at_unix = bundle.generated_at_unix - 1;
        let err = bundle.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasProofBundleValidationError::GeneratedAfterExpiry { .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_rejects_empty_council_signature() {
        let mut bundle = sample_alias_proof_bundle();
        bundle.council_signatures[0].signature.clear();
        let err = bundle.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasProofBundleValidationError::EmptyCouncilSignature { .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_verification_succeeds() {
        let (bundle, _) = signed_alias_proof_bundle();
        verify_alias_proof_bundle(&bundle).expect("bundle must verify");
    }

    #[test]
    fn alias_proof_bundle_verification_rejects_root_mismatch() {
        let (mut bundle, _) = signed_alias_proof_bundle();
        bundle.registry_root[0] ^= 0xFF;
        let err = verify_alias_proof_bundle(&bundle).expect_err("verification must fail");
        assert!(matches!(
            err,
            AliasProofVerificationError::MerkleRootMismatch { .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_verification_rejects_bad_signature() {
        let (mut bundle, _) = signed_alias_proof_bundle();
        bundle.council_signatures[0].signature[0] ^= 0xFF;
        let err = verify_alias_proof_bundle(&bundle).expect_err("verification must fail");
        assert!(matches!(
            err,
            AliasProofVerificationError::SignatureVerificationFailed { .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_rejects_zero_council_signer() {
        let mut bundle = sample_alias_proof_bundle();
        bundle.council_signatures[0].signer = [0u8; 32];
        let err = bundle.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasProofBundleValidationError::EmptyCouncilSigner { .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_rewraps_alias_binding_error() {
        let mut bundle = sample_alias_proof_bundle();
        bundle.binding.alias = " invalid ".into();
        let err = bundle.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasProofBundleValidationError::InvalidAliasBinding(
                AliasBindingValidationError::AliasHasWhitespace
            )
        ));
    }

    fn sample_replication_order() -> ReplicationOrderV1 {
        ReplicationOrderV1 {
            order_id: [0x44; 32],
            manifest_cid: b"bafyexamplecid".to_vec(),
            providers: vec![[0x55; 32], [0x66; 32], [0x77; 32]],
            redundancy: 2,
            deadline: 1_700_090_000,
            policy_hash: [0x88; 32],
        }
    }

    #[test]
    fn replication_order_validate() {
        let order = sample_replication_order();
        order.validate().expect("valid order");
    }

    #[test]
    fn replication_order_rejects_duplicate_provider() {
        let mut order = sample_replication_order();
        order.providers[1] = order.providers[0];
        let err = order.validate().unwrap_err();
        assert!(matches!(
            err,
            ReplicationOrderValidationError::DuplicateProvider { .. }
        ));
    }

    #[test]
    fn receipt_requires_digest_for_completed() {
        let mut receipt = ReplicationReceiptV1 {
            order_id: [0x44; 32],
            provider_id: [0x55; 32],
            status: ReplicationReceiptStatus::Completed,
            timestamp: 1_700_000_123,
            por_sample_digest: Some([0x99; 32]),
        };
        receipt.validate().expect("valid receipt");
        receipt.por_sample_digest = None;
        let err = receipt.validate().unwrap_err();
        assert_eq!(
            err,
            ReplicationReceiptValidationError::MissingPorSampleDigest
        );
    }

    #[test]
    fn manifest_policy_validate() {
        let policy = ManifestPolicyV1 {
            min_replicas: 2,
            max_retention_epochs: Some(250_000),
            allowed_profiles: vec!["sorafs.sf1@1.0.0".to_owned()],
            pin_fee_basis_points: 250,
        };
        policy.validate().expect("valid policy");
    }

    #[test]
    fn manifest_policy_rejects_unknown_profile() {
        let policy = ManifestPolicyV1 {
            min_replicas: 1,
            max_retention_epochs: None,
            allowed_profiles: vec!["unknown.profile@1.2.3".to_owned()],
            pin_fee_basis_points: 100,
        };
        let err = policy.validate().unwrap_err();
        assert!(matches!(
            err,
            ManifestPolicyValidationError::UnknownProfileHandle { .. }
        ));
    }
}
