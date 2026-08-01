//! Norito schemas for the SoraFS Pin Registry contract.
//!
//! The types defined here represent the on-chain/state-machine structures
//! driving manifest lifecycle, alias bindings, replication orders, and
//! governance policy snapshots. Validation helpers ensure the records obey
//! canonical encoding and governance constraints before they are persisted.

use blake3::Hasher;
use norito::{
    Error as NoritoError,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use thiserror::Error;

use crate::{
    BLAKE3_256_MULTIHASH_CODE, CouncilSignature, chunker_registry,
    provider_admission::{
        ProviderAdmissionCouncilPolicy, ProviderAdmissionSignatureError,
        verify_council_signatures_over_digest, verify_council_signatures_without_trust,
    },
    validation::{ManifestValidationError, validate_manifest_root_cid},
};

fn validate_first_release_manifest_cid(cid: &[u8]) -> Result<(), ManifestValidationError> {
    validate_manifest_root_cid(
        cid,
        chunker_registry::MANIFEST_DAG_CODEC,
        BLAKE3_256_MULTIHASH_CODE,
    )
}

/// Alias binding that maps a human-friendly alias to a manifest CID.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct AliasBindingV1 {
    /// Alias identifier (`namespace/name` or account-style `name@domain` lower-case ASCII).
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
        validate_first_release_manifest_cid(&self.manifest_cid).map_err(|error| {
            AliasBindingValidationError::MalformedManifestCid {
                reason: error.to_string(),
            }
        })?;
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
    if !trimmed.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_' | '/' | '@')
    }) {
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
    #[error("manifest CID is not canonical first-release CIDv1: {reason}")]
    MalformedManifestCid { reason: String },
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

/// Maximum encoded alias proof accepted by first-release consumers.
pub const MAX_ALIAS_PROOF_ENCODED_BYTES: usize = 1024 * 1024;
/// Maximum Merkle-tree height represented by an alias inclusion proof.
pub const MAX_ALIAS_PROOF_MERKLE_DEPTH: usize = 64;
/// Maximum distinct council signatures carried by one alias proof.
pub const MAX_ALIAS_PROOF_COUNCIL_SIGNATURES: usize = 64;

impl AliasProofBundleV1 {
    /// Validates structural invariants for the bundle.
    pub fn validate(&self) -> Result<(), AliasProofBundleValidationError> {
        self.binding
            .validate()
            .map_err(AliasProofBundleValidationError::InvalidAliasBinding)?;

        if self.registry_root.iter().all(|&byte| byte == 0) {
            return Err(AliasProofBundleValidationError::EmptyRegistryRoot);
        }

        if self.generated_at_unix == 0 {
            return Err(AliasProofBundleValidationError::InvalidGeneratedAt);
        }

        if self.expires_at_unix <= self.generated_at_unix {
            return Err(AliasProofBundleValidationError::GeneratedAfterExpiry {
                generated_at_unix: self.generated_at_unix,
                expires_at_unix: self.expires_at_unix,
            });
        }

        if self.merkle_path.len() > MAX_ALIAS_PROOF_MERKLE_DEPTH {
            return Err(AliasProofBundleValidationError::MerklePathTooDeep {
                found: self.merkle_path.len(),
                maximum: MAX_ALIAS_PROOF_MERKLE_DEPTH,
            });
        }

        if self.council_signatures.len() > MAX_ALIAS_PROOF_COUNCIL_SIGNATURES {
            return Err(AliasProofBundleValidationError::TooManyCouncilSignatures {
                found: self.council_signatures.len(),
                maximum: MAX_ALIAS_PROOF_COUNCIL_SIGNATURES,
            });
        }

        let mut previous_signer = None;
        for (index, signature) in self.council_signatures.iter().enumerate() {
            if signature.signer.iter().all(|&byte| byte == 0) {
                return Err(AliasProofBundleValidationError::EmptyCouncilSigner { index });
            }
            if previous_signer.is_some_and(|previous| previous >= signature.signer) {
                return Err(
                    AliasProofBundleValidationError::NonCanonicalCouncilSignerOrder { index },
                );
            }
            previous_signer = Some(signature.signer);
            if signature.signature.len() != 64 {
                return Err(
                    AliasProofBundleValidationError::InvalidCouncilSignatureLength {
                        index,
                        found: signature.signature.len(),
                    },
                );
            }
            if crate::inert_bytes(&signature.signature) {
                return Err(AliasProofBundleValidationError::InertCouncilSignature { index });
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
    #[error("generated_at_unix must be positive")]
    InvalidGeneratedAt,
    #[error(
        "expires_at_unix {expires_at_unix} must be greater than generated_at_unix {generated_at_unix}"
    )]
    GeneratedAfterExpiry {
        generated_at_unix: u64,
        expires_at_unix: u64,
    },
    #[error("alias proof Merkle path has {found} entries; maximum is {maximum}")]
    MerklePathTooDeep { found: usize, maximum: usize },
    #[error("alias proof has {found} council signatures; maximum is {maximum}")]
    TooManyCouncilSignatures { found: usize, maximum: usize },
    #[error("council signature at index {index} has a zeroed signer identifier")]
    EmptyCouncilSigner { index: usize },
    #[error("council signers must be distinct and strictly ascending (violation at index {index})")]
    NonCanonicalCouncilSignerOrder { index: usize },
    #[error("council signature at index {index} must contain 64 bytes (found {found})")]
    InvalidCouncilSignatureLength { index: usize, found: usize },
    #[error("council signature at index {index} must not be all zero")]
    InertCouncilSignature { index: usize },
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
    #[error("alias proof council authorization failed: {0}")]
    CouncilAuthorization(#[source] ProviderAdmissionSignatureError),
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

/// Compute the digest that council members sign for an alias-registry root.
///
/// Council signatures deliberately authorize the root and its lifetime rather
/// than each leaf independently. [`verify_alias_proof_bundle`] authenticates a
/// particular binding by recomputing its leaf-to-root Merkle path before it
/// verifies the trusted council quorum over this digest.
#[must_use]
pub fn alias_proof_signature_digest(bundle: &AliasProofBundleV1) -> [u8; 32] {
    alias_signature_message(bundle)
}

/// Verify an alias proof bundle against an operator-controlled council policy.
///
/// # Errors
///
/// Returns [`AliasProofVerificationError`] when the bundle fails structural
/// validation, its binding is not a member of the signed registry root, or the
/// configured trusted council quorum did not authorize that root.
pub fn verify_alias_proof_bundle(
    bundle: &AliasProofBundleV1,
    policy: &ProviderAdmissionCouncilPolicy,
) -> Result<(), AliasProofVerificationError> {
    verify_alias_proof_bundle_inner(bundle, |signatures, digest| {
        verify_council_signatures_over_digest(signatures, digest, policy)
    })
}

/// Verify alias-proof integrity without establishing signer trust.
///
/// This helper is reserved for fixture generation, language-neutral reference
/// validation, and cache-freshness tooling. A node or gateway making an
/// admission decision must use [`verify_alias_proof_bundle`] with an
/// operator-controlled [`ProviderAdmissionCouncilPolicy`].
///
/// # Errors
///
/// Returns [`AliasProofVerificationError`] when the bundle fails structural,
/// Merkle-membership, or embedded-signature validation.
pub fn verify_alias_proof_bundle_untrusted_signers(
    bundle: &AliasProofBundleV1,
) -> Result<(), AliasProofVerificationError> {
    verify_alias_proof_bundle_inner(bundle, verify_council_signatures_without_trust)
}

fn verify_alias_proof_bundle_inner<F>(
    bundle: &AliasProofBundleV1,
    verify_signatures: F,
) -> Result<(), AliasProofVerificationError>
where
    F: FnOnce(&[CouncilSignature], &[u8; 32]) -> Result<(), ProviderAdmissionSignatureError>,
{
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
    verify_signatures(&bundle.council_signatures, &message)
        .map_err(AliasProofVerificationError::CouncilAuthorization)
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

    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};

    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn canonical_cid(seed: u8) -> Vec<u8> {
        crate::canonical_manifest_root_cid([seed; 32])
    }

    fn sample_alias_binding() -> AliasBindingV1 {
        AliasBindingV1 {
            alias: "docs/main".into(),
            manifest_cid: canonical_cid(0x10),
            bound_at: 1_700_000_000,
            expiry_epoch: 1_700_086_400,
        }
    }

    #[test]
    fn alias_binding_validation() {
        let binding = sample_alias_binding();
        binding.validate().expect("valid alias binding");
    }

    #[test]
    fn alias_binding_accepts_account_style_alias() {
        let binding = AliasBindingV1 {
            alias: "alias@capability.dataspace".into(),
            manifest_cid: canonical_cid(0x30),
            bound_at: 1,
            expiry_epoch: 2,
        };

        binding.validate().expect("account-style alias");
    }

    #[test]
    fn alias_binding_rejects_whitespace() {
        let binding = AliasBindingV1 {
            alias: " docs ".into(),
            manifest_cid: canonical_cid(0x30),
            bound_at: 1,
            expiry_epoch: 2,
        };
        let err = binding.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasBindingValidationError::AliasHasWhitespace
        ));
    }

    #[test]
    fn alias_binding_rejects_noncanonical_manifest_cid() {
        let mut binding = sample_alias_binding();
        binding.manifest_cid[0] = 2;
        assert!(matches!(
            binding.validate(),
            Err(AliasBindingValidationError::MalformedManifestCid { .. })
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
                signature: vec![0x01; 64],
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
        let signature = Signature::try_new(keypair.private_key(), digest.as_ref())
            .expect("sign SoraFS alias proof fixture");
        let signer_vec = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid")
            .1;
        let signer: [u8; 32] = signer_vec
            .try_into()
            .expect("ed25519 public key must be 32 bytes");
        bundle.council_signatures.push(crate::CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        });
        (bundle, keypair)
    }

    fn council_policy_for(keypair: &KeyPair) -> ProviderAdmissionCouncilPolicy {
        let signer: [u8; 32] = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid")
            .1
            .try_into()
            .expect("ed25519 public key must be 32 bytes");
        ProviderAdmissionCouncilPolicy::new([signer], 1).expect("valid fixture council policy")
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
    fn alias_proof_bundle_rejects_zero_generation_and_zero_lifetime() {
        let mut zero_generation = sample_alias_proof_bundle();
        zero_generation.generated_at_unix = 0;
        assert!(matches!(
            zero_generation.validate(),
            Err(AliasProofBundleValidationError::InvalidGeneratedAt)
        ));

        let mut zero_lifetime = sample_alias_proof_bundle();
        zero_lifetime.expires_at_unix = zero_lifetime.generated_at_unix;
        assert!(matches!(
            zero_lifetime.validate(),
            Err(AliasProofBundleValidationError::GeneratedAfterExpiry { .. })
        ));
    }

    #[test]
    fn alias_proof_bundle_rejects_resource_exhaustion_shapes() {
        let mut deep = sample_alias_proof_bundle();
        deep.merkle_path = vec![[0xA5; 32]; MAX_ALIAS_PROOF_MERKLE_DEPTH + 1];
        assert!(matches!(
            deep.validate(),
            Err(AliasProofBundleValidationError::MerklePathTooDeep { .. })
        ));

        let mut signature_flood = sample_alias_proof_bundle();
        signature_flood.council_signatures = (0..=MAX_ALIAS_PROOF_COUNCIL_SIGNATURES)
            .map(|index| crate::CouncilSignature {
                signer: [u8::try_from(index + 1).expect("test index fits u8"); 32],
                signature: vec![0x01; 64],
            })
            .collect();
        assert!(matches!(
            signature_flood.validate(),
            Err(AliasProofBundleValidationError::TooManyCouncilSignatures { .. })
        ));
    }

    #[test]
    fn alias_proof_bundle_rejects_duplicate_and_unsorted_signers() {
        for signers in [vec![[0x11; 32], [0x11; 32]], vec![[0x22; 32], [0x11; 32]]] {
            let mut bundle = sample_alias_proof_bundle();
            bundle.council_signatures = signers
                .into_iter()
                .map(|signer| crate::CouncilSignature {
                    signer,
                    signature: vec![0x01; 64],
                })
                .collect();
            assert!(matches!(
                bundle.validate(),
                Err(AliasProofBundleValidationError::NonCanonicalCouncilSignerOrder { .. })
            ));
        }
    }

    #[test]
    fn alias_proof_bundle_rejects_empty_council_signature() {
        let mut bundle = sample_alias_proof_bundle();
        bundle.council_signatures[0].signature.clear();
        let err = bundle.validate().unwrap_err();
        assert!(matches!(
            err,
            AliasProofBundleValidationError::InvalidCouncilSignatureLength { found: 0, .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_verification_succeeds() {
        let (bundle, keypair) = signed_alias_proof_bundle();
        let policy = council_policy_for(&keypair);
        verify_alias_proof_bundle(&bundle, &policy).expect("bundle must verify");
        verify_alias_proof_bundle_untrusted_signers(&bundle)
            .expect("untrusted fixture verification must preserve integrity checks");
    }

    #[test]
    fn alias_proof_bundle_rejects_self_asserted_council() {
        let (bundle, _) = signed_alias_proof_bundle();
        let trusted = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x22; 32])
            .and_then(KeyPair::from_private_key)
            .expect("derive trusted fixture keypair");
        let policy = council_policy_for(&trusted);

        let err = verify_alias_proof_bundle(&bundle, &policy)
            .expect_err("a self-signed bundle outside the trust set must fail");
        assert!(matches!(
            err,
            AliasProofVerificationError::CouncilAuthorization(
                ProviderAdmissionSignatureError::UntrustedSigner { .. }
            )
        ));
    }

    #[test]
    fn signed_alias_proof_bundle_checked_signature_verifies_digest() {
        let (bundle, keypair) = signed_alias_proof_bundle();
        let digest = alias_proof_signature_digest(&bundle);
        let signature = &bundle.council_signatures[0].signature;
        let signature = Signature::try_from_bytes(signature)
            .expect("checked alias proof fixture signature passes admission");

        signature
            .verify(keypair.public_key(), digest.as_ref())
            .expect("checked alias proof fixture signature verifies");
    }

    #[test]
    fn alias_proof_bundle_verification_rejects_root_mismatch() {
        let (mut bundle, keypair) = signed_alias_proof_bundle();
        let policy = council_policy_for(&keypair);
        bundle.registry_root[0] ^= 0xFF;
        let err = verify_alias_proof_bundle(&bundle, &policy).expect_err("verification must fail");
        assert!(matches!(
            err,
            AliasProofVerificationError::MerkleRootMismatch { .. }
        ));
    }

    #[test]
    fn alias_proof_bundle_verification_rejects_bad_signature() {
        let (mut bundle, keypair) = signed_alias_proof_bundle();
        let policy = council_policy_for(&keypair);
        bundle.council_signatures[0].signature[0] ^= 0xFF;
        let err = verify_alias_proof_bundle(&bundle, &policy).expect_err("verification must fail");
        assert!(matches!(
            err,
            AliasProofVerificationError::CouncilAuthorization(
                ProviderAdmissionSignatureError::Verification { .. }
            )
        ));
    }

    #[test]
    fn alias_proof_bundle_verification_rejects_all_zero_signature_material() {
        let (mut bundle, keypair) = signed_alias_proof_bundle();
        let policy = council_policy_for(&keypair);
        bundle.council_signatures[0].signature.fill(0);
        let err = verify_alias_proof_bundle(&bundle, &policy).expect_err("verification must fail");
        assert!(matches!(
            err,
            AliasProofVerificationError::Validation(
                AliasProofBundleValidationError::InertCouncilSignature { .. }
            )
        ));
    }

    #[test]
    fn alias_proof_bundle_verification_rejects_malformed_signature_r() {
        for (label, replacement_r, expected_reason) in [
            ("small-order", SMALL_ORDER_R, "small-order"),
            ("noncanonical", NONCANONICAL_R, "not a canonical"),
        ] {
            let (mut bundle, keypair) = signed_alias_proof_bundle();
            let policy = council_policy_for(&keypair);
            bundle.council_signatures[0].signature[..32].copy_from_slice(&replacement_r);

            let err =
                verify_alias_proof_bundle(&bundle, &policy).expect_err("verification must fail");
            assert!(
                matches!(
                    &err,
                    AliasProofVerificationError::CouncilAuthorization(
                        ProviderAdmissionSignatureError::Verification { reason, .. }
                    ) if reason.contains(expected_reason)
                ),
                "{label} signature R produced unexpected error: {err}"
            );
        }
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
