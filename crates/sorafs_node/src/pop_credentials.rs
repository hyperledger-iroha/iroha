//! Production service primitives for SoraFS proof-of-personhood credentials.
//!
//! The service deliberately separates three kinds of state:
//!
//! - the ledger receives only signed public roots, signed revocation snapshots,
//!   and domain-separated commitments to private credentials and nonces;
//! - the issuer checkpoint retains encrypted enrollment and encrypted wallet
//!   delivery envelopes, plus payload-free approval/outbox metadata;
//! - the wallet vault retains credential and witness material only inside a
//!   KMS/PKCS#11-wrapped ChaCha20-Poly1305 envelope.
//!
//! There is no local-authority fallback. Issuance and verification are bound to
//! an explicitly supplied finalized policy/root projection, and every registry
//! submission is recovered through a durable idempotent outbox.

use std::{
    collections::BTreeSet,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_config::parameters::validate_production_runtime_handle;
#[cfg(test)]
use iroha_crypto::HybridSecretKey;
use iroha_crypto::{
    Algorithm, HybridPublicKey, PublicKey, Signature,
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
};
use iroha_data_model::sorafs::pop_registry::{
    POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1, PopCredentialCommitmentBatchV1,
    PopCredentialCommitmentV1, pop_credential_payload_commitment_v1,
    pop_revocation_nonce_commitment_v1,
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use rand::{rand_core::TryCryptoRng, rand_core::TryRngCore as _, rngs::OsRng};
#[cfg(test)]
use sorafs_manifest::hybrid_envelope::decrypt_payload;
use sorafs_manifest::{
    hybrid_envelope::{HybridPayloadEnvelopeV1, encrypt_payload},
    pop_credentials::{
        POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1, POP_MEMBERSHIP_SEEN_NULLIFIERS_MAX_V1,
        PopCommitmentRootV1, PopCredentialMerklePathV1, PopCredentialV1, PopEnrollmentRequestV1,
        PopIssuedCredentialBundleV1, PopMembershipProofV1, PopMembershipWitnessV1,
        PopRevocationListV1, PopRevocationNonMembershipPathV1, PopSignatureAlgorithmV1,
        PopSignatureV1, pop_commitment_root_signature_digest_v1,
        pop_credential_signature_digest_v1, pop_revocation_list_signature_digest_v1,
        prove_pop_membership_v1, verify_pop_commitment_root_signature_v1,
        verify_pop_membership_proof_v1, verify_pop_revocation_list_signature_v1,
    },
};
use thiserror::Error;

use crate::{
    decode_local_checkpoint_canonical, read_local_checkpoint_bounded,
    write_local_private_checkpoint_atomic,
};

/// V1 issuer-service policy version.
pub const POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1: u16 = 1;
/// V1 encrypted enrollment envelope version.
pub const POP_ENCRYPTED_ENROLLMENT_VERSION_V1: u16 = 1;
/// V1 dual-control approval version.
pub const POP_APPROVAL_VERSION_V1: u16 = 1;
/// V1 encrypted wallet delivery version.
pub const POP_WALLET_DELIVERY_VERSION_V1: u16 = 1;
/// V1 registry operation version.
pub const POP_REGISTRY_OPERATION_VERSION_V1: u16 = 1;
/// V1 finalized registry projection version.
pub const POP_FINALIZED_REGISTRY_PROJECTION_VERSION_V1: u16 = 1;
/// V1 issuer checkpoint version.
pub const POP_ISSUER_CHECKPOINT_VERSION_V1: u16 = 1;
/// V1 wallet-vault envelope version.
pub const POP_WALLET_VAULT_ENVELOPE_VERSION_V1: u16 = 1;
/// Maximum canonical encrypted enrollment envelope size.
pub const POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1: usize = 1024 * 1024;
/// Maximum private attestation payload accepted before encryption.
pub const POP_ENROLLMENT_ATTESTATION_MAX_BYTES_V1: usize = 768 * 1024;
/// Maximum encrypted wallet delivery size.
pub const POP_WALLET_DELIVERY_MAX_BYTES_V1: usize = 2 * 1024 * 1024;
/// Maximum issuer checkpoint size.
pub const POP_ISSUER_CHECKPOINT_MAX_BYTES_V1: u64 = 32 * 1024 * 1024;
/// Maximum wallet-vault envelope size.
pub const POP_WALLET_VAULT_MAX_BYTES_V1: u64 = 4 * 1024 * 1024;
/// Maximum opaque wrapped-DEK size.
pub const POP_WRAPPED_DEK_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum opaque authentication credential accepted at the API boundary.
pub const POP_API_AUTHENTICATION_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum bounded operational collection size.
pub const POP_SERVICE_COLLECTION_MAX_V1: usize = 65_536;
/// Fixed issuer checkpoint file name.
pub const POP_ISSUER_CHECKPOINT_FILE_V1: &str = "issuer-checkpoint.to";

const ENROLLMENT_ATTESTATION_DOMAIN_V1: &[u8] = b"sorafs.pop.enrollment-attestation.v1";
const ENROLLMENT_ENVELOPE_DOMAIN_V1: &[u8] = b"sorafs.pop.encrypted-enrollment.v1";
const ENROLLMENT_AAD_DOMAIN_V1: &[u8] = b"sorafs.pop.enrollment-aad.v1";
const ENROLLMENT_RECIPIENT_PUBLIC_KEY_DOMAIN_V1: &[u8] =
    b"sorafs.pop.enrollment-recipient-public-key.v1";
const APPROVAL_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.pop.dual-control-approval.v1";
const REGISTRY_OPERATION_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-operation.v1";
const REGISTRY_IDEMPOTENCY_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-idempotency.v1";
const ISSUE_TRIGGER_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.issue-trigger.v1";
const REVOCATION_API_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.revocation-api.v1";
const REGISTRY_SUBMIT_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-submit-api.v1";
const REGISTRY_RECONCILE_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-reconcile-api.v1";
const REGISTRY_PROJECTION_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.registry-projection-api.v1";
const WALLET_DELIVERY_AAD_DOMAIN_V1: &[u8] = b"sorafs.pop.wallet-delivery-aad.v1";
const WALLET_IMPORT_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.wallet-import-api.v1";
const WALLET_WITNESS_SYNC_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.wallet-witness-sync-api.v1";
const WALLET_PROVE_BINDING_DOMAIN_V1: &[u8] = b"sorafs.pop.wallet-prove-api.v1";
const WALLET_VAULT_AAD_DOMAIN_V1: &[u8] = b"sorafs.pop.wallet-vault-aad.v1";
const WALLET_VAULT_FILE_PREFIX_V1: &str = "credential-";
const WALLET_VAULT_FILE_SUFFIX_V1: &str = ".to";

fn digest_domain(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

/// Return the canonical V1 digest of one hybrid enrollment-recipient public key.
///
/// The digest is safe to place in public configuration. It lets startup prove
/// that a runtime-only recipient secret is the exact configured key without
/// exporting or logging either secret component.
#[must_use]
pub fn pop_enrollment_recipient_public_key_digest_v1(recipient: &HybridPublicKey) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ENROLLMENT_RECIPIENT_PUBLIC_KEY_DOMAIN_V1);
    hasher.update(&recipient.x25519_bytes());
    hasher.update(recipient.kyber_bytes());
    *hasher.finalize().as_bytes()
}

fn scrub_sensitive_bytes(bytes: &mut [u8]) {
    bytes.fill(0);
    // Make the cleared contents observable to the optimizer without requiring
    // unsafe volatile writes. This module is compiled with `unsafe_code = deny`.
    std::hint::black_box(bytes);
}

fn scrub_sensitive_string(value: &mut String) {
    if !value.is_empty() {
        let cleared = "\0".repeat(value.len());
        value.replace_range(.., &cleared);
        std::hint::black_box(value.as_bytes());
        value.clear();
    }
}

/// Borrowed secret bytes that are scrubbed on every return path.
struct SensitiveBytesGuard<'a> {
    bytes: &'a mut [u8],
}

impl<'a> SensitiveBytesGuard<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes }
    }

    fn as_slice(&self) -> &[u8] {
        &*self.bytes
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.bytes
    }
}

impl Drop for SensitiveBytesGuard<'_> {
    fn drop(&mut self) {
        scrub_sensitive_bytes(self.bytes);
    }
}

fn nonzero_digest(field: &'static str, digest: [u8; 32]) -> Result<(), PopCredentialServiceError> {
    if digest == [0; 32] {
        return Err(PopCredentialServiceError::InvalidInput { field });
    }
    Ok(())
}

fn bounded_clean_text(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), PopCredentialServiceError> {
    if value.is_empty()
        || value != value.trim()
        || value.len() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(PopCredentialServiceError::InvalidInput { field });
    }
    Ok(())
}

fn bounded_production_runtime_handle(
    field: &'static str,
    value: &str,
) -> Result<(), PopCredentialServiceError> {
    validate_production_runtime_handle(value)
        .map_err(|_| PopCredentialServiceError::InvalidInput { field })
}

fn encode_canonical<T: norito::core::NoritoSerialize>(
    value: &T,
) -> Result<Vec<u8>, PopCredentialServiceError> {
    norito::to_bytes(value).map_err(|_| PopCredentialServiceError::Codec)
}

fn decode_canonical<T>(
    bytes: &[u8],
    max_bytes: u64,
    max_sequence_elements: usize,
) -> Result<T, PopCredentialServiceError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    decode_local_checkpoint_canonical(bytes, max_bytes, max_sequence_elements)
        .map_err(|_| PopCredentialServiceError::Codec)
}

/// Domain-separated digest for off-chain enrollment attestations.
#[must_use]
pub fn pop_enrollment_attestation_digest_v1(attestation: &[u8]) -> [u8; 32] {
    digest_domain(ENROLLMENT_ATTESTATION_DOMAIN_V1, attestation)
}

/// One governed dual-control approver.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct PopApprovalSignerV1 {
    /// Stable payload-free signer identifier.
    pub signer_id: String,
    /// Raw Ed25519 public key.
    pub public_key: [u8; 32],
    /// Finalized epoch at which this signer was revoked, if any.
    pub revoked_at_epoch: Option<u64>,
}

/// Finalized operational policy consumed by the PoP service.
///
/// This policy is intentionally required at construction. The service never
/// invents a default issuer, approval roster, or local authority.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct PopCredentialServicePolicyV1 {
    /// Schema version.
    pub version: u16,
    /// Exact active ledger issuer-policy digest.
    pub issuer_policy_digest: [u8; 32],
    /// Exact governed issuer identifier.
    pub issuer_id: String,
    /// Non-secret HSM key handle expected at runtime.
    pub issuer_hsm_key_id: String,
    /// Exact governed Ed25519 issuer public key.
    pub issuer_public_key: [u8; 32],
    /// Non-secret recipient key handle for encrypted enrollment.
    pub enrollment_recipient_key_id: String,
    /// Required number of distinct active approvals; V1 requires at least two.
    pub approval_quorum: u8,
    /// Canonically signer-id-ordered governed approvers.
    pub approval_signers: Vec<PopApprovalSignerV1>,
    /// Maximum pending encrypted enrollments.
    pub max_pending_enrollments: u32,
    /// Maximum durable registry outbox entries.
    pub max_outbox_entries: u32,
    /// Maximum durable dead-letter records.
    pub max_dead_letters: u32,
    /// Maximum verifier replay-cache nullifiers.
    pub max_seen_nullifiers: u32,
    /// Submission attempts before an unconfirmed operation enters dead letter.
    pub max_submission_attempts: u16,
}

impl PopCredentialServicePolicyV1 {
    /// Validate bounded, canonical V1 policy invariants.
    pub fn validate(&self) -> Result<(), PopCredentialServiceError> {
        if self.version != POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1 {
            return Err(PopCredentialServiceError::UnsupportedVersion);
        }
        nonzero_digest("issuer_policy_digest", self.issuer_policy_digest)?;
        bounded_clean_text("issuer_id", &self.issuer_id, 256)?;
        bounded_production_runtime_handle("issuer_hsm_key_id", &self.issuer_hsm_key_id)?;
        bounded_production_runtime_handle(
            "enrollment_recipient_key_id",
            &self.enrollment_recipient_key_id,
        )?;
        PublicKey::from_bytes(Algorithm::Ed25519, &self.issuer_public_key).map_err(|_| {
            PopCredentialServiceError::InvalidInput {
                field: "issuer_public_key",
            }
        })?;
        if self.approval_quorum < 2
            || usize::from(self.approval_quorum) > self.approval_signers.len()
        {
            return Err(PopCredentialServiceError::InvalidInput {
                field: "approval_quorum",
            });
        }
        let mut previous: Option<&str> = None;
        let mut distinct_public_keys = BTreeSet::new();
        for signer in &self.approval_signers {
            bounded_clean_text("approval_signer_id", &signer.signer_id, 256)?;
            if previous.is_some_and(|value| value >= signer.signer_id.as_str()) {
                return Err(PopCredentialServiceError::InvalidInput {
                    field: "approval_signers",
                });
            }
            PublicKey::from_bytes(Algorithm::Ed25519, &signer.public_key).map_err(|_| {
                PopCredentialServiceError::InvalidInput {
                    field: "approval_signer_public_key",
                }
            })?;
            if !distinct_public_keys.insert(signer.public_key) {
                return Err(PopCredentialServiceError::InvalidInput {
                    field: "approval_signer_public_key",
                });
            }
            if signer.revoked_at_epoch == Some(0) {
                return Err(PopCredentialServiceError::InvalidInput {
                    field: "approval_signer_revoked_at_epoch",
                });
            }
            previous = Some(signer.signer_id.as_str());
        }
        if self.max_pending_enrollments == 0
            || usize::try_from(self.max_pending_enrollments).unwrap_or(usize::MAX)
                > POP_SERVICE_COLLECTION_MAX_V1
            || self.max_outbox_entries == 0
            || usize::try_from(self.max_outbox_entries).unwrap_or(usize::MAX)
                > POP_SERVICE_COLLECTION_MAX_V1
            || self.max_dead_letters == 0
            || usize::try_from(self.max_dead_letters).unwrap_or(usize::MAX)
                > POP_SERVICE_COLLECTION_MAX_V1
            || self.max_seen_nullifiers == 0
            || usize::try_from(self.max_seen_nullifiers).unwrap_or(usize::MAX)
                > POP_MEMBERSHIP_SEEN_NULLIFIERS_MAX_V1
            || self.max_submission_attempts == 0
        {
            return Err(PopCredentialServiceError::InvalidInput {
                field: "service_resource_limits",
            });
        }
        Ok(())
    }

    fn approval_signer(
        &self,
        signer_id: &str,
        now_epoch: u64,
    ) -> Result<&PopApprovalSignerV1, PopCredentialServiceError> {
        let signer = self
            .approval_signers
            .binary_search_by(|candidate| candidate.signer_id.as_str().cmp(signer_id))
            .ok()
            .and_then(|index| self.approval_signers.get(index))
            .ok_or(PopCredentialServiceError::Unauthorized)?;
        if signer
            .revoked_at_epoch
            .is_some_and(|revoked_at| now_epoch >= revoked_at)
        {
            return Err(PopCredentialServiceError::SignerRevoked);
        }
        Ok(signer)
    }
}

/// Public metadata integrity-protected with an encrypted enrollment.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
struct PopEnrollmentAadV1 {
    version: u16,
    request_id: [u8; 32],
    issuer_policy_digest: [u8; 32],
    issuer_id: String,
    recipient_key_id: String,
}

/// Private enrollment plaintext. It is never written or logged.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PopPrivateEnrollmentV1 {
    /// Canonical enrollment request, including the private applicant alias.
    pub request: PopEnrollmentRequestV1,
    /// Holder commitment to bind into the issued credential.
    pub holder_commitment: [u8; 32],
    /// Wallet X25519 public component for encrypted credential delivery.
    pub wallet_x25519_public_key: [u8; 32],
    /// Wallet ML-KEM-768 public component for encrypted credential delivery.
    pub wallet_mlkem_public_key: Vec<u8>,
    /// Private enrollment attestation payload.
    pub attestation_payload: Vec<u8>,
}

impl fmt::Debug for PopPrivateEnrollmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopPrivateEnrollmentV1")
            .field("request_id", &hex::encode(self.request.request_id))
            .field("requested_class", &self.request.requested_class)
            .field(
                "requested_attribute_count",
                &self.request.requested_attributes.len(),
            )
            .field("private_payload", &"[REDACTED]")
            .finish()
    }
}

impl Drop for PopPrivateEnrollmentV1 {
    fn drop(&mut self) {
        scrub_sensitive_string(&mut self.request.applicant_id);
        for attribute in &mut self.request.requested_attributes {
            scrub_sensitive_string(attribute);
        }
        self.request.requested_attributes.clear();
        scrub_sensitive_bytes(&mut self.request.attestation_digest);
        scrub_sensitive_bytes(&mut self.holder_commitment);
        scrub_sensitive_bytes(&mut self.wallet_x25519_public_key);
        scrub_sensitive_bytes(&mut self.wallet_mlkem_public_key);
        scrub_sensitive_bytes(&mut self.attestation_payload);
    }
}

impl PopPrivateEnrollmentV1 {
    fn validate(&self) -> Result<HybridPublicKey, PopCredentialServiceError> {
        self.request
            .validate()
            .map_err(|_| PopCredentialServiceError::InvalidEnrollment)?;
        nonzero_digest("holder_commitment", self.holder_commitment)?;
        if self.attestation_payload.is_empty()
            || self.attestation_payload.len() > POP_ENROLLMENT_ATTESTATION_MAX_BYTES_V1
            || pop_enrollment_attestation_digest_v1(&self.attestation_payload)
                != self.request.attestation_digest
        {
            return Err(PopCredentialServiceError::InvalidEnrollment);
        }
        HybridPublicKey::from_bytes(self.wallet_x25519_public_key, &self.wallet_mlkem_public_key)
            .map_err(|_| PopCredentialServiceError::InvalidEnrollment)
    }
}

/// Canonical encrypted enrollment accepted by the issuer service.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
pub struct PopEncryptedEnrollmentV1 {
    /// Schema version.
    pub version: u16,
    /// Random request identifier; no account or PII is exposed.
    pub request_id: [u8; 32],
    /// Finalized issuer policy expected by the applicant.
    pub issuer_policy_digest: [u8; 32],
    /// Governed issuer identifier.
    pub issuer_id: String,
    /// Runtime recipient-key handle.
    pub recipient_key_id: String,
    /// Hybrid X25519+ML-KEM/ChaCha20-Poly1305 private payload.
    pub encrypted_payload: HybridPayloadEnvelopeV1,
}

impl PopEncryptedEnrollmentV1 {
    fn aad(&self) -> Result<Vec<u8>, PopCredentialServiceError> {
        let metadata = PopEnrollmentAadV1 {
            version: self.version,
            request_id: self.request_id,
            issuer_policy_digest: self.issuer_policy_digest,
            issuer_id: self.issuer_id.clone(),
            recipient_key_id: self.recipient_key_id.clone(),
        };
        let encoded = encode_canonical(&metadata)?;
        Ok([ENROLLMENT_AAD_DOMAIN_V1, encoded.as_slice()].concat())
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, PopCredentialServiceError> {
        let bytes = encode_canonical(self)?;
        if bytes.len() > POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        Ok(bytes)
    }

    fn digest(&self) -> Result<[u8; 32], PopCredentialServiceError> {
        Ok(digest_domain(
            ENROLLMENT_ENVELOPE_DOMAIN_V1,
            &self.canonical_bytes()?,
        ))
    }
}

/// Encrypt a private enrollment for the governed issuer recipient key.
pub fn encrypt_pop_enrollment_v1<R: TryCryptoRng>(
    private: &PopPrivateEnrollmentV1,
    policy: &PopCredentialServicePolicyV1,
    recipient: &HybridPublicKey,
    rng: &mut R,
) -> Result<PopEncryptedEnrollmentV1, PopCredentialServiceError> {
    policy.validate()?;
    private.validate()?;
    let mut envelope = PopEncryptedEnrollmentV1 {
        version: POP_ENCRYPTED_ENROLLMENT_VERSION_V1,
        request_id: private.request.request_id,
        issuer_policy_digest: policy.issuer_policy_digest,
        issuer_id: policy.issuer_id.clone(),
        recipient_key_id: policy.enrollment_recipient_key_id.clone(),
        encrypted_payload: HybridPayloadEnvelopeV1 {
            version: 0,
            suite: String::new(),
            kem: sorafs_manifest::hybrid_envelope::HybridKemBundleV1 {
                ephemeral_public: Vec::new(),
                kyber_ciphertext: Vec::new(),
            },
            nonce: [0; 12],
            ciphertext: Vec::new(),
        },
    };
    let aad = envelope.aad()?;
    let mut plaintext = encode_canonical(private)?;
    let plaintext = SensitiveBytesGuard::new(&mut plaintext);
    envelope.encrypted_payload = encrypt_payload(plaintext.as_slice(), &aad, recipient, rng)
        .map_err(|_| PopCredentialServiceError::Encryption)?;
    drop(plaintext);
    envelope.canonical_bytes()?;
    Ok(envelope)
}

/// Approval decision.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "decision", content = "value", rename_all = "snake_case")]
pub enum PopApprovalDecisionV1 {
    /// Approve issuance.
    Approve,
    /// Reject issuance.
    Reject,
}

/// Signed payload-free dual-control decision.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct PopApprovalV1 {
    /// Schema version.
    pub version: u16,
    /// Enrollment request identifier.
    pub request_id: [u8; 32],
    /// Digest of the exact encrypted enrollment envelope.
    pub enrollment_envelope_digest: [u8; 32],
    /// Exact finalized issuer-policy digest.
    pub issuer_policy_digest: [u8; 32],
    /// Approval or rejection.
    pub decision: PopApprovalDecisionV1,
    /// Decision epoch.
    pub decided_at_epoch: u64,
    /// Governed signer id.
    pub signer_id: String,
    /// Raw Ed25519 signature.
    pub signature: Vec<u8>,
}

impl PopApprovalV1 {
    /// Compute the canonical signed digest.
    pub fn signature_digest(&self) -> Result<[u8; 32], PopCredentialServiceError> {
        let mut signable = self.clone();
        signable.signature.clear();
        Ok(digest_domain(
            APPROVAL_SIGNATURE_DOMAIN_V1,
            &encode_canonical(&signable)?,
        ))
    }

    fn validate(
        &self,
        policy: &PopCredentialServicePolicyV1,
        expected_request_id: [u8; 32],
        expected_envelope_digest: [u8; 32],
        now_epoch: u64,
    ) -> Result<(), PopCredentialServiceError> {
        if self.version != POP_APPROVAL_VERSION_V1 {
            return Err(PopCredentialServiceError::UnsupportedVersion);
        }
        if self.request_id != expected_request_id
            || self.enrollment_envelope_digest != expected_envelope_digest
            || self.issuer_policy_digest != policy.issuer_policy_digest
            || self.decided_at_epoch == 0
            || self.decided_at_epoch > now_epoch
        {
            return Err(PopCredentialServiceError::ApprovalBinding);
        }
        let signer = policy.approval_signer(&self.signer_id, now_epoch)?;
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer.public_key)
            .map_err(|_| PopCredentialServiceError::Unauthorized)?;
        let signature = Signature::try_from_bytes(&self.signature)
            .map_err(|_| PopCredentialServiceError::InvalidSignature)?;
        signature
            .verify(&public_key, &self.signature_digest()?)
            .map_err(|_| PopCredentialServiceError::InvalidSignature)
    }
}

/// Runtime-only HSM/PKCS#11 signing interface.
///
/// Implementations must not expose or persist private key bytes. The service
/// checks both the key handle and public key against finalized policy.
pub trait PopIssuerHsm: Send + Sync + fmt::Debug {
    /// Return the non-secret runtime key handle.
    fn key_id(&self) -> &str;
    /// Return the raw Ed25519 public key.
    fn public_key(&self) -> [u8; 32];
    /// Sign one canonical 32-byte protocol digest.
    fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], String>;
}

/// Payload-free failure returned by a runtime-owned recipient capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopRecipientOpenErrorV1 {
    /// The provider or its protected key is unavailable.
    Unavailable,
    /// The exact ciphertext, AAD, or governed key binding was rejected.
    Rejected,
}

/// Runtime-only capability that opens encrypted enrollment payloads without
/// exposing the hybrid recipient private key to the caller.
pub trait PopEnrollmentRecipientV1: Send + Sync + fmt::Debug {
    /// Return the stable non-secret protected-key handle.
    fn key_id(&self) -> &str;
    /// Return the digest of the exact hybrid recipient public key.
    fn public_key_digest(&self) -> [u8; 32];
    /// Open one exact encrypted enrollment payload under its canonical AAD.
    fn open_enrollment(
        &self,
        encrypted_payload: &HybridPayloadEnvelopeV1,
        aad: &[u8],
    ) -> Result<Vec<u8>, PopRecipientOpenErrorV1>;
}

/// Runtime-only capability that opens finalized encrypted wallet deliveries
/// without exposing the wallet recipient private key to the caller.
pub trait PopWalletRecipientV1: Send + Sync + fmt::Debug {
    /// Return the stable non-secret protected-key handle.
    fn key_id(&self) -> &str;
    /// Return the digest of the exact hybrid recipient public key.
    fn public_key_digest(&self) -> [u8; 32];
    /// Open one exact finalized wallet-delivery payload under its canonical AAD.
    fn open_wallet_delivery(
        &self,
        encrypted_payload: &HybridPayloadEnvelopeV1,
        aad: &[u8],
    ) -> Result<Vec<u8>, PopRecipientOpenErrorV1>;
}

fn map_enrollment_recipient_error(error: PopRecipientOpenErrorV1) -> PopCredentialServiceError {
    match error {
        PopRecipientOpenErrorV1::Unavailable => {
            PopCredentialServiceError::RuntimeProviderUnavailable
        }
        PopRecipientOpenErrorV1::Rejected => PopCredentialServiceError::InvalidEnrollment,
    }
}

fn map_wallet_recipient_error(error: PopRecipientOpenErrorV1) -> PopCredentialServiceError {
    match error {
        PopRecipientOpenErrorV1::Unavailable => {
            PopCredentialServiceError::RuntimeProviderUnavailable
        }
        PopRecipientOpenErrorV1::Rejected => PopCredentialServiceError::Encryption,
    }
}

/// Runtime-only KMS/PKCS#11 wrapper used by a wallet vault.
pub trait PopWalletKeyWrapper: Send + Sync + fmt::Debug {
    /// Return the active non-secret wrapping-key handle.
    fn active_key_id(&self) -> &str;
    /// Wrap a per-credential 256-bit data-encryption key.
    fn wrap_dek(&self, context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String>;
    /// Unwrap a DEK using the persisted key handle and exact context.
    fn unwrap_dek(
        &self,
        key_id: &str,
        context: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], String>;
}

/// Private serializable witness representation used only inside encryption.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PopPrivateWitnessEnvelopeV1 {
    holder_secret: [u8; 32],
    credential_siblings: Vec<[u8; 32]>,
    credential_directions: Vec<bool>,
    revocation_siblings: Vec<[u8; 32]>,
}

impl fmt::Debug for PopPrivateWitnessEnvelopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopPrivateWitnessEnvelopeV1([REDACTED])")
    }
}

impl PopPrivateWitnessEnvelopeV1 {
    fn from_witness(witness: &PopMembershipWitnessV1) -> Self {
        Self {
            holder_secret: witness.holder_secret,
            credential_siblings: witness.credential_path.siblings.clone(),
            credential_directions: witness.credential_path.directions.clone(),
            revocation_siblings: witness.revocation_path.siblings.clone(),
        }
    }

    fn into_witness(mut self) -> PopMembershipWitnessV1 {
        let witness = PopMembershipWitnessV1 {
            holder_secret: self.holder_secret,
            credential_path: PopCredentialMerklePathV1 {
                siblings: std::mem::take(&mut self.credential_siblings),
                directions: std::mem::take(&mut self.credential_directions),
            },
            revocation_path: PopRevocationNonMembershipPathV1 {
                siblings: std::mem::take(&mut self.revocation_siblings),
            },
        };
        scrub_sensitive_bytes(&mut self.holder_secret);
        witness
    }

    fn zeroize(&mut self) {
        scrub_sensitive_bytes(&mut self.holder_secret);
        for sibling in &mut self.credential_siblings {
            scrub_sensitive_bytes(sibling);
        }
        self.credential_directions.fill(false);
        std::hint::black_box(&mut self.credential_directions);
        for sibling in &mut self.revocation_siblings {
            scrub_sensitive_bytes(sibling);
        }
    }
}

impl Drop for PopPrivateWitnessEnvelopeV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// RAII owner for a decoded membership witness used during local proof
/// generation. `PopMembershipWitnessV1` is defined in the manifest crate and
/// cannot implement `Drop` here, so this guard ensures every return path
/// scrubs the holder secret and Merkle witness material.
struct PrivateMembershipWitnessGuard {
    witness: PopMembershipWitnessV1,
}

impl PrivateMembershipWitnessGuard {
    fn new(witness: PopMembershipWitnessV1) -> Self {
        Self { witness }
    }

    fn as_ref(&self) -> &PopMembershipWitnessV1 {
        &self.witness
    }

    fn zeroize(&mut self) {
        scrub_sensitive_bytes(&mut self.witness.holder_secret);
        for sibling in &mut self.witness.credential_path.siblings {
            scrub_sensitive_bytes(sibling);
        }
        self.witness.credential_path.directions.fill(false);
        std::hint::black_box(&mut self.witness.credential_path.directions);
        for sibling in &mut self.witness.revocation_path.siblings {
            scrub_sensitive_bytes(sibling);
        }
    }
}

impl Drop for PrivateMembershipWitnessGuard {
    fn drop(&mut self) {
        self.zeroize();
    }
}

fn scrub_pop_signature(signature: &mut PopSignatureV1) {
    scrub_sensitive_bytes(&mut signature.public_key);
    scrub_sensitive_bytes(&mut signature.signature);
}

fn scrub_pop_credential(credential: &mut PopCredentialV1) {
    scrub_sensitive_bytes(&mut credential.credential_id);
    scrub_sensitive_bytes(&mut credential.holder_commitment);
    for attribute in &mut credential.attributes {
        scrub_sensitive_string(&mut attribute.key);
        scrub_sensitive_bytes(&mut attribute.value_commitment);
    }
    credential.attributes.clear();
    scrub_sensitive_string(&mut credential.issuer_id);
    scrub_sensitive_bytes(&mut credential.revocation_nonce);
    scrub_sensitive_bytes(&mut credential.commitment_root);
    scrub_pop_signature(&mut credential.issuer_signature);
}

fn scrub_pop_bundle(bundle: &mut PopIssuedCredentialBundleV1) {
    scrub_pop_credential(&mut bundle.credential);
    scrub_sensitive_bytes(&mut bundle.commitment_root.root_digest);
    if let Some(previous) = &mut bundle.commitment_root.previous_root_digest {
        scrub_sensitive_bytes(previous);
    }
    scrub_sensitive_bytes(&mut bundle.commitment_root.governance_event_digest);
    scrub_sensitive_string(&mut bundle.commitment_root.issuer_id);
    scrub_pop_signature(&mut bundle.commitment_root.publisher_signature);
    scrub_sensitive_bytes(&mut bundle.revocation_list.commitment_root);
    scrub_sensitive_bytes(&mut bundle.revocation_list.revocation_root);
    scrub_sensitive_string(&mut bundle.revocation_list.issuer_id);
    for entry in &mut bundle.revocation_list.entries {
        scrub_sensitive_bytes(&mut entry.nonce);
    }
    bundle.revocation_list.entries.clear();
    scrub_pop_signature(&mut bundle.revocation_list.publisher_signature);
}

struct PrivateBundleGuard {
    bundle: Option<PopIssuedCredentialBundleV1>,
}

impl PrivateBundleGuard {
    fn new(bundle: PopIssuedCredentialBundleV1) -> Self {
        Self {
            bundle: Some(bundle),
        }
    }

    fn as_ref(&self) -> Result<&PopIssuedCredentialBundleV1, PopCredentialServiceError> {
        self.bundle
            .as_ref()
            .ok_or(PopCredentialServiceError::InvalidState)
    }

    fn as_mut(&mut self) -> Result<&mut PopIssuedCredentialBundleV1, PopCredentialServiceError> {
        self.bundle
            .as_mut()
            .ok_or(PopCredentialServiceError::InvalidState)
    }

    fn into_inner(mut self) -> Result<PopIssuedCredentialBundleV1, PopCredentialServiceError> {
        self.bundle
            .take()
            .ok_or(PopCredentialServiceError::InvalidState)
    }
}

impl Drop for PrivateBundleGuard {
    fn drop(&mut self) {
        if let Some(bundle) = &mut self.bundle {
            scrub_pop_bundle(bundle);
        }
    }
}

/// Issuance material prepared after enrollment review.
pub struct PopIssuanceDraftV1 {
    /// Enrollment request being fulfilled.
    pub request_id: [u8; 32],
    /// Unsigned private credential body.
    pub credential: PopCredentialV1,
    /// Unsigned public commitment-root publication.
    pub commitment_root: PopCommitmentRootV1,
    /// Unsigned public revocation snapshot.
    pub revocation_list: PopRevocationListV1,
    /// Private wallet witness.
    pub witness: PopMembershipWitnessV1,
}

impl fmt::Debug for PopIssuanceDraftV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopIssuanceDraftV1")
            .field("request_id", &hex::encode(self.request_id))
            .field("private_issuance_material", &"[REDACTED]")
            .finish()
    }
}

impl Drop for PopIssuanceDraftV1 {
    fn drop(&mut self) {
        scrub_pop_credential(&mut self.credential);
        scrub_sensitive_bytes(&mut self.witness.holder_secret);
        for sibling in &mut self.witness.credential_path.siblings {
            scrub_sensitive_bytes(sibling);
        }
        self.witness.credential_path.directions.fill(false);
        std::hint::black_box(&mut self.witness.credential_path.directions);
        for sibling in &mut self.witness.revocation_path.siblings {
            scrub_sensitive_bytes(sibling);
        }
    }
}

/// Registry operation payload retained in the durable outbox.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PopRegistryOperationKindV1 {
    /// Atomic credential commitment/root/revocation batch.
    CommitCredentialBatch {
        /// Exact canonical [`PopCredentialCommitmentBatchV1`] bytes.
        canonical_batch: Vec<u8>,
    },
    /// Signed revocation-list publication.
    PublishRevocationList {
        /// Exact canonical signed [`PopRevocationListV1`] bytes.
        canonical_revocation_list: Vec<u8>,
        /// Exact finalized issuer policy digest.
        issuer_policy_digest: [u8; 32],
    },
}

/// One payload-free ledger submission.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct PopRegistryOperationV1 {
    /// Schema version.
    pub version: u16,
    /// Domain-separated digest of this exact operation.
    pub operation_digest: [u8; 32],
    /// Ledger instruction body.
    pub kind: PopRegistryOperationKindV1,
}

impl PopRegistryOperationV1 {
    fn new(kind: PopRegistryOperationKindV1) -> Result<Self, PopCredentialServiceError> {
        let mut operation = Self {
            version: POP_REGISTRY_OPERATION_VERSION_V1,
            operation_digest: [0; 32],
            kind,
        };
        operation.operation_digest =
            digest_domain(REGISTRY_OPERATION_DOMAIN_V1, &encode_canonical(&operation)?);
        Ok(operation)
    }

    /// Validate the digest and exact canonical nested payload.
    pub fn validate(&self) -> Result<(), PopCredentialServiceError> {
        if self.version != POP_REGISTRY_OPERATION_VERSION_V1 {
            return Err(PopCredentialServiceError::UnsupportedVersion);
        }
        let mut signable = self.clone();
        signable.operation_digest = [0; 32];
        if self.operation_digest
            != digest_domain(REGISTRY_OPERATION_DOMAIN_V1, &encode_canonical(&signable)?)
        {
            return Err(PopCredentialServiceError::OperationDigest);
        }
        match &self.kind {
            PopRegistryOperationKindV1::CommitCredentialBatch { canonical_batch } => {
                let batch: PopCredentialCommitmentBatchV1 = decode_canonical(
                    canonical_batch,
                    POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
                    POP_SERVICE_COLLECTION_MAX_V1,
                )?;
                batch
                    .validate()
                    .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
            }
            PopRegistryOperationKindV1::PublishRevocationList {
                canonical_revocation_list,
                issuer_policy_digest,
            } => {
                nonzero_digest("issuer_policy_digest", *issuer_policy_digest)?;
                let list: PopRevocationListV1 = decode_canonical(
                    canonical_revocation_list,
                    POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
                    POP_SERVICE_COLLECTION_MAX_V1,
                )?;
                verify_pop_revocation_list_signature_v1(&list)
                    .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
            }
        }
        Ok(())
    }

    fn validate_for_policy(
        &self,
        expected_policy_digest: [u8; 32],
    ) -> Result<(), PopCredentialServiceError> {
        self.validate()?;
        let embedded_policy_digest = match &self.kind {
            PopRegistryOperationKindV1::CommitCredentialBatch { canonical_batch } => {
                decode_canonical::<PopCredentialCommitmentBatchV1>(
                    canonical_batch,
                    POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
                    POP_SERVICE_COLLECTION_MAX_V1,
                )?
                .issuer_policy_digest
            }
            PopRegistryOperationKindV1::PublishRevocationList {
                issuer_policy_digest,
                ..
            } => *issuer_policy_digest,
        };
        if embedded_policy_digest != expected_policy_digest {
            return Err(PopCredentialServiceError::WrongPolicy);
        }
        Ok(())
    }
}

/// Idempotent registry transaction submitter.
pub trait PopRegistrySubmitter: Send + Sync + fmt::Debug {
    /// Submit an operation. Reusing `idempotency_key` must return the original
    /// transaction result rather than creating another transaction.
    fn submit(
        &self,
        idempotency_key: [u8; 32],
        operation: &PopRegistryOperationV1,
    ) -> Result<(), String>;
}

/// Finalized block cursor used by registry reconciliation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct PopFinalizedCursorV1 {
    /// Finalized block height.
    pub block_height: u64,
    /// Finalized block hash.
    pub block_hash: [u8; 32],
}

/// Finalized, public PoP registry projection returned by ledger queries.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct PopFinalizedRegistryProjectionV1 {
    /// Schema version.
    pub version: u16,
    /// Finalized cursor.
    pub cursor: PopFinalizedCursorV1,
    /// Previous finalized block hash, absent only at height one.
    pub previous_block_hash: Option<[u8; 32]>,
    /// Exact active issuer policy digest.
    pub issuer_policy_digest: [u8; 32],
    /// Exact canonical signed active root.
    pub canonical_commitment_root: Vec<u8>,
    /// Exact canonical signed active revocation list.
    pub canonical_revocation_list: Vec<u8>,
    /// Operations committed in this finalized block.
    pub committed_operation_digests: Vec<[u8; 32]>,
    /// Operations terminally rejected in this finalized block.
    pub rejected_operation_digests: Vec<[u8; 32]>,
    /// Governed issuer keys that are revoked at this cursor.
    pub revoked_issuer_public_keys: Vec<[u8; 32]>,
}

/// Reader that advances exactly one finalized projection at a time.
pub trait PopFinalizedRegistryReader: Send + Sync + fmt::Debug {
    /// Return the next projection after `cursor`, or `None` if caught up.
    fn next_after(
        &self,
        cursor: Option<PopFinalizedCursorV1>,
    ) -> Result<Option<PopFinalizedRegistryProjectionV1>, String>;
}

/// One request's exact authoritative finalized-head reconciliation context.
#[derive(Clone, Copy, Debug)]
pub struct PopCommittedRegistryContextV1<'a> {
    reader: &'a dyn PopFinalizedRegistryReader,
    expected_cursor: PopFinalizedCursorV1,
    now_epoch: u64,
}

impl<'a> PopCommittedRegistryContextV1<'a> {
    /// Bind a committed-state reader and time to one exact finalized head.
    pub fn new(
        reader: &'a dyn PopFinalizedRegistryReader,
        expected_cursor: PopFinalizedCursorV1,
        now_epoch: u64,
    ) -> Result<Self, PopCredentialServiceError> {
        if expected_cursor.block_height == 0
            || expected_cursor.block_hash == [0; 32]
            || now_epoch == 0
        {
            return Err(PopCredentialServiceError::RegistryUnavailable);
        }
        Ok(Self {
            reader,
            expected_cursor,
            now_epoch,
        })
    }

    /// Finalized epoch bound to this exact head.
    #[must_use]
    pub const fn now_epoch(self) -> u64 {
        self.now_epoch
    }

    /// Reconcile and prove that `service` reached this exact finalized head.
    pub fn reconcile(
        self,
        service: &mut PopCredentialService,
    ) -> Result<(), PopCredentialServiceError> {
        service.reconcile_finalized_tip(self.reader, self.now_epoch, self.expected_cursor)
    }

    fn reconcile_bounded(
        self,
        service: &mut PopCredentialService,
        max_reconciliations: usize,
    ) -> Result<(), PopCredentialServiceError> {
        service.reconcile_finalized_tip_bounded(
            self.reader,
            self.now_epoch,
            self.expected_cursor,
            max_reconciliations,
        )
    }
}

/// Authenticated PoP service operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopCredentialApiActionV1 {
    /// Submit a private encrypted enrollment.
    SubmitEnrollment,
    /// Read payload-free enrollment status.
    ReadEnrollmentStatus,
    /// Record a governed dual-control decision.
    ApproveEnrollment,
    /// Invoke issuer/HSM issuance.
    IssueCredential,
    /// Trigger server-resolved issuance by public request identifier.
    TriggerCredentialIssuance,
    /// HSM-sign and enqueue a governed revocation successor.
    EnqueueRevocation,
    /// Submit the next durable registry outbox operation.
    SubmitRegistryOutbox,
    /// Reconcile the next finalized registry projection.
    ReconcileRegistry,
    /// Read the current finalized registry projection.
    ReadRegistryProjection,
    /// Fetch encrypted finalized wallet delivery.
    FetchWalletDelivery,
    /// Acknowledge encrypted wallet delivery.
    AcknowledgeWalletDelivery,
    /// Import a finalized encrypted delivery into runtime wallet custody.
    ImportWalletDelivery,
    /// Synchronize a private wallet witness to finalized public state.
    SynchronizeWalletWitness,
    /// Generate a membership proof inside runtime wallet custody.
    ProveMembership,
    /// Verify and consume a membership proof.
    VerifyMembership,
}

impl PopCredentialApiActionV1 {
    /// Return the minimum authority accepted for this action.
    #[must_use]
    pub const fn minimum_request_authority(self) -> PopRequestAuthorityV1 {
        match self {
            Self::SubmitEnrollment
            | Self::ApproveEnrollment
            | Self::IssueCredential
            | Self::TriggerCredentialIssuance
            | Self::EnqueueRevocation
            | Self::AcknowledgeWalletDelivery
            | Self::ImportWalletDelivery
            | Self::SynchronizeWalletWitness
            | Self::VerifyMembership => PopRequestAuthorityV1::CallerSignedTransaction,
            Self::ReadEnrollmentStatus
            | Self::SubmitRegistryOutbox
            | Self::ReconcileRegistry
            | Self::ReadRegistryProjection
            | Self::FetchWalletDelivery
            | Self::ProveMembership => PopRequestAuthorityV1::AuthenticatedRequest,
        }
    }

    /// Return whether this action may mutate private or issuer checkpoint state
    /// only after the deployment authenticator verifies a caller signature over
    /// the exact action and request binding.
    #[must_use]
    pub const fn requires_caller_signed_transaction(self) -> bool {
        matches!(
            self.minimum_request_authority(),
            PopRequestAuthorityV1::CallerSignedTransaction
        )
    }
}

/// Authority established for one exact authenticated API request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopRequestAuthorityV1 {
    /// The request is authenticated but carries no verified caller signature.
    AuthenticatedRequest,
    /// A caller signature covers the exact action and request binding.
    CallerSignedTransaction,
}

/// Result returned by the deployment authentication adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopAuthenticatedPrincipalV1 {
    /// Payload-free digest of the authenticated principal.
    pub principal_digest: [u8; 32],
    /// Authentication expiry.
    pub expires_at_epoch: u64,
    /// Exact authority established by the deployment authenticator.
    pub request_authority: PopRequestAuthorityV1,
}

/// Runtime authentication adapter used by the PoP API facade.
///
/// The opaque credential can be a Torii bearer token, mutual-TLS exporter,
/// WebAuthn assertion, or a deployment-specific composite. Implementations
/// must bind authorization to `action` and `request_binding`, and may report
/// [`PopRequestAuthorityV1::CallerSignedTransaction`] only after verifying a
/// caller signature over both. The service never stores the credential or
/// provider error details.
pub trait PopCredentialApiAuthenticator: Send + Sync + fmt::Debug {
    /// Authenticate and authorize one request.
    fn authenticate(
        &self,
        opaque_credential: &[u8],
        action: PopCredentialApiActionV1,
        request_binding: [u8; 32],
        now_epoch: u64,
    ) -> Result<PopAuthenticatedPrincipalV1, String>;
}

/// Authenticated API facade over [`PopCredentialService`].
#[derive(Debug)]
pub struct PopCredentialApiV1 {
    authenticator: Arc<dyn PopCredentialApiAuthenticator>,
}

impl PopCredentialApiV1 {
    /// Construct an API facade with an explicit production authenticator.
    #[must_use]
    pub fn new(authenticator: Arc<dyn PopCredentialApiAuthenticator>) -> Self {
        Self { authenticator }
    }

    fn authorize(
        &self,
        opaque_credential: &[u8],
        action: PopCredentialApiActionV1,
        request_binding: [u8; 32],
        now_epoch: u64,
    ) -> Result<PopAuthenticatedPrincipalV1, PopCredentialServiceError> {
        if opaque_credential.is_empty()
            || opaque_credential.len() > POP_API_AUTHENTICATION_MAX_BYTES_V1
            || now_epoch == 0
        {
            return Err(PopCredentialServiceError::Unauthorized);
        }
        let principal = self
            .authenticator
            .authenticate(opaque_credential, action, request_binding, now_epoch)
            .map_err(|_| PopCredentialServiceError::Unauthorized)?;
        if principal.principal_digest == [0; 32] || principal.expires_at_epoch <= now_epoch {
            return Err(PopCredentialServiceError::Unauthorized);
        }
        if action.requires_caller_signed_transaction()
            && principal.request_authority != PopRequestAuthorityV1::CallerSignedTransaction
        {
            return Err(PopCredentialServiceError::Unauthorized);
        }
        Ok(principal)
    }

    /// Authenticate and submit an encrypted enrollment.
    pub fn submit_enrollment(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        canonical_enrollment: &[u8],
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        let binding = digest_domain(ENROLLMENT_ENVELOPE_DOMAIN_V1, canonical_enrollment);
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::SubmitEnrollment,
            binding,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.submit_enrollment(canonical_enrollment, now_epoch)
    }

    /// Authenticate and read payload-free enrollment status.
    pub fn enrollment_status(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        request_id: [u8; 32],
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ReadEnrollmentStatus,
            request_id,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.enrollment_status(request_id, now_epoch)
    }

    /// Authenticate and record a governed approval.
    pub fn record_approval(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        approval: PopApprovalV1,
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        let binding = approval.signature_digest()?;
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ApproveEnrollment,
            binding,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.record_approval(approval, now_epoch)
    }

    /// Authenticate and invoke HSM-backed issuance.
    pub fn issue<R: TryCryptoRng>(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        draft: PopIssuanceDraftV1,
        committed: PopCommittedRegistryContextV1<'_>,
        rng: &mut R,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        let binding = issuance_request_binding(&draft)?;
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::IssueCredential,
            binding,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.issue(draft, now_epoch, rng)
    }

    /// Authenticate a request-id-only trigger, require the runtime-resolved
    /// private draft to match it, and invoke HSM-backed issuance.
    ///
    /// This is the HTTP-safe issuance pattern: the private draft is obtained
    /// from a runtime-only provider and never accepted from or returned to the
    /// client.
    pub fn issue_resolved<R: TryCryptoRng>(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        request_id: [u8; 32],
        draft: PopIssuanceDraftV1,
        committed: PopCommittedRegistryContextV1<'_>,
        rng: &mut R,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        if draft.request_id != request_id {
            return Err(PopCredentialServiceError::InvalidIssuance);
        }
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::TriggerCredentialIssuance,
            digest_domain(ISSUE_TRIGGER_BINDING_DOMAIN_V1, &request_id),
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.issue(draft, now_epoch, rng)
    }

    /// Authenticate, HSM-sign, and durably enqueue a revocation successor.
    pub fn enqueue_revocation(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        revocations: PopRevocationListV1,
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        let canonical = encode_canonical(&revocations)?;
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::EnqueueRevocation,
            digest_domain(REVOCATION_API_BINDING_DOMAIN_V1, &canonical),
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.enqueue_revocation(revocations)
    }

    /// Authenticate and run one retry-safe registry submission step.
    pub fn submit_next(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        submitter: &dyn PopRegistrySubmitter,
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<PopOutboxSubmitOutcomeV1, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::SubmitRegistryOutbox,
            registry_submit_api_binding(service),
            now_epoch,
        )?;
        committed.reconcile(service)?;
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::SubmitRegistryOutbox,
            registry_submit_api_binding(service),
            now_epoch,
        )?;
        service.submit_next(submitter, now_epoch)
    }

    /// Authenticate and reconcile at most one finalized registry projection.
    pub fn reconcile_next(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        reader: &dyn PopFinalizedRegistryReader,
        now_epoch: u64,
    ) -> Result<bool, PopCredentialServiceError> {
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ReconcileRegistry,
            registry_projection_api_binding(REGISTRY_RECONCILE_BINDING_DOMAIN_V1, service),
            now_epoch,
        )?;
        service.reconcile_next(reader, now_epoch)
    }

    /// Authenticate and return a bounded clone of the current finalized public
    /// registry projection.
    pub fn finalized_projection(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<Option<PopFinalizedRegistryProjectionV1>, PopCredentialServiceError> {
        self.finalized_projection_bounded(
            service,
            opaque_credential,
            committed,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
    }

    /// Authenticate and return a current finalized projection after proving
    /// bounded catch-up against the authoritative reader.
    pub fn finalized_projection_bounded(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        committed: PopCommittedRegistryContextV1<'_>,
        max_reconciliations: usize,
    ) -> Result<Option<PopFinalizedRegistryProjectionV1>, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ReadRegistryProjection,
            registry_projection_api_binding(REGISTRY_PROJECTION_BINDING_DOMAIN_V1, service),
            now_epoch,
        )?;
        committed.reconcile_bounded(service, max_reconciliations)?;
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ReadRegistryProjection,
            registry_projection_api_binding(REGISTRY_PROJECTION_BINDING_DOMAIN_V1, service),
            now_epoch,
        )?;
        Ok(service.finalized_projection().cloned())
    }

    /// Authenticate and fetch encrypted finalized wallet delivery.
    pub fn wallet_delivery(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        request_id: [u8; 32],
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<Vec<u8>, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::FetchWalletDelivery,
            request_id,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.wallet_delivery(request_id)
    }

    /// Authenticate and acknowledge wallet delivery.
    pub fn acknowledge_wallet_delivery(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        request_id: [u8; 32],
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<(), PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::AcknowledgeWalletDelivery,
            request_id,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.acknowledge_wallet_delivery(request_id)
    }

    /// Authenticate and import a finalized encrypted delivery into the injected
    /// runtime wallet vault.
    pub fn import_wallet_delivery(
        &self,
        service: &mut PopCredentialService,
        vault: &PopWalletVault,
        opaque_credential: &[u8],
        request_id: [u8; 32],
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ImportWalletDelivery,
            digest_domain(WALLET_IMPORT_BINDING_DOMAIN_V1, &request_id),
            now_epoch,
        )?;
        committed.reconcile(service)?;
        let finalized = service
            .finalized_projection()
            .ok_or(PopCredentialServiceError::NotSynchronized)?;
        let delivery = service.wallet_delivery(request_id)?;
        vault.import_finalized_delivery(&delivery, finalized)
    }

    /// Authenticate and synchronize a runtime-injected private witness against
    /// the service's current finalized registry projection.
    pub fn synchronize_wallet_witness(
        &self,
        service: &mut PopCredentialService,
        vault: &PopWalletVault,
        opaque_credential: &[u8],
        credential_commitment: [u8; 32],
        witness: &PopMembershipWitnessV1,
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<(), PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::SynchronizeWalletWitness,
            digest_domain(
                WALLET_WITNESS_SYNC_BINDING_DOMAIN_V1,
                &credential_commitment,
            ),
            now_epoch,
        )?;
        committed.reconcile(service)?;
        let finalized = service
            .finalized_projection()
            .ok_or(PopCredentialServiceError::NotSynchronized)?;
        vault.synchronize_witness(credential_commitment, finalized, witness)
    }

    /// Authenticate and produce a local proof from runtime wallet custody.
    #[expect(
        clippy::too_many_arguments,
        reason = "the public facade keeps service state, wallet custody, authentication material, proof challenge, verifier domain, and epoch as explicit security bindings"
    )]
    pub fn prove_membership(
        &self,
        service: &mut PopCredentialService,
        vault: &PopWalletVault,
        opaque_credential: &[u8],
        credential_commitment: [u8; 32],
        challenge_digest: [u8; 32],
        verifier_context: &str,
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<PopMembershipProofV1, PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        bounded_clean_text(
            "verifier_context",
            verifier_context,
            POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1,
        )?;
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::ProveMembership,
            wallet_prove_api_binding(credential_commitment, challenge_digest, verifier_context),
            now_epoch,
        )?;
        committed.reconcile(service)?;
        let finalized = service
            .finalized_projection()
            .ok_or(PopCredentialServiceError::NotSynchronized)?;
        vault.prove_membership(
            credential_commitment,
            finalized,
            challenge_digest,
            verifier_context,
            now_epoch,
        )
    }

    /// Authenticate, verify, and atomically consume a proof nullifier.
    pub fn verify_membership(
        &self,
        service: &mut PopCredentialService,
        opaque_credential: &[u8],
        proof: &PopMembershipProofV1,
        challenge_digest: [u8; 32],
        verifier_context: &str,
        committed: PopCommittedRegistryContextV1<'_>,
    ) -> Result<(), PopCredentialServiceError> {
        let now_epoch = committed.now_epoch();
        bounded_clean_text(
            "verifier_context",
            verifier_context,
            POP_MEMBERSHIP_CONTEXT_MAX_BYTES_V1,
        )?;
        let mut binding_material = encode_canonical(proof)?;
        binding_material.extend_from_slice(&challenge_digest);
        binding_material.extend_from_slice(verifier_context.as_bytes());
        let binding_material = SensitiveBytesGuard::new(&mut binding_material);
        let binding = digest_domain(
            b"sorafs.pop.verify-api-request.v1",
            binding_material.as_slice(),
        );
        drop(binding_material);
        self.authorize(
            opaque_credential,
            PopCredentialApiActionV1::VerifyMembership,
            binding,
            now_epoch,
        )?;
        committed.reconcile(service)?;
        service.verify_membership(proof, challenge_digest, verifier_context, now_epoch)
    }
}

fn issuance_request_binding(
    draft: &PopIssuanceDraftV1,
) -> Result<[u8; 32], PopCredentialServiceError> {
    let mut credential_bytes = encode_canonical(&draft.credential)?;
    let credential_bytes = SensitiveBytesGuard::new(&mut credential_bytes);
    let commitment_root_bytes = encode_canonical(&draft.commitment_root)?;
    let revocation_list_bytes = encode_canonical(&draft.revocation_list)?;
    let private_witness = PopPrivateWitnessEnvelopeV1::from_witness(&draft.witness);
    let mut witness_bytes = encode_canonical(&private_witness)?;
    let witness_bytes = SensitiveBytesGuard::new(&mut witness_bytes);
    let mut material = Vec::new();
    material.extend_from_slice(&draft.request_id);
    material.extend_from_slice(credential_bytes.as_slice());
    material.extend_from_slice(&commitment_root_bytes);
    material.extend_from_slice(&revocation_list_bytes);
    material.extend_from_slice(witness_bytes.as_slice());
    let material = SensitiveBytesGuard::new(&mut material);
    Ok(digest_domain(
        b"sorafs.pop.issue-api-request.v1",
        material.as_slice(),
    ))
}

fn registry_submit_api_binding(service: &PopCredentialService) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REGISTRY_SUBMIT_BINDING_DOMAIN_V1);
    hasher.update(&service.policy.issuer_policy_digest);
    if let Some(entry) = service.state.outbox.first() {
        hasher.update(&[1]);
        hasher.update(&entry.sequence.to_le_bytes());
        hasher.update(&entry.operation.operation_digest);
    } else {
        hasher.update(&[0]);
    }
    *hasher.finalize().as_bytes()
}

fn registry_projection_api_binding(domain: &[u8], service: &PopCredentialService) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&service.policy.issuer_policy_digest);
    if let Some(projection) = service.state.finalized_projection.as_ref() {
        hasher.update(&[1]);
        hasher.update(&projection.cursor.block_height.to_le_bytes());
        hasher.update(&projection.cursor.block_hash);
    } else {
        hasher.update(&[0]);
    }
    *hasher.finalize().as_bytes()
}

fn wallet_prove_api_binding(
    credential_commitment: [u8; 32],
    challenge_digest: [u8; 32],
    verifier_context: &str,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(WALLET_PROVE_BINDING_DOMAIN_V1);
    hasher.update(&credential_commitment);
    hasher.update(&challenge_digest);
    hasher.update(
        &u64::try_from(verifier_context.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(verifier_context.as_bytes());
    *hasher.finalize().as_bytes()
}

/// Stable outbox/dead-letter failure class; external payloads are never stored.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "failure", content = "value", rename_all = "snake_case")]
pub enum PopRegistryFailureCodeV1 {
    /// Transaction transport or admission endpoint was unavailable.
    SubmissionUnavailable,
    /// Finalized ledger explicitly rejected the operation.
    LedgerRejected,
    /// Operation exhausted its bounded retry budget.
    RetryExhausted,
}

#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
struct PopRegistryOutboxEntryV1 {
    sequence: u64,
    idempotency_key: [u8; 32],
    operation: PopRegistryOperationV1,
    accepted_once: bool,
    attempt_count: u16,
    last_attempt_epoch: Option<u64>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
struct PopRegistryDeadLetterV1 {
    sequence: u64,
    operation_digest: [u8; 32],
    failure: PopRegistryFailureCodeV1,
    recorded_at_epoch: u64,
}

/// Payload-free enrollment lifecycle.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "state", content = "value", rename_all = "snake_case")]
pub enum PopEnrollmentStateV1 {
    /// Waiting for distinct governed approvals.
    AwaitingApproval,
    /// Approval quorum is satisfied.
    Approved,
    /// A governed approver rejected the enrollment.
    Rejected,
    /// HSM issuance completed and registry submission is pending.
    PendingRegistry,
    /// Registry commitment finalized; delivery is claimable.
    DeliveryReady,
    /// Wallet acknowledged durable custody.
    Delivered,
}

#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
struct PopEnrollmentRecordV1 {
    request_id: [u8; 32],
    envelope_digest: [u8; 32],
    canonical_encrypted_enrollment: Vec<u8>,
    submitted_at_epoch: u64,
    state: PopEnrollmentStateV1,
    approvals: Vec<PopApprovalV1>,
    registry_operation_digest: Option<[u8; 32]>,
    credential_commitment: Option<[u8; 32]>,
    canonical_encrypted_delivery: Option<Vec<u8>>,
}

/// Encrypted credential/witness delivery from issuer to wallet.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize)]
pub struct PopEncryptedWalletDeliveryV1 {
    /// Schema version.
    pub version: u16,
    /// Enrollment request identifier.
    pub request_id: [u8; 32],
    /// Finalized registry operation expected before import.
    pub registry_operation_digest: [u8; 32],
    /// Payload-free credential commitment.
    pub credential_commitment: [u8; 32],
    /// Encrypted signed bundle and private witness.
    pub encrypted_payload: HybridPayloadEnvelopeV1,
}

#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PopPrivateWalletDeliveryV1 {
    bundle: PopIssuedCredentialBundleV1,
    witness: PopPrivateWitnessEnvelopeV1,
}

impl fmt::Debug for PopPrivateWalletDeliveryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopPrivateWalletDeliveryV1([REDACTED])")
    }
}

impl Drop for PopPrivateWalletDeliveryV1 {
    fn drop(&mut self) {
        scrub_pop_bundle(&mut self.bundle);
        self.witness.zeroize();
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
struct PopIssuerCheckpointV1 {
    version: u16,
    issuer_policy_digest: [u8; 32],
    next_outbox_sequence: u64,
    enrollments: Vec<PopEnrollmentRecordV1>,
    outbox: Vec<PopRegistryOutboxEntryV1>,
    dead_letters: Vec<PopRegistryDeadLetterV1>,
    finalized_projection: Option<PopFinalizedRegistryProjectionV1>,
    seen_nullifiers: Vec<[u8; 32]>,
}

impl PopIssuerCheckpointV1 {
    fn empty(policy: &PopCredentialServicePolicyV1) -> Self {
        Self {
            version: POP_ISSUER_CHECKPOINT_VERSION_V1,
            issuer_policy_digest: policy.issuer_policy_digest,
            next_outbox_sequence: 1,
            enrollments: Vec::new(),
            outbox: Vec::new(),
            dead_letters: Vec::new(),
            finalized_projection: None,
            seen_nullifiers: Vec::new(),
        }
    }

    fn validate(
        &self,
        policy: &PopCredentialServicePolicyV1,
    ) -> Result<(), PopCredentialServiceError> {
        if self.version != POP_ISSUER_CHECKPOINT_VERSION_V1
            || self.issuer_policy_digest != policy.issuer_policy_digest
            || self.next_outbox_sequence == 0
            || self.enrollments.len()
                > usize::try_from(policy.max_pending_enrollments).unwrap_or(usize::MAX)
            || self.outbox.len() > usize::try_from(policy.max_outbox_entries).unwrap_or(usize::MAX)
            || self.dead_letters.len()
                > usize::try_from(policy.max_dead_letters).unwrap_or(usize::MAX)
            || self.seen_nullifiers.len()
                > usize::try_from(policy.max_seen_nullifiers).unwrap_or(usize::MAX)
        {
            return Err(PopCredentialServiceError::PoisonedCheckpoint);
        }
        let mut request_ids = BTreeSet::new();
        for record in &self.enrollments {
            if !request_ids.insert(record.request_id) {
                return Err(PopCredentialServiceError::PoisonedCheckpoint);
            }
            let envelope: PopEncryptedEnrollmentV1 = decode_canonical(
                &record.canonical_encrypted_enrollment,
                POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
                POP_SERVICE_COLLECTION_MAX_V1,
            )
            .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
            if envelope.version != POP_ENCRYPTED_ENROLLMENT_VERSION_V1
                || envelope.request_id != record.request_id
                || envelope.issuer_policy_digest != policy.issuer_policy_digest
                || envelope.issuer_id != policy.issuer_id
                || envelope.recipient_key_id != policy.enrollment_recipient_key_id
                || envelope.digest().ok() != Some(record.envelope_digest)
            {
                return Err(PopCredentialServiceError::PoisonedCheckpoint);
            }
            let issuance_fields_present = record.registry_operation_digest.is_some()
                && record.credential_commitment.is_some()
                && record.canonical_encrypted_delivery.is_some();
            let expects_issuance = matches!(
                record.state,
                PopEnrollmentStateV1::PendingRegistry
                    | PopEnrollmentStateV1::DeliveryReady
                    | PopEnrollmentStateV1::Delivered
            );
            if issuance_fields_present != expects_issuance
                || (!expects_issuance
                    && (record.registry_operation_digest.is_some()
                        || record.credential_commitment.is_some()
                        || record.canonical_encrypted_delivery.is_some()))
            {
                return Err(PopCredentialServiceError::PoisonedCheckpoint);
            }
            if let Some(canonical_delivery) = &record.canonical_encrypted_delivery {
                let delivery: PopEncryptedWalletDeliveryV1 = decode_canonical(
                    canonical_delivery,
                    POP_WALLET_DELIVERY_MAX_BYTES_V1 as u64,
                    POP_SERVICE_COLLECTION_MAX_V1,
                )
                .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
                if delivery.version != POP_WALLET_DELIVERY_VERSION_V1
                    || delivery.request_id != record.request_id
                    || Some(delivery.registry_operation_digest) != record.registry_operation_digest
                    || Some(delivery.credential_commitment) != record.credential_commitment
                {
                    return Err(PopCredentialServiceError::PoisonedCheckpoint);
                }
            }
            let mut previous_signer: Option<&str> = None;
            for approval in &record.approvals {
                if previous_signer.is_some_and(|value| value >= approval.signer_id.as_str()) {
                    return Err(PopCredentialServiceError::PoisonedCheckpoint);
                }
                previous_signer = Some(approval.signer_id.as_str());
            }
        }
        let mut sequences = BTreeSet::new();
        let mut idempotency_keys = BTreeSet::new();
        let mut previous_sequence = 0_u64;
        for entry in &self.outbox {
            if entry.sequence == 0
                || entry.sequence <= previous_sequence
                || entry.sequence >= self.next_outbox_sequence
                || entry.attempt_count > policy.max_submission_attempts
                || !sequences.insert(entry.sequence)
                || !idempotency_keys.insert(entry.idempotency_key)
                || entry.idempotency_key
                    != registry_idempotency_key(entry.sequence, entry.operation.operation_digest)
                || entry
                    .operation
                    .validate_for_policy(policy.issuer_policy_digest)
                    .is_err()
            {
                return Err(PopCredentialServiceError::PoisonedCheckpoint);
            }
            previous_sequence = entry.sequence;
        }
        let mut previous_dead_letter_sequence = 0_u64;
        for entry in &self.dead_letters {
            if entry.sequence == 0
                || entry.sequence <= previous_dead_letter_sequence
                || entry.sequence >= self.next_outbox_sequence
                || entry.operation_digest == [0; 32]
                || entry.recorded_at_epoch == 0
                || !sequences.insert(entry.sequence)
            {
                return Err(PopCredentialServiceError::PoisonedCheckpoint);
            }
            previous_dead_letter_sequence = entry.sequence;
        }
        let mut nullifiers = BTreeSet::new();
        if self
            .seen_nullifiers
            .iter()
            .any(|value| *value == [0; 32] || !nullifiers.insert(*value))
        {
            return Err(PopCredentialServiceError::PoisonedCheckpoint);
        }
        if let Some(projection) = &self.finalized_projection {
            validate_projection(None, projection, policy)?;
        }
        Ok(())
    }
}

/// Payload-free enrollment status returned by service APIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopEnrollmentStatusV1 {
    /// Request identifier.
    pub request_id: [u8; 32],
    /// Current lifecycle state.
    pub state: PopEnrollmentStateV1,
    /// Number of currently valid distinct approvals.
    pub active_approval_count: u8,
    /// Registry operation digest, once issued.
    pub registry_operation_digest: Option<[u8; 32]>,
}

/// Submission worker outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopOutboxSubmitOutcomeV1 {
    /// No pending operation was available.
    Idle,
    /// Operation was accepted or idempotently recognized by the submitter.
    Submitted {
        /// Digest of the accepted registry operation.
        operation_digest: [u8; 32],
    },
    /// Operation remains queued for retry.
    RetryScheduled {
        /// Digest of the registry operation retained for retry.
        operation_digest: [u8; 32],
    },
    /// Operation exhausted retries and entered the durable dead letter.
    DeadLettered {
        /// Digest of the registry operation moved to dead letter.
        operation_digest: [u8; 32],
    },
}

/// Issuer, reconciliation, and verifier service.
pub struct PopCredentialService {
    policy: PopCredentialServicePolicyV1,
    checkpoint_path: PathBuf,
    enrollment_recipient: Arc<dyn PopEnrollmentRecipientV1>,
    hsm: Arc<dyn PopIssuerHsm>,
    state: PopIssuerCheckpointV1,
    checkpoint_writer: PopCheckpointWriter,
    mutations_disabled: bool,
}

impl fmt::Debug for PopCredentialService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopCredentialService")
            .field("issuer_id", &self.policy.issuer_id)
            .field("checkpoint_path", &self.checkpoint_path)
            .field("runtime_secrets", &"[REDACTED]")
            .finish()
    }
}

impl PopCredentialService {
    /// Open or initialize a fail-closed issuer service.
    pub fn open(
        data_dir: impl AsRef<Path>,
        policy: PopCredentialServicePolicyV1,
        enrollment_recipient: Arc<dyn PopEnrollmentRecipientV1>,
        hsm: Arc<dyn PopIssuerHsm>,
    ) -> Result<Self, PopCredentialServiceError> {
        policy.validate()?;
        bounded_production_runtime_handle(
            "enrollment_recipient_key_id",
            enrollment_recipient.key_id(),
        )?;
        if enrollment_recipient.key_id() != policy.enrollment_recipient_key_id
            || enrollment_recipient.public_key_digest() == [0; 32]
        {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryMismatch);
        }
        if hsm.key_id() != policy.issuer_hsm_key_id || hsm.public_key() != policy.issuer_public_key
        {
            return Err(PopCredentialServiceError::HsmPolicyMismatch);
        }
        let checkpoint_path = data_dir.as_ref().join(POP_ISSUER_CHECKPOINT_FILE_V1);
        let state = match read_local_checkpoint_bounded(
            &checkpoint_path,
            POP_ISSUER_CHECKPOINT_MAX_BYTES_V1,
        )
        .map_err(|_| PopCredentialServiceError::CheckpointIo)?
        {
            Some(bytes) => decode_canonical(
                &bytes,
                POP_ISSUER_CHECKPOINT_MAX_BYTES_V1,
                POP_SERVICE_COLLECTION_MAX_V1,
            )
            .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?,
            None => {
                let state = PopIssuerCheckpointV1::empty(&policy);
                persist_checkpoint(&checkpoint_path, &state, production_checkpoint_write)
                    .map_err(|failure| failure.error)?;
                state
            }
        };
        state.validate(&policy)?;
        Ok(Self {
            policy,
            checkpoint_path,
            enrollment_recipient,
            hsm,
            state,
            checkpoint_writer: production_checkpoint_write,
            mutations_disabled: false,
        })
    }

    fn transact<T>(
        &mut self,
        mutation: impl FnOnce(&mut PopIssuerCheckpointV1) -> Result<T, PopCredentialServiceError>,
    ) -> Result<T, PopCredentialServiceError> {
        if self.mutations_disabled {
            return Err(PopCredentialServiceError::CheckpointDurabilityUncertain);
        }
        let mut next = self.state.clone();
        let output = mutation(&mut next)?;
        next.validate(&self.policy)?;
        match persist_checkpoint(&self.checkpoint_path, &next, self.checkpoint_writer) {
            Ok(()) => {
                self.state = next;
                Ok(output)
            }
            Err(failure) if failure.committed => {
                self.state = next;
                self.mutations_disabled = true;
                Err(PopCredentialServiceError::CheckpointDurabilityUncertain)
            }
            Err(failure) => Err(failure.error),
        }
    }

    /// Submit an exact canonical encrypted enrollment.
    pub fn submit_enrollment(
        &mut self,
        canonical_enrollment: &[u8],
        now_epoch: u64,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        if now_epoch == 0 {
            return Err(PopCredentialServiceError::InvalidInput { field: "now_epoch" });
        }
        let envelope: PopEncryptedEnrollmentV1 = decode_canonical(
            canonical_enrollment,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        if envelope.version != POP_ENCRYPTED_ENROLLMENT_VERSION_V1
            || envelope.issuer_policy_digest != self.policy.issuer_policy_digest
            || envelope.issuer_id != self.policy.issuer_id
            || envelope.recipient_key_id != self.policy.enrollment_recipient_key_id
        {
            return Err(PopCredentialServiceError::WrongPolicy);
        }
        let canonical = envelope.canonical_bytes()?;
        if canonical != canonical_enrollment {
            return Err(PopCredentialServiceError::Codec);
        }
        let envelope_digest = envelope.digest()?;
        if let Some(existing) = self
            .state
            .enrollments
            .iter()
            .find(|record| record.request_id == envelope.request_id)
        {
            if existing.envelope_digest == envelope_digest {
                return Ok(self.status_for_record(existing, now_epoch));
            }
            return Err(PopCredentialServiceError::EnrollmentReplay);
        }
        let mut plaintext = self
            .enrollment_recipient
            .open_enrollment(&envelope.encrypted_payload, &envelope.aad()?)
            .map_err(map_enrollment_recipient_error)?;
        let plaintext = SensitiveBytesGuard::new(&mut plaintext);
        let private: PopPrivateEnrollmentV1 = decode_canonical(
            plaintext.as_slice(),
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .map_err(|_| PopCredentialServiceError::InvalidEnrollment)?;
        drop(plaintext);
        private.validate()?;
        if private.request.request_id != envelope.request_id
            || private.request.expires_at_epoch <= now_epoch
        {
            return Err(PopCredentialServiceError::InvalidEnrollment);
        }
        if self.state.enrollments.len()
            >= usize::try_from(self.policy.max_pending_enrollments).unwrap_or(usize::MAX)
        {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        let record = PopEnrollmentRecordV1 {
            request_id: envelope.request_id,
            envelope_digest,
            canonical_encrypted_enrollment: canonical,
            submitted_at_epoch: now_epoch,
            state: PopEnrollmentStateV1::AwaitingApproval,
            approvals: Vec::new(),
            registry_operation_digest: None,
            credential_commitment: None,
            canonical_encrypted_delivery: None,
        };
        self.transact(|state| {
            state.enrollments.push(record.clone());
            state.enrollments.sort_by_key(|entry| entry.request_id);
            Ok(())
        })?;
        Ok(self.status_for_record(&record, now_epoch))
    }

    /// Record one signed dual-control approval or rejection.
    pub fn record_approval(
        &mut self,
        approval: PopApprovalV1,
        now_epoch: u64,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        let index = self
            .state
            .enrollments
            .binary_search_by_key(&approval.request_id, |record| record.request_id)
            .map_err(|_| PopCredentialServiceError::EnrollmentNotFound)?;
        let record = &self.state.enrollments[index];
        if !matches!(
            record.state,
            PopEnrollmentStateV1::AwaitingApproval | PopEnrollmentStateV1::Approved
        ) {
            return Err(PopCredentialServiceError::InvalidState);
        }
        approval.validate(
            &self.policy,
            record.request_id,
            record.envelope_digest,
            now_epoch,
        )?;
        if record
            .approvals
            .iter()
            .any(|existing| existing.signer_id == approval.signer_id)
        {
            return Err(PopCredentialServiceError::DuplicateApproval);
        }
        let decision = approval.decision;
        let policy = self.policy.clone();
        self.transact(|state| {
            let record = &mut state.enrollments[index];
            record.approvals.push(approval);
            record
                .approvals
                .sort_by(|left, right| left.signer_id.cmp(&right.signer_id));
            let active_count = record
                .approvals
                .iter()
                .filter(|approval| {
                    approval.decision == PopApprovalDecisionV1::Approve
                        && approval
                            .validate(
                                &policy,
                                record.request_id,
                                record.envelope_digest,
                                now_epoch,
                            )
                            .is_ok()
                })
                .count();
            record.state = if decision == PopApprovalDecisionV1::Reject {
                PopEnrollmentStateV1::Rejected
            } else if active_count >= usize::from(policy.approval_quorum) {
                PopEnrollmentStateV1::Approved
            } else {
                PopEnrollmentStateV1::AwaitingApproval
            };
            Ok(())
        })?;
        Ok(self.status_for_record(&self.state.enrollments[index], now_epoch))
    }

    fn active_approval_count(&self, record: &PopEnrollmentRecordV1, now_epoch: u64) -> u8 {
        let count = record
            .approvals
            .iter()
            .filter(|approval| {
                approval.decision == PopApprovalDecisionV1::Approve
                    && approval
                        .validate(
                            &self.policy,
                            record.request_id,
                            record.envelope_digest,
                            now_epoch,
                        )
                        .is_ok()
            })
            .count();
        u8::try_from(count).unwrap_or(u8::MAX)
    }

    fn status_for_record(
        &self,
        record: &PopEnrollmentRecordV1,
        now_epoch: u64,
    ) -> PopEnrollmentStatusV1 {
        PopEnrollmentStatusV1 {
            request_id: record.request_id,
            state: record.state,
            active_approval_count: self.active_approval_count(record, now_epoch),
            registry_operation_digest: record.registry_operation_digest,
        }
    }

    /// Return payload-free enrollment status.
    pub fn enrollment_status(
        &self,
        request_id: [u8; 32],
        now_epoch: u64,
    ) -> Result<PopEnrollmentStatusV1, PopCredentialServiceError> {
        let record = self
            .state
            .enrollments
            .binary_search_by_key(&request_id, |entry| entry.request_id)
            .ok()
            .and_then(|index| self.state.enrollments.get(index))
            .ok_or(PopCredentialServiceError::EnrollmentNotFound)?;
        Ok(self.status_for_record(record, now_epoch))
    }

    /// HSM-sign an approved issuance and atomically enqueue its payload-free
    /// registry operation together with encrypted wallet delivery.
    pub fn issue<R: TryCryptoRng>(
        &mut self,
        draft: PopIssuanceDraftV1,
        now_epoch: u64,
        rng: &mut R,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let index = self
            .state
            .enrollments
            .binary_search_by_key(&draft.request_id, |record| record.request_id)
            .map_err(|_| PopCredentialServiceError::EnrollmentNotFound)?;
        if self.state.enrollments[index].state != PopEnrollmentStateV1::Approved
            || self.active_approval_count(&self.state.enrollments[index], now_epoch)
                < self.policy.approval_quorum
        {
            return Err(PopCredentialServiceError::ApprovalQuorum);
        }
        let envelope: PopEncryptedEnrollmentV1 = decode_canonical(
            &self.state.enrollments[index].canonical_encrypted_enrollment,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        let mut enrollment_bytes = self
            .enrollment_recipient
            .open_enrollment(&envelope.encrypted_payload, &envelope.aad()?)
            .map_err(map_enrollment_recipient_error)?;
        let enrollment_bytes = SensitiveBytesGuard::new(&mut enrollment_bytes);
        let enrollment: PopPrivateEnrollmentV1 = decode_canonical(
            enrollment_bytes.as_slice(),
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .map_err(|_| PopCredentialServiceError::InvalidEnrollment)?;
        drop(enrollment_bytes);
        let wallet_public_key = enrollment.validate()?;
        validate_issuance_draft(&draft, &enrollment, &self.policy, now_epoch)?;
        let bundle = PrivateBundleGuard::new(sign_bundle_with_hsm(
            draft.credential.clone(),
            draft.commitment_root.clone(),
            draft.revocation_list.clone(),
            self.hsm.as_ref(),
        )?);
        let signed_bundle = bundle.as_ref()?;
        let mut canonical_credential = encode_canonical(&signed_bundle.credential)?;
        let canonical_credential = SensitiveBytesGuard::new(&mut canonical_credential);
        let credential_commitment =
            pop_credential_payload_commitment_v1(canonical_credential.as_slice());
        drop(canonical_credential);
        let commitment = PopCredentialCommitmentV1 {
            credential_commitment,
            revocation_nonce_commitment: pop_revocation_nonce_commitment_v1(
                signed_bundle.credential.revocation_nonce,
            ),
            commitment_root: signed_bundle.commitment_root.root_digest,
            commitment_tree_version: signed_bundle.commitment_root.tree_version,
            revocation_list_version: signed_bundle.revocation_list.list_version,
            issued_at_epoch: signed_bundle.credential.issued_at_epoch,
            expires_at_epoch: signed_bundle.credential.expires_at_epoch,
        };
        let batch = PopCredentialCommitmentBatchV1 {
            version: POP_CREDENTIAL_COMMITMENT_BATCH_VERSION_V1,
            issuer_policy_digest: self.policy.issuer_policy_digest,
            commitment_root_payload: encode_canonical(&signed_bundle.commitment_root)?,
            revocation_list_payload: encode_canonical(&signed_bundle.revocation_list)?,
            commitments: vec![commitment],
        };
        batch
            .validate()
            .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
        let operation =
            PopRegistryOperationV1::new(PopRegistryOperationKindV1::CommitCredentialBatch {
                canonical_batch: encode_canonical(&batch)?,
            })?;
        let private_delivery = PopPrivateWalletDeliveryV1 {
            bundle: bundle.into_inner()?,
            witness: PopPrivateWitnessEnvelopeV1::from_witness(&draft.witness),
        };
        let mut private_delivery_bytes = encode_canonical(&private_delivery)?;
        let private_delivery_bytes = SensitiveBytesGuard::new(&mut private_delivery_bytes);
        let delivery_aad = wallet_delivery_aad(
            draft.request_id,
            operation.operation_digest,
            credential_commitment,
        )?;
        let encrypted_payload = encrypt_payload(
            private_delivery_bytes.as_slice(),
            &delivery_aad,
            &wallet_public_key,
            rng,
        )
        .map_err(|_| PopCredentialServiceError::Encryption)?;
        drop(private_delivery_bytes);
        let delivery = PopEncryptedWalletDeliveryV1 {
            version: POP_WALLET_DELIVERY_VERSION_V1,
            request_id: draft.request_id,
            registry_operation_digest: operation.operation_digest,
            credential_commitment,
            encrypted_payload,
        };
        let canonical_delivery = encode_canonical(&delivery)?;
        if canonical_delivery.len() > POP_WALLET_DELIVERY_MAX_BYTES_V1 {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        if self.state.outbox.len()
            >= usize::try_from(self.policy.max_outbox_entries).unwrap_or(usize::MAX)
        {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        let sequence = self.state.next_outbox_sequence;
        let idempotency_key = registry_idempotency_key(sequence, operation.operation_digest);
        let operation_digest = operation.operation_digest;
        self.transact(|state| {
            state.next_outbox_sequence = state
                .next_outbox_sequence
                .checked_add(1)
                .ok_or(PopCredentialServiceError::ResourceExhausted)?;
            state.outbox.push(PopRegistryOutboxEntryV1 {
                sequence,
                idempotency_key,
                operation,
                accepted_once: false,
                attempt_count: 0,
                last_attempt_epoch: None,
            });
            let record = &mut state.enrollments[index];
            record.registry_operation_digest = Some(operation_digest);
            record.credential_commitment = Some(credential_commitment);
            record.canonical_encrypted_delivery = Some(canonical_delivery);
            record.state = PopEnrollmentStateV1::PendingRegistry;
            Ok(())
        })?;
        Ok(operation_digest)
    }

    /// HSM-sign and enqueue a strict successor revocation snapshot.
    pub fn enqueue_revocation(
        &mut self,
        revocations: PopRevocationListV1,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let projection = self
            .state
            .finalized_projection
            .as_ref()
            .ok_or(PopCredentialServiceError::NotSynchronized)?;
        let current: PopRevocationListV1 = decode_canonical(
            &projection.canonical_revocation_list,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        if revocations.list_version != current.list_version.saturating_add(1)
            || revocations.commitment_root != current.commitment_root
            || revocations.entries.len() < current.entries.len()
            || current.entries.iter().any(|previous| {
                revocations
                    .entries
                    .binary_search_by_key(&previous.nonce, |entry| entry.nonce)
                    .ok()
                    .and_then(|index| revocations.entries.get(index))
                    != Some(previous)
            })
        {
            return Err(PopCredentialServiceError::RootRollback);
        }
        let signed = sign_revocation_with_hsm(revocations, self.hsm.as_ref())?;
        let operation =
            PopRegistryOperationV1::new(PopRegistryOperationKindV1::PublishRevocationList {
                canonical_revocation_list: encode_canonical(&signed)?,
                issuer_policy_digest: self.policy.issuer_policy_digest,
            })?;
        if self.state.outbox.len()
            >= usize::try_from(self.policy.max_outbox_entries).unwrap_or(usize::MAX)
        {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        let sequence = self.state.next_outbox_sequence;
        let digest = operation.operation_digest;
        self.transact(|state| {
            state.next_outbox_sequence = state
                .next_outbox_sequence
                .checked_add(1)
                .ok_or(PopCredentialServiceError::ResourceExhausted)?;
            state.outbox.push(PopRegistryOutboxEntryV1 {
                sequence,
                idempotency_key: registry_idempotency_key(sequence, digest),
                operation,
                accepted_once: false,
                attempt_count: 0,
                last_attempt_epoch: None,
            });
            Ok(())
        })?;
        Ok(digest)
    }

    /// Submit the oldest outbox entry. Failures retain only a stable code.
    pub fn submit_next(
        &mut self,
        submitter: &dyn PopRegistrySubmitter,
        now_epoch: u64,
    ) -> Result<PopOutboxSubmitOutcomeV1, PopCredentialServiceError> {
        let Some(entry) = self.state.outbox.first().cloned() else {
            return Ok(PopOutboxSubmitOutcomeV1::Idle);
        };
        let digest = entry.operation.operation_digest;
        match submitter.submit(entry.idempotency_key, &entry.operation) {
            Ok(()) => {
                self.transact(|state| {
                    state.outbox[0].accepted_once = true;
                    state.outbox[0].last_attempt_epoch = Some(now_epoch);
                    Ok(())
                })?;
                Ok(PopOutboxSubmitOutcomeV1::Submitted {
                    operation_digest: digest,
                })
            }
            Err(_) => {
                let attempts = entry.attempt_count.saturating_add(1);
                if attempts >= self.policy.max_submission_attempts && !entry.accepted_once {
                    let policy = self.policy.clone();
                    self.transact(|state| {
                        let removed = state.outbox.remove(0);
                        push_dead_letter(
                            state,
                            &policy,
                            removed.sequence,
                            removed.operation.operation_digest,
                            PopRegistryFailureCodeV1::RetryExhausted,
                            now_epoch,
                        )
                    })?;
                    Ok(PopOutboxSubmitOutcomeV1::DeadLettered {
                        operation_digest: digest,
                    })
                } else {
                    self.transact(|state| {
                        state.outbox[0].attempt_count = attempts;
                        state.outbox[0].last_attempt_epoch = Some(now_epoch);
                        Ok(())
                    })?;
                    Ok(PopOutboxSubmitOutcomeV1::RetryScheduled {
                        operation_digest: digest,
                    })
                }
            }
        }
    }

    /// Consume one next finalized projection and reconcile the outbox.
    pub fn reconcile_next(
        &mut self,
        reader: &dyn PopFinalizedRegistryReader,
        now_epoch: u64,
    ) -> Result<bool, PopCredentialServiceError> {
        let current_cursor = self
            .state
            .finalized_projection
            .as_ref()
            .map(|projection| projection.cursor);
        let Some(next) = reader
            .next_after(current_cursor)
            .map_err(|_| PopCredentialServiceError::RegistryUnavailable)?
        else {
            return Ok(false);
        };
        validate_projection(
            self.state.finalized_projection.as_ref(),
            &next,
            &self.policy,
        )?;
        let policy = self.policy.clone();
        self.transact(|state| {
            for digest in &next.rejected_operation_digests {
                if let Some(index) = state
                    .outbox
                    .iter()
                    .position(|entry| entry.operation.operation_digest == *digest)
                {
                    let removed = state.outbox.remove(index);
                    push_dead_letter(
                        state,
                        &policy,
                        removed.sequence,
                        *digest,
                        PopRegistryFailureCodeV1::LedgerRejected,
                        now_epoch,
                    )?;
                }
            }
            for digest in &next.committed_operation_digests {
                state
                    .outbox
                    .retain(|entry| entry.operation.operation_digest != *digest);
                state
                    .dead_letters
                    .retain(|entry| entry.operation_digest != *digest);
                for enrollment in &mut state.enrollments {
                    if enrollment.registry_operation_digest == Some(*digest)
                        && enrollment.state == PopEnrollmentStateV1::PendingRegistry
                    {
                        enrollment.state = PopEnrollmentStateV1::DeliveryReady;
                    }
                }
            }
            state.finalized_projection = Some(next);
            Ok(())
        })?;
        Ok(true)
    }

    /// Reconcile finalized successors until the durable projection reaches
    /// the exact independently sampled authoritative head.
    ///
    /// A reader error or a successor stream that exhausts the hard collection
    /// bound fails closed. Already validated successors remain durably applied,
    /// so a later retry resumes from the exact persisted cursor.
    pub fn reconcile_finalized_tip(
        &mut self,
        reader: &dyn PopFinalizedRegistryReader,
        now_epoch: u64,
        expected_cursor: PopFinalizedCursorV1,
    ) -> Result<(), PopCredentialServiceError> {
        self.reconcile_finalized_tip_bounded(
            reader,
            now_epoch,
            expected_cursor,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
    }

    fn reconcile_finalized_tip_bounded(
        &mut self,
        reader: &dyn PopFinalizedRegistryReader,
        now_epoch: u64,
        expected_cursor: PopFinalizedCursorV1,
        max_reconciliations: usize,
    ) -> Result<(), PopCredentialServiceError> {
        let cursor_matches = |service: &Self| {
            service
                .state
                .finalized_projection
                .as_ref()
                .is_some_and(|projection| projection.cursor == expected_cursor)
        };
        if cursor_matches(self) {
            return Ok(());
        }
        if self
            .state
            .finalized_projection
            .as_ref()
            .is_some_and(|projection| {
                projection.cursor.block_height >= expected_cursor.block_height
            })
        {
            return Err(PopCredentialServiceError::RegistryUnavailable);
        }
        for _ in 0..max_reconciliations {
            if !self.reconcile_next(reader, now_epoch)? {
                return Err(PopCredentialServiceError::RegistryUnavailable);
            }
            if cursor_matches(self) {
                return Ok(());
            }
            if self
                .state
                .finalized_projection
                .as_ref()
                .is_some_and(|projection| {
                    projection.cursor.block_height >= expected_cursor.block_height
                })
            {
                return Err(PopCredentialServiceError::RegistryUnavailable);
            }
        }
        Err(PopCredentialServiceError::RegistryUnavailable)
    }

    /// Fetch the stable encrypted wallet delivery after ledger finalization.
    pub fn wallet_delivery(
        &self,
        request_id: [u8; 32],
    ) -> Result<Vec<u8>, PopCredentialServiceError> {
        let record = self
            .state
            .enrollments
            .binary_search_by_key(&request_id, |entry| entry.request_id)
            .ok()
            .and_then(|index| self.state.enrollments.get(index))
            .ok_or(PopCredentialServiceError::EnrollmentNotFound)?;
        if !matches!(
            record.state,
            PopEnrollmentStateV1::DeliveryReady | PopEnrollmentStateV1::Delivered
        ) {
            return Err(PopCredentialServiceError::InvalidState);
        }
        record
            .canonical_encrypted_delivery
            .clone()
            .ok_or(PopCredentialServiceError::PoisonedCheckpoint)
    }

    /// Record wallet acknowledgement without deleting the recoverable ciphertext.
    pub fn acknowledge_wallet_delivery(
        &mut self,
        request_id: [u8; 32],
    ) -> Result<(), PopCredentialServiceError> {
        let index = self
            .state
            .enrollments
            .binary_search_by_key(&request_id, |entry| entry.request_id)
            .map_err(|_| PopCredentialServiceError::EnrollmentNotFound)?;
        if !matches!(
            self.state.enrollments[index].state,
            PopEnrollmentStateV1::DeliveryReady | PopEnrollmentStateV1::Delivered
        ) {
            return Err(PopCredentialServiceError::InvalidState);
        }
        self.transact(|state| {
            state.enrollments[index].state = PopEnrollmentStateV1::Delivered;
            Ok(())
        })
    }

    /// Verify a proof against the exact finalized roots and atomically consume
    /// its nullifier before returning success.
    pub fn verify_membership(
        &mut self,
        proof: &PopMembershipProofV1,
        challenge_digest: [u8; 32],
        verifier_context: &str,
        now_epoch: u64,
    ) -> Result<(), PopCredentialServiceError> {
        let projection = self
            .state
            .finalized_projection
            .as_ref()
            .ok_or(PopCredentialServiceError::NotSynchronized)?;
        if projection
            .revoked_issuer_public_keys
            .binary_search(&self.policy.issuer_public_key)
            .is_ok()
        {
            return Err(PopCredentialServiceError::SignerRevoked);
        }
        let root: PopCommitmentRootV1 = decode_canonical(
            &projection.canonical_commitment_root,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        let revocations: PopRevocationListV1 = decode_canonical(
            &projection.canonical_revocation_list,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        verify_pop_membership_proof_v1(
            proof,
            &root,
            &revocations,
            challenge_digest,
            verifier_context,
            now_epoch,
            &self.state.seen_nullifiers,
        )
        .map_err(|_| PopCredentialServiceError::InvalidMembershipProof)?;
        self.consume_verified_nullifier(proof.nullifier)
    }

    fn consume_verified_nullifier(
        &mut self,
        nullifier: [u8; 32],
    ) -> Result<(), PopCredentialServiceError> {
        nonzero_digest("membership_nullifier", nullifier)?;
        let max_seen_nullifiers =
            usize::try_from(self.policy.max_seen_nullifiers).unwrap_or(usize::MAX);
        self.transact(|state| {
            if state.seen_nullifiers.contains(&nullifier) {
                return Err(PopCredentialServiceError::ReplayedProof);
            }
            if state.seen_nullifiers.len() >= max_seen_nullifiers {
                return Err(PopCredentialServiceError::ResourceExhausted);
            }
            state.seen_nullifiers.push(nullifier);
            state.seen_nullifiers.sort_unstable();
            Ok(())
        })
    }

    /// Return the exact validated policy used to construct this service.
    #[must_use]
    pub fn policy(&self) -> &PopCredentialServicePolicyV1 {
        &self.policy
    }

    /// Return the current finalized public projection.
    #[must_use]
    pub fn finalized_projection(&self) -> Option<&PopFinalizedRegistryProjectionV1> {
        self.state.finalized_projection.as_ref()
    }
}

#[derive(Debug, Clone, Copy)]
struct PopCheckpointPersistFailure {
    error: PopCredentialServiceError,
    committed: bool,
}

type PopCheckpointWriter = fn(&Path, &[u8]) -> Result<(), PopCheckpointPersistFailure>;

fn production_checkpoint_write(
    path: &Path,
    bytes: &[u8],
) -> Result<(), PopCheckpointPersistFailure> {
    write_local_private_checkpoint_atomic(path, bytes).map_err(|error| {
        PopCheckpointPersistFailure {
            error: if error.committed {
                PopCredentialServiceError::CheckpointDurabilityUncertain
            } else {
                PopCredentialServiceError::CheckpointIo
            },
            committed: error.committed,
        }
    })
}

fn persist_checkpoint(
    path: &Path,
    state: &PopIssuerCheckpointV1,
    writer: PopCheckpointWriter,
) -> Result<(), PopCheckpointPersistFailure> {
    let bytes = encode_canonical(state).map_err(|error| PopCheckpointPersistFailure {
        error,
        committed: false,
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > POP_ISSUER_CHECKPOINT_MAX_BYTES_V1 {
        return Err(PopCheckpointPersistFailure {
            error: PopCredentialServiceError::ResourceExhausted,
            committed: false,
        });
    }
    writer(path, &bytes)
}

fn push_dead_letter(
    state: &mut PopIssuerCheckpointV1,
    policy: &PopCredentialServicePolicyV1,
    sequence: u64,
    operation_digest: [u8; 32],
    failure: PopRegistryFailureCodeV1,
    now_epoch: u64,
) -> Result<(), PopCredentialServiceError> {
    if state.dead_letters.len() >= usize::try_from(policy.max_dead_letters).unwrap_or(usize::MAX) {
        return Err(PopCredentialServiceError::ResourceExhausted);
    }
    state.dead_letters.push(PopRegistryDeadLetterV1 {
        sequence,
        operation_digest,
        failure,
        recorded_at_epoch: now_epoch,
    });
    state.dead_letters.sort_by_key(|entry| entry.sequence);
    Ok(())
}

fn registry_idempotency_key(sequence: u64, operation_digest: [u8; 32]) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(40);
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(&operation_digest);
    digest_domain(REGISTRY_IDEMPOTENCY_DOMAIN_V1, &bytes)
}

fn validate_issuance_draft(
    draft: &PopIssuanceDraftV1,
    enrollment: &PopPrivateEnrollmentV1,
    policy: &PopCredentialServicePolicyV1,
    now_epoch: u64,
) -> Result<(), PopCredentialServiceError> {
    if draft.request_id != enrollment.request.request_id
        || draft.credential.holder_commitment != enrollment.holder_commitment
        || draft.credential.eligibility_class != enrollment.request.requested_class
        || draft.credential.issuer_id != policy.issuer_id
        || draft.commitment_root.issuer_id != policy.issuer_id
        || draft.revocation_list.issuer_id != policy.issuer_id
        || draft.credential.issued_at_epoch > now_epoch
        || draft.credential.expires_at_epoch <= now_epoch
    {
        return Err(PopCredentialServiceError::InvalidIssuance);
    }
    let requested: BTreeSet<&str> = enrollment
        .request
        .requested_attributes
        .iter()
        .map(String::as_str)
        .collect();
    let supplied: BTreeSet<&str> = draft
        .credential
        .attributes
        .iter()
        .map(|attribute| attribute.key.as_str())
        .collect();
    if requested != supplied {
        return Err(PopCredentialServiceError::InvalidIssuance);
    }
    draft
        .witness
        .validate()
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    Ok(())
}

fn pop_signature(public_key: [u8; 32], signature: [u8; 64]) -> PopSignatureV1 {
    PopSignatureV1 {
        algorithm: PopSignatureAlgorithmV1::Ed25519,
        public_key: public_key.to_vec(),
        signature: signature.to_vec(),
    }
}

fn empty_pop_signature(public_key: [u8; 32]) -> PopSignatureV1 {
    PopSignatureV1 {
        algorithm: PopSignatureAlgorithmV1::Ed25519,
        public_key: public_key.to_vec(),
        signature: Vec::new(),
    }
}

fn sign_bundle_with_hsm(
    credential: PopCredentialV1,
    root: PopCommitmentRootV1,
    revocations: PopRevocationListV1,
    hsm: &dyn PopIssuerHsm,
) -> Result<PopIssuedCredentialBundleV1, PopCredentialServiceError> {
    let public_key = hsm.public_key();
    let mut bundle = PrivateBundleGuard::new(PopIssuedCredentialBundleV1 {
        version: sorafs_manifest::POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1,
        credential,
        commitment_root: root,
        revocation_list: revocations,
    });
    bundle.as_mut()?.credential.issuer_signature = empty_pop_signature(public_key);
    let credential_digest = pop_credential_signature_digest_v1(&bundle.as_ref()?.credential)
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    bundle.as_mut()?.credential.issuer_signature = pop_signature(
        public_key,
        hsm.sign_digest(credential_digest)
            .map_err(|_| PopCredentialServiceError::HsmUnavailable)?,
    );
    bundle.as_mut()?.commitment_root.publisher_signature = empty_pop_signature(public_key);
    let root_digest = pop_commitment_root_signature_digest_v1(&bundle.as_ref()?.commitment_root)
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    bundle.as_mut()?.commitment_root.publisher_signature = pop_signature(
        public_key,
        hsm.sign_digest(root_digest)
            .map_err(|_| PopCredentialServiceError::HsmUnavailable)?,
    );
    bundle.as_mut()?.revocation_list.publisher_signature = empty_pop_signature(public_key);
    let revocation_digest =
        pop_revocation_list_signature_digest_v1(&bundle.as_ref()?.revocation_list)
            .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    bundle.as_mut()?.revocation_list.publisher_signature = pop_signature(
        public_key,
        hsm.sign_digest(revocation_digest)
            .map_err(|_| PopCredentialServiceError::HsmUnavailable)?,
    );
    bundle
        .as_ref()?
        .validate()
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    bundle.into_inner()
}

fn sign_revocation_with_hsm(
    mut revocations: PopRevocationListV1,
    hsm: &dyn PopIssuerHsm,
) -> Result<PopRevocationListV1, PopCredentialServiceError> {
    let public_key = hsm.public_key();
    revocations.publisher_signature = empty_pop_signature(public_key);
    let digest = pop_revocation_list_signature_digest_v1(&revocations)
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    revocations.publisher_signature = pop_signature(
        public_key,
        hsm.sign_digest(digest)
            .map_err(|_| PopCredentialServiceError::HsmUnavailable)?,
    );
    verify_pop_revocation_list_signature_v1(&revocations)
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
    Ok(revocations)
}

fn validate_projection(
    current: Option<&PopFinalizedRegistryProjectionV1>,
    next: &PopFinalizedRegistryProjectionV1,
    policy: &PopCredentialServicePolicyV1,
) -> Result<(), PopCredentialServiceError> {
    if next.version != POP_FINALIZED_REGISTRY_PROJECTION_VERSION_V1
        || next.cursor.block_height == 0
        || next.cursor.block_hash == [0; 32]
        || next.issuer_policy_digest != policy.issuer_policy_digest
    {
        return Err(PopCredentialServiceError::WrongPolicy);
    }
    match current {
        None => {
            if (next.cursor.block_height == 1 && next.previous_block_hash.is_some())
                || (next.cursor.block_height > 1
                    && next
                        .previous_block_hash
                        .is_none_or(|digest| digest == [0; 32]))
            {
                return Err(PopCredentialServiceError::RootRollback);
            }
        }
        Some(current) => {
            if next.cursor.block_height != current.cursor.block_height.saturating_add(1)
                || next.previous_block_hash != Some(current.cursor.block_hash)
            {
                return Err(PopCredentialServiceError::RootRollback);
            }
        }
    }
    if next
        .committed_operation_digests
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || next
            .rejected_operation_digests
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || next
            .revoked_issuer_public_keys
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || next.committed_operation_digests.iter().any(|digest| {
            next.rejected_operation_digests
                .binary_search(digest)
                .is_ok()
        })
    {
        return Err(PopCredentialServiceError::InvalidProjection);
    }
    let root: PopCommitmentRootV1 = decode_canonical(
        &next.canonical_commitment_root,
        POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
        POP_SERVICE_COLLECTION_MAX_V1,
    )?;
    let revocations: PopRevocationListV1 = decode_canonical(
        &next.canonical_revocation_list,
        POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
        POP_SERVICE_COLLECTION_MAX_V1,
    )?;
    verify_pop_commitment_root_signature_v1(&root)
        .map_err(|_| PopCredentialServiceError::InvalidProjection)?;
    verify_pop_revocation_list_signature_v1(&revocations)
        .map_err(|_| PopCredentialServiceError::InvalidProjection)?;
    if root.publisher_signature.public_key.as_slice() != policy.issuer_public_key
        || revocations.publisher_signature.public_key.as_slice() != policy.issuer_public_key
        || root.issuer_id != policy.issuer_id
        || revocations.issuer_id != policy.issuer_id
        || revocations.commitment_root != root.root_digest
    {
        return Err(PopCredentialServiceError::WrongPolicy);
    }
    if let Some(current) = current {
        let previous_root: PopCommitmentRootV1 = decode_canonical(
            &current.canonical_commitment_root,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        let previous_revocations: PopRevocationListV1 = decode_canonical(
            &current.canonical_revocation_list,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        if root.tree_version < previous_root.tree_version
            || (root.tree_version == previous_root.tree_version
                && root.root_digest != previous_root.root_digest)
            || (root.tree_version > previous_root.tree_version
                && (root.tree_version != previous_root.tree_version.saturating_add(1)
                    || root.previous_root_digest != Some(previous_root.root_digest)))
            || revocations.list_version < previous_revocations.list_version
            || (revocations.list_version == previous_revocations.list_version
                && revocations != previous_revocations)
            || (revocations.list_version > previous_revocations.list_version
                && revocations.list_version != previous_revocations.list_version.saturating_add(1))
            || previous_revocations.entries.iter().any(|previous| {
                revocations
                    .entries
                    .binary_search_by_key(&previous.nonce, |entry| entry.nonce)
                    .ok()
                    .and_then(|index| revocations.entries.get(index))
                    != Some(previous)
            })
        {
            return Err(PopCredentialServiceError::RootRollback);
        }
    }
    Ok(())
}

fn wallet_delivery_aad(
    request_id: [u8; 32],
    operation_digest: [u8; 32],
    credential_commitment: [u8; 32],
) -> Result<Vec<u8>, PopCredentialServiceError> {
    let metadata = PopWalletDeliveryAadV1 {
        version: POP_WALLET_DELIVERY_VERSION_V1,
        request_id,
        operation_digest,
        credential_commitment,
    };
    Ok([
        WALLET_DELIVERY_AAD_DOMAIN_V1,
        encode_canonical(&metadata)?.as_slice(),
    ]
    .concat())
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PopWalletDeliveryAadV1 {
    version: u16,
    request_id: [u8; 32],
    operation_digest: [u8; 32],
    credential_commitment: [u8; 32],
}

#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PopWalletVaultPlaintextV1 {
    bundle: PopIssuedCredentialBundleV1,
    witness: PopPrivateWitnessEnvelopeV1,
    finalized_operation_digest: [u8; 32],
    witness_commitment_root: [u8; 32],
    witness_commitment_tree_version: u64,
    active_revocation_list: PopRevocationListV1,
}

impl fmt::Debug for PopWalletVaultPlaintextV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopWalletVaultPlaintextV1([REDACTED])")
    }
}

impl Drop for PopWalletVaultPlaintextV1 {
    fn drop(&mut self) {
        scrub_pop_bundle(&mut self.bundle);
        self.witness.zeroize();
        scrub_sensitive_bytes(&mut self.finalized_operation_digest);
        scrub_sensitive_bytes(&mut self.witness_commitment_root);
        scrub_sensitive_bytes(&mut self.active_revocation_list.commitment_root);
        scrub_sensitive_bytes(&mut self.active_revocation_list.revocation_root);
        scrub_sensitive_string(&mut self.active_revocation_list.issuer_id);
        for entry in &mut self.active_revocation_list.entries {
            scrub_sensitive_bytes(&mut entry.nonce);
        }
        self.active_revocation_list.entries.clear();
        scrub_pop_signature(&mut self.active_revocation_list.publisher_signature);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PopWalletVaultMetadataV1 {
    version: u16,
    credential_commitment: [u8; 32],
    commitment_root: [u8; 32],
    commitment_tree_version: u64,
    revocation_root: [u8; 32],
    revocation_list_version: u64,
}

#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PopWalletVaultEnvelopeV1 {
    metadata: PopWalletVaultMetadataV1,
    wrapping_key_id: String,
    wrapped_dek: Vec<u8>,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

impl fmt::Debug for PopWalletVaultEnvelopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopWalletVaultEnvelopeV1")
            .field(
                "credential_commitment",
                &hex::encode(self.metadata.credential_commitment),
            )
            .field("ciphertext_len", &self.ciphertext.len())
            .field("private_payload", &"[REDACTED]")
            .finish()
    }
}

/// Encrypted local wallet credential custody and proof generation.
#[derive(Debug)]
pub struct PopWalletVault {
    root: PathBuf,
    recipient: Arc<dyn PopWalletRecipientV1>,
    key_wrapper: Arc<dyn PopWalletKeyWrapper>,
}

impl PopWalletVault {
    /// Open an encrypted wallet vault. Key material remains runtime-only.
    pub fn open(
        root: impl AsRef<Path>,
        recipient: Arc<dyn PopWalletRecipientV1>,
        key_wrapper: Arc<dyn PopWalletKeyWrapper>,
    ) -> Result<Self, PopCredentialServiceError> {
        bounded_production_runtime_handle("wallet_recipient_key_id", recipient.key_id())?;
        if recipient.public_key_digest() == [0; 32] {
            return Err(PopCredentialServiceError::RuntimeProviderRegistryMismatch);
        }
        bounded_production_runtime_handle("wallet_wrapping_key_id", key_wrapper.active_key_id())?;
        fs::create_dir_all(root.as_ref()).map_err(|_| PopCredentialServiceError::CheckpointIo)?;
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            recipient,
            key_wrapper,
        })
    }

    /// Import an issuer delivery only after its registry operation finalized.
    pub fn import_finalized_delivery(
        &self,
        canonical_delivery: &[u8],
        finalized: &PopFinalizedRegistryProjectionV1,
    ) -> Result<[u8; 32], PopCredentialServiceError> {
        let delivery: PopEncryptedWalletDeliveryV1 = decode_canonical(
            canonical_delivery,
            POP_WALLET_DELIVERY_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        if delivery.version != POP_WALLET_DELIVERY_VERSION_V1
            || finalized
                .committed_operation_digests
                .binary_search(&delivery.registry_operation_digest)
                .is_err()
        {
            return Err(PopCredentialServiceError::NotFinalized);
        }
        let aad = wallet_delivery_aad(
            delivery.request_id,
            delivery.registry_operation_digest,
            delivery.credential_commitment,
        )?;
        let mut plaintext_bytes = self
            .recipient
            .open_wallet_delivery(&delivery.encrypted_payload, &aad)
            .map_err(map_wallet_recipient_error)?;
        let plaintext_bytes = SensitiveBytesGuard::new(&mut plaintext_bytes);
        let private: PopPrivateWalletDeliveryV1 = decode_canonical(
            plaintext_bytes.as_slice(),
            POP_WALLET_DELIVERY_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
        drop(plaintext_bytes);
        private
            .bundle
            .validate()
            .map_err(|_| PopCredentialServiceError::InvalidIssuance)?;
        let mut canonical_credential = encode_canonical(&private.bundle.credential)?;
        let canonical_credential = SensitiveBytesGuard::new(&mut canonical_credential);
        if pop_credential_payload_commitment_v1(canonical_credential.as_slice())
            != delivery.credential_commitment
        {
            return Err(PopCredentialServiceError::InvalidIssuance);
        }
        drop(canonical_credential);
        let root: PopCommitmentRootV1 = decode_canonical(
            &finalized.canonical_commitment_root,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        let revocations: PopRevocationListV1 = decode_canonical(
            &finalized.canonical_revocation_list,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        if private.bundle.commitment_root != root
            || private.bundle.revocation_list.list_version > revocations.list_version
            || private.bundle.revocation_list.commitment_root != revocations.commitment_root
        {
            return Err(PopCredentialServiceError::RootRollback);
        }
        let vault_plaintext = PopWalletVaultPlaintextV1 {
            bundle: private.bundle.clone(),
            witness: private.witness.clone(),
            finalized_operation_digest: delivery.registry_operation_digest,
            witness_commitment_root: root.root_digest,
            witness_commitment_tree_version: root.tree_version,
            active_revocation_list: revocations,
        };
        self.persist_credential(delivery.credential_commitment, &vault_plaintext)?;
        Ok(delivery.credential_commitment)
    }

    fn persist_credential(
        &self,
        credential_commitment: [u8; 32],
        private: &PopWalletVaultPlaintextV1,
    ) -> Result<(), PopCredentialServiceError> {
        let wrapping_key_id = self.key_wrapper.active_key_id().to_owned();
        bounded_production_runtime_handle("wallet_wrapping_key_id", &wrapping_key_id)?;
        let metadata = PopWalletVaultMetadataV1 {
            version: POP_WALLET_VAULT_ENVELOPE_VERSION_V1,
            credential_commitment,
            commitment_root: private.witness_commitment_root,
            commitment_tree_version: private.witness_commitment_tree_version,
            revocation_root: private.active_revocation_list.revocation_root,
            revocation_list_version: private.active_revocation_list.list_version,
        };
        let metadata_bytes = encode_canonical(&metadata)?;
        let context = digest_domain(WALLET_VAULT_AAD_DOMAIN_V1, &metadata_bytes);
        let mut dek = [0_u8; 32];
        let mut dek = SensitiveBytesGuard::new(&mut dek);
        fill_nonzero_random(dek.as_mut_slice())?;
        let dek_array: &[u8; 32] = dek
            .as_slice()
            .try_into()
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        let wrapped_dek = self
            .key_wrapper
            .wrap_dek(context, dek_array)
            .map_err(|_| PopCredentialServiceError::KeyWrapping)?;
        if wrapped_dek.is_empty() || wrapped_dek.len() > POP_WRAPPED_DEK_MAX_BYTES_V1 {
            return Err(PopCredentialServiceError::KeyWrapping);
        }
        let mut nonce = [0_u8; 12];
        fill_nonzero_random(&mut nonce)?;
        let mut plaintext = encode_canonical(private)?;
        let plaintext = SensitiveBytesGuard::new(&mut plaintext);
        let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(dek.as_slice())
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        let ciphertext = encryptor
            .encrypt(
                nonce.as_slice(),
                metadata_bytes.as_slice(),
                plaintext.as_slice(),
            )
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        drop(encryptor);
        drop(plaintext);
        drop(dek);
        let envelope = PopWalletVaultEnvelopeV1 {
            metadata,
            wrapping_key_id,
            wrapped_dek,
            nonce,
            ciphertext,
        };
        let bytes = encode_canonical(&envelope)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > POP_WALLET_VAULT_MAX_BYTES_V1 {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        write_local_private_checkpoint_atomic(&self.credential_path(credential_commitment), &bytes)
            .map_err(|error| {
                if error.committed {
                    PopCredentialServiceError::CheckpointDurabilityUncertain
                } else {
                    PopCredentialServiceError::CheckpointIo
                }
            })
    }

    fn load_credential(
        &self,
        credential_commitment: [u8; 32],
    ) -> Result<PopWalletVaultPlaintextV1, PopCredentialServiceError> {
        let path = self.credential_path(credential_commitment);
        let bytes = read_local_checkpoint_bounded(&path, POP_WALLET_VAULT_MAX_BYTES_V1)
            .map_err(|_| PopCredentialServiceError::CheckpointIo)?
            .ok_or(PopCredentialServiceError::CredentialNotFound)?;
        let envelope: PopWalletVaultEnvelopeV1 = decode_canonical(
            &bytes,
            POP_WALLET_VAULT_MAX_BYTES_V1,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
        if envelope.metadata.version != POP_WALLET_VAULT_ENVELOPE_VERSION_V1
            || envelope.metadata.credential_commitment != credential_commitment
            || envelope.wrapped_dek.is_empty()
            || envelope.wrapped_dek.len() > POP_WRAPPED_DEK_MAX_BYTES_V1
        {
            return Err(PopCredentialServiceError::PoisonedCheckpoint);
        }
        bounded_production_runtime_handle("wallet_wrapping_key_id", &envelope.wrapping_key_id)
            .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
        let metadata_bytes = encode_canonical(&envelope.metadata)?;
        let context = digest_domain(WALLET_VAULT_AAD_DOMAIN_V1, &metadata_bytes);
        let mut dek = self
            .key_wrapper
            .unwrap_dek(&envelope.wrapping_key_id, context, &envelope.wrapped_dek)
            .map_err(|_| PopCredentialServiceError::KeyWrapping)?;
        let dek = SensitiveBytesGuard::new(&mut dek);
        let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(dek.as_slice())
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        let mut plaintext_bytes = decryptor
            .decrypt(
                envelope.nonce.as_slice(),
                metadata_bytes.as_slice(),
                envelope.ciphertext.as_slice(),
            )
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        drop(decryptor);
        drop(dek);
        let plaintext_bytes = SensitiveBytesGuard::new(&mut plaintext_bytes);
        let plaintext: PopWalletVaultPlaintextV1 = decode_canonical(
            plaintext_bytes.as_slice(),
            POP_WALLET_VAULT_MAX_BYTES_V1,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
        drop(plaintext_bytes);
        plaintext
            .bundle
            .validate()
            .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
        verify_pop_revocation_list_signature_v1(&plaintext.active_revocation_list)
            .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
        let mut canonical_credential = encode_canonical(&plaintext.bundle.credential)?;
        let canonical_credential = SensitiveBytesGuard::new(&mut canonical_credential);
        if pop_credential_payload_commitment_v1(canonical_credential.as_slice())
            != credential_commitment
            || plaintext.witness_commitment_root != envelope.metadata.commitment_root
            || plaintext.witness_commitment_tree_version
                != envelope.metadata.commitment_tree_version
            || plaintext.active_revocation_list.revocation_root != envelope.metadata.revocation_root
            || plaintext.active_revocation_list.list_version
                != envelope.metadata.revocation_list_version
        {
            return Err(PopCredentialServiceError::PoisonedCheckpoint);
        }
        drop(canonical_credential);
        Ok(plaintext)
    }

    /// Synchronize a private revocation witness to a newer finalized signed
    /// snapshot. Root/list rollback, issuer substitution, and mutation of an
    /// existing revocation entry are rejected before the vault is rewritten.
    pub fn synchronize_witness(
        &self,
        credential_commitment: [u8; 32],
        finalized: &PopFinalizedRegistryProjectionV1,
        witness: &PopMembershipWitnessV1,
    ) -> Result<(), PopCredentialServiceError> {
        witness
            .validate()
            .map_err(|_| PopCredentialServiceError::InvalidMembershipProof)?;
        let mut private = self.load_credential(credential_commitment)?;
        let root: PopCommitmentRootV1 = decode_canonical(
            &finalized.canonical_commitment_root,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        let revocations: PopRevocationListV1 = decode_canonical(
            &finalized.canonical_revocation_list,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        verify_pop_commitment_root_signature_v1(&root)
            .map_err(|_| PopCredentialServiceError::InvalidProjection)?;
        verify_pop_revocation_list_signature_v1(&revocations)
            .map_err(|_| PopCredentialServiceError::InvalidProjection)?;
        let issuer_key = private
            .bundle
            .credential
            .issuer_signature
            .public_key
            .as_slice();
        if root.root_digest != private.witness_commitment_root
            || root.tree_version != private.witness_commitment_tree_version
            || root.publisher_signature.public_key.as_slice() != issuer_key
            || revocations.publisher_signature.public_key.as_slice() != issuer_key
            || revocations.commitment_root != root.root_digest
            || revocations.list_version < private.active_revocation_list.list_version
            || (revocations.list_version == private.active_revocation_list.list_version
                && revocations != private.active_revocation_list)
            || private
                .active_revocation_list
                .entries
                .iter()
                .any(|previous| {
                    revocations
                        .entries
                        .binary_search_by_key(&previous.nonce, |entry| entry.nonce)
                        .ok()
                        .and_then(|index| revocations.entries.get(index))
                        != Some(previous)
                })
        {
            return Err(PopCredentialServiceError::RootRollback);
        }
        if witness.credential_path.siblings != private.witness.credential_siblings
            || witness.credential_path.directions != private.witness.credential_directions
        {
            return Err(PopCredentialServiceError::RootRollback);
        }
        private.witness = PopPrivateWitnessEnvelopeV1::from_witness(witness);
        private.active_revocation_list = revocations;
        self.persist_credential(credential_commitment, &private)
    }

    /// Rewrap one credential DEK under a replacement KMS/PKCS#11 key without
    /// changing ciphertext, nonce, or immutable authenticated metadata.
    pub fn rewrap_credential(
        &self,
        credential_commitment: [u8; 32],
        replacement: &dyn PopWalletKeyWrapper,
    ) -> Result<(), PopCredentialServiceError> {
        bounded_production_runtime_handle("wallet_wrapping_key_id", replacement.active_key_id())?;
        let path = self.credential_path(credential_commitment);
        let bytes = read_local_checkpoint_bounded(&path, POP_WALLET_VAULT_MAX_BYTES_V1)
            .map_err(|_| PopCredentialServiceError::CheckpointIo)?
            .ok_or(PopCredentialServiceError::CredentialNotFound)?;
        let mut envelope: PopWalletVaultEnvelopeV1 = decode_canonical(
            &bytes,
            POP_WALLET_VAULT_MAX_BYTES_V1,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .map_err(|_| PopCredentialServiceError::PoisonedCheckpoint)?;
        if envelope.metadata.version != POP_WALLET_VAULT_ENVELOPE_VERSION_V1
            || envelope.metadata.credential_commitment != credential_commitment
            || envelope.wrapped_dek.is_empty()
            || envelope.wrapped_dek.len() > POP_WRAPPED_DEK_MAX_BYTES_V1
        {
            return Err(PopCredentialServiceError::PoisonedCheckpoint);
        }
        let metadata_bytes = encode_canonical(&envelope.metadata)?;
        let context = digest_domain(WALLET_VAULT_AAD_DOMAIN_V1, &metadata_bytes);
        let mut dek = self
            .key_wrapper
            .unwrap_dek(&envelope.wrapping_key_id, context, &envelope.wrapped_dek)
            .map_err(|_| PopCredentialServiceError::KeyWrapping)?;
        let dek = SensitiveBytesGuard::new(&mut dek);
        let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(dek.as_slice())
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        let mut authenticated_plaintext = decryptor
            .decrypt(
                envelope.nonce.as_slice(),
                metadata_bytes.as_slice(),
                envelope.ciphertext.as_slice(),
            )
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        drop(decryptor);
        let authenticated_plaintext = SensitiveBytesGuard::new(&mut authenticated_plaintext);
        drop(authenticated_plaintext);
        let dek_array: &[u8; 32] = dek
            .as_slice()
            .try_into()
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        let replacement_wrapped_dek = replacement
            .wrap_dek(context, dek_array)
            .map_err(|_| PopCredentialServiceError::KeyWrapping)?;
        drop(dek);
        if replacement_wrapped_dek.is_empty()
            || replacement_wrapped_dek.len() > POP_WRAPPED_DEK_MAX_BYTES_V1
        {
            return Err(PopCredentialServiceError::KeyWrapping);
        }
        envelope.wrapping_key_id = replacement.active_key_id().to_owned();
        envelope.wrapped_dek = replacement_wrapped_dek;
        let replacement_bytes = encode_canonical(&envelope)?;
        if u64::try_from(replacement_bytes.len()).unwrap_or(u64::MAX)
            > POP_WALLET_VAULT_MAX_BYTES_V1
        {
            return Err(PopCredentialServiceError::ResourceExhausted);
        }
        write_local_private_checkpoint_atomic(&path, &replacement_bytes).map_err(|error| {
            if error.committed {
                PopCredentialServiceError::CheckpointDurabilityUncertain
            } else {
                PopCredentialServiceError::CheckpointIo
            }
        })
    }

    /// Produce a membership proof locally. Credential and witness bytes never
    /// leave the vault API; only the zero-knowledge proof is returned.
    pub fn prove_membership(
        &self,
        credential_commitment: [u8; 32],
        finalized: &PopFinalizedRegistryProjectionV1,
        challenge_digest: [u8; 32],
        verifier_context: &str,
        now_epoch: u64,
    ) -> Result<PopMembershipProofV1, PopCredentialServiceError> {
        let private = self.load_credential(credential_commitment)?;
        if finalized
            .committed_operation_digests
            .binary_search(&private.finalized_operation_digest)
            .is_err()
        {
            return Err(PopCredentialServiceError::NotFinalized);
        }
        let root: PopCommitmentRootV1 = decode_canonical(
            &finalized.canonical_commitment_root,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        let revocations: PopRevocationListV1 = decode_canonical(
            &finalized.canonical_revocation_list,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )?;
        if root.root_digest != private.witness_commitment_root
            || root.tree_version != private.witness_commitment_tree_version
            || revocations != private.active_revocation_list
        {
            return Err(PopCredentialServiceError::NotSynchronized);
        }
        let witness = PrivateMembershipWitnessGuard::new(private.witness.clone().into_witness());
        prove_pop_membership_v1(
            &private.bundle.credential,
            &root,
            &revocations,
            witness.as_ref(),
            challenge_digest,
            verifier_context,
            now_epoch,
        )
        .map_err(|_| PopCredentialServiceError::InvalidMembershipProof)
    }

    fn credential_path(&self, credential_commitment: [u8; 32]) -> PathBuf {
        self.root.join(format!(
            "{WALLET_VAULT_FILE_PREFIX_V1}{}{WALLET_VAULT_FILE_SUFFIX_V1}",
            hex::encode(credential_commitment)
        ))
    }
}

fn fill_nonzero_random(output: &mut [u8]) -> Result<(), PopCredentialServiceError> {
    for _ in 0..8 {
        OsRng
            .try_fill_bytes(output)
            .map_err(|_| PopCredentialServiceError::Encryption)?;
        if output.iter().any(|byte| *byte != 0) {
            return Ok(());
        }
    }
    Err(PopCredentialServiceError::Encryption)
}

/// PoP service failures. Variants intentionally omit private payload values.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PopCredentialServiceError {
    /// Unsupported V1 envelope or policy version.
    #[error("unsupported PoP service schema version")]
    UnsupportedVersion,
    /// A named non-secret input failed canonical validation.
    #[error("invalid PoP service input: {field}")]
    InvalidInput {
        /// Stable field label.
        field: &'static str,
    },
    /// Canonical bounded Norito encoding or decoding failed.
    #[error("invalid canonical bounded Norito payload")]
    Codec,
    /// Runtime resource policy was exhausted.
    #[error("PoP service resource limit exhausted")]
    ResourceExhausted,
    /// Enrollment encryption or authentication failed.
    #[error("PoP encrypted payload authentication failed")]
    Encryption,
    /// Private enrollment failed validation.
    #[error("invalid private PoP enrollment")]
    InvalidEnrollment,
    /// Request id replayed with a different envelope.
    #[error("PoP enrollment request replay")]
    EnrollmentReplay,
    /// Enrollment was not found.
    #[error("PoP enrollment not found")]
    EnrollmentNotFound,
    /// Approval was not bound to the exact request, envelope, and policy.
    #[error("PoP approval binding mismatch")]
    ApprovalBinding,
    /// Duplicate signer approval.
    #[error("duplicate PoP approval signer")]
    DuplicateApproval,
    /// Approval quorum is not currently satisfied.
    #[error("PoP dual-control approval quorum not satisfied")]
    ApprovalQuorum,
    /// Caller or approval signer is not governed.
    #[error("unauthorized PoP operation")]
    Unauthorized,
    /// Governed signer was revoked.
    #[error("PoP signer is revoked")]
    SignerRevoked,
    /// Signature is malformed or invalid.
    #[error("invalid PoP service signature")]
    InvalidSignature,
    /// Service state does not permit the transition.
    #[error("invalid PoP service state transition")]
    InvalidState,
    /// Runtime HSM key does not match finalized policy.
    #[error("PoP runtime HSM does not match finalized issuer policy")]
    HsmPolicyMismatch,
    /// Runtime HSM failed without exposing provider details.
    #[error("PoP runtime HSM unavailable")]
    HsmUnavailable,
    /// A runtime-only draft, witness, wallet, or clock provider is unavailable.
    #[error("PoP runtime provider unavailable")]
    RuntimeProviderUnavailable,
    /// The enabled service was not supplied its deployment provider registry.
    #[error("PoP runtime provider registry was not injected")]
    RuntimeProviderRegistryMissing,
    /// The registry identity or expected external policy does not match config.
    #[error("PoP runtime provider registry does not match configured policy")]
    RuntimeProviderRegistryMismatch,
    /// The registry or its external policy control plane is unavailable.
    #[error("PoP runtime provider registry is unavailable or stale")]
    RuntimeProviderRegistryUnavailable,
    /// Registry identity or policy changed across a guarded operation.
    #[error("PoP runtime provider registry identity or policy changed")]
    RuntimeProviderRegistryDrift,
    /// Issuance material failed cross-binding validation.
    #[error("invalid PoP issuance material")]
    InvalidIssuance,
    /// Registry operation digest is inconsistent.
    #[error("invalid PoP registry operation digest")]
    OperationDigest,
    /// Finalized policy does not match the configured authority.
    #[error("PoP finalized policy mismatch")]
    WrongPolicy,
    /// Finalized root/list/cursor attempted a rollback or fork.
    #[error("PoP finalized root or cursor rollback detected")]
    RootRollback,
    /// Finalized projection is malformed.
    #[error("invalid PoP finalized registry projection")]
    InvalidProjection,
    /// Registry client is unavailable.
    #[error("PoP registry service unavailable")]
    RegistryUnavailable,
    /// Service has not synchronized a finalized registry projection.
    #[error("PoP verifier is not synchronized")]
    NotSynchronized,
    /// Registry operation has not finalized.
    #[error("PoP registry operation is not finalized")]
    NotFinalized,
    /// Proof failed verification.
    #[error("invalid PoP membership proof")]
    InvalidMembershipProof,
    /// Nullifier replay was detected.
    #[error("PoP membership nullifier replay")]
    ReplayedProof,
    /// Wallet wrapping or unwrapping failed.
    #[error("PoP wallet key wrapping failed")]
    KeyWrapping,
    /// Wallet credential does not exist.
    #[error("PoP wallet credential not found")]
    CredentialNotFound,
    /// Private checkpoint I/O failed.
    #[error("PoP private checkpoint I/O failed")]
    CheckpointIo,
    /// Checkpoint rename became visible but parent-directory durability is
    /// uncertain; mutations remain fail-stopped until restart.
    #[error("PoP checkpoint is visible but durability is uncertain; restart required")]
    CheckpointDurabilityUncertain,
    /// Checkpoint is malformed, noncanonical, or internally inconsistent.
    #[error("PoP private checkpoint is poisoned")]
    PoisonedCheckpoint,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io, sync::Arc};

    use iroha_crypto::{HybridKeyPair, KeyPair};
    use rand::SeedableRng as _;
    use rand_chacha::ChaCha20Rng;
    use sorafs_manifest::pop_credentials::{
        POP_CREDENTIAL_TREE_DEPTH_V1, POP_REVOCATION_TREE_DEPTH_V1, derive_pop_holder_commitment_v1,
    };
    use tempfile::TempDir;

    #[derive(Debug)]
    struct TestAuthenticator {
        request_authority: PopRequestAuthorityV1,
    }

    impl PopCredentialApiAuthenticator for TestAuthenticator {
        fn authenticate(
            &self,
            _opaque_credential: &[u8],
            _action: PopCredentialApiActionV1,
            _request_binding: [u8; 32],
            _now_epoch: u64,
        ) -> Result<PopAuthenticatedPrincipalV1, String> {
            Ok(PopAuthenticatedPrincipalV1 {
                principal_digest: [0x31; 32],
                expires_at_epoch: 101,
                request_authority: self.request_authority,
            })
        }
    }

    #[test]
    fn mutation_actions_require_exact_caller_signed_authority() {
        let authenticated = PopCredentialApiV1::new(Arc::new(TestAuthenticator {
            request_authority: PopRequestAuthorityV1::AuthenticatedRequest,
        }));
        for action in [
            PopCredentialApiActionV1::ReadEnrollmentStatus,
            PopCredentialApiActionV1::SubmitRegistryOutbox,
            PopCredentialApiActionV1::ReconcileRegistry,
            PopCredentialApiActionV1::ReadRegistryProjection,
            PopCredentialApiActionV1::FetchWalletDelivery,
            PopCredentialApiActionV1::ProveMembership,
        ] {
            assert!(
                authenticated
                    .authorize(b"credential", action, [0x32; 32], 100)
                    .is_ok(),
                "authenticated read or durable work action {action:?}"
            );
        }
        for action in [
            PopCredentialApiActionV1::SubmitEnrollment,
            PopCredentialApiActionV1::ApproveEnrollment,
            PopCredentialApiActionV1::IssueCredential,
            PopCredentialApiActionV1::TriggerCredentialIssuance,
            PopCredentialApiActionV1::EnqueueRevocation,
            PopCredentialApiActionV1::AcknowledgeWalletDelivery,
            PopCredentialApiActionV1::ImportWalletDelivery,
            PopCredentialApiActionV1::SynchronizeWalletWitness,
            PopCredentialApiActionV1::VerifyMembership,
        ] {
            assert_eq!(
                authenticated.authorize(b"credential", action, [0x33; 32], 100),
                Err(PopCredentialServiceError::Unauthorized),
                "unsigned mutation action {action:?}"
            );
        }

        let caller_signed = PopCredentialApiV1::new(Arc::new(TestAuthenticator {
            request_authority: PopRequestAuthorityV1::CallerSignedTransaction,
        }));
        for action in [
            PopCredentialApiActionV1::SubmitEnrollment,
            PopCredentialApiActionV1::ApproveEnrollment,
            PopCredentialApiActionV1::IssueCredential,
            PopCredentialApiActionV1::TriggerCredentialIssuance,
            PopCredentialApiActionV1::EnqueueRevocation,
            PopCredentialApiActionV1::AcknowledgeWalletDelivery,
            PopCredentialApiActionV1::ImportWalletDelivery,
            PopCredentialApiActionV1::SynchronizeWalletWitness,
            PopCredentialApiActionV1::VerifyMembership,
        ] {
            assert!(
                caller_signed
                    .authorize(b"credential", action, [0x34; 32], 100)
                    .is_ok(),
                "caller-signed mutation action {action:?}"
            );
        }
    }

    #[derive(Debug)]
    struct TestHsm {
        key_id: String,
        keypair: KeyPair,
    }

    impl PopIssuerHsm for TestHsm {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key(&self) -> [u8; 32] {
            let (_, bytes) = self
                .keypair
                .public_key()
                .try_to_bytes()
                .expect("public key");
            bytes.try_into().expect("ed25519")
        }

        fn sign_digest(&self, digest: [u8; 32]) -> Result<[u8; 64], String> {
            Signature::try_new(self.keypair.private_key(), &digest)
                .map_err(|error| error.to_string())?
                .payload()
                .try_into()
                .map_err(|_| "signature length".to_owned())
        }
    }

    #[derive(Debug)]
    struct TestWrapper {
        key_id: String,
        key: [u8; 32],
    }

    impl PopWalletKeyWrapper for TestWrapper {
        fn active_key_id(&self) -> &str {
            &self.key_id
        }

        fn wrap_dek(&self, context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
            Ok(dek
                .iter()
                .zip(self.key)
                .zip(context)
                .map(|((&byte, key), aad)| byte ^ key ^ aad)
                .collect())
        }

        fn unwrap_dek(
            &self,
            key_id: &str,
            context: [u8; 32],
            wrapped_dek: &[u8],
        ) -> Result<[u8; 32], String> {
            if key_id != self.key_id || wrapped_dek.len() != 32 {
                return Err("wrong key".to_owned());
            }
            let mut dek = [0; 32];
            for (index, output) in dek.iter_mut().enumerate() {
                *output = wrapped_dek[index] ^ self.key[index] ^ context[index];
            }
            Ok(dek)
        }
    }

    struct TestRecipient {
        key_id: String,
        secret: HybridSecretKey,
        public_key_digest: [u8; 32],
    }

    impl fmt::Debug for TestRecipient {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestRecipient")
                .field("key_id", &self.key_id)
                .field("private_key", &"[REDACTED]")
                .finish()
        }
    }

    impl PopEnrollmentRecipientV1 for TestRecipient {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key_digest(&self) -> [u8; 32] {
            self.public_key_digest
        }

        fn open_enrollment(
            &self,
            encrypted_payload: &HybridPayloadEnvelopeV1,
            aad: &[u8],
        ) -> Result<Vec<u8>, PopRecipientOpenErrorV1> {
            decrypt_payload(encrypted_payload, aad, &self.secret)
                .map_err(|_| PopRecipientOpenErrorV1::Rejected)
        }
    }

    impl PopWalletRecipientV1 for TestRecipient {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn public_key_digest(&self) -> [u8; 32] {
            self.public_key_digest
        }

        fn open_wallet_delivery(
            &self,
            encrypted_payload: &HybridPayloadEnvelopeV1,
            aad: &[u8],
        ) -> Result<Vec<u8>, PopRecipientOpenErrorV1> {
            decrypt_payload(encrypted_payload, aad, &self.secret)
                .map_err(|_| PopRecipientOpenErrorV1::Rejected)
        }
    }

    fn test_recipient(key_id: &str, keypair: &HybridKeyPair) -> Arc<TestRecipient> {
        Arc::new(TestRecipient {
            key_id: key_id.to_owned(),
            secret: keypair.secret().clone(),
            public_key_digest: pop_enrollment_recipient_public_key_digest_v1(keypair.public()),
        })
    }

    fn ed25519(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("keypair")
    }

    #[test]
    fn enrollment_recipient_public_key_digest_binds_both_hybrid_components() {
        let mut first_rng = ChaCha20Rng::from_seed([0x31; 32]);
        let first = HybridKeyPair::generate(&mut first_rng).expect("first hybrid key");
        let mut replay_rng = ChaCha20Rng::from_seed([0x31; 32]);
        let replay = HybridKeyPair::generate(&mut replay_rng).expect("replayed hybrid key");
        let mut second_rng = ChaCha20Rng::from_seed([0x32; 32]);
        let second = HybridKeyPair::generate(&mut second_rng).expect("second hybrid key");

        let first_digest = pop_enrollment_recipient_public_key_digest_v1(first.public());
        assert_ne!(first_digest, [0; 32]);
        assert_eq!(
            first_digest,
            pop_enrollment_recipient_public_key_digest_v1(first.secret().public())
        );
        assert_eq!(
            first_digest,
            pop_enrollment_recipient_public_key_digest_v1(replay.public())
        );
        assert_ne!(
            first_digest,
            pop_enrollment_recipient_public_key_digest_v1(second.public())
        );
    }

    #[test]
    fn recipient_capability_failures_map_without_provider_details() {
        assert_eq!(
            map_enrollment_recipient_error(PopRecipientOpenErrorV1::Unavailable),
            PopCredentialServiceError::RuntimeProviderUnavailable
        );
        assert_eq!(
            map_enrollment_recipient_error(PopRecipientOpenErrorV1::Rejected),
            PopCredentialServiceError::InvalidEnrollment
        );
        assert_eq!(
            map_wallet_recipient_error(PopRecipientOpenErrorV1::Unavailable),
            PopCredentialServiceError::RuntimeProviderUnavailable
        );
        assert_eq!(
            map_wallet_recipient_error(PopRecipientOpenErrorV1::Rejected),
            PopCredentialServiceError::Encryption
        );
    }

    fn public_key(keypair: &KeyPair) -> [u8; 32] {
        let (_, bytes) = keypair.public_key().try_to_bytes().expect("key");
        bytes.try_into().expect("ed25519")
    }

    fn policy(hsm: &TestHsm, approvers: &[KeyPair]) -> PopCredentialServicePolicyV1 {
        PopCredentialServicePolicyV1 {
            version: POP_CREDENTIAL_SERVICE_POLICY_VERSION_V1,
            issuer_policy_digest: [0x41; 32],
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issuer_hsm_key_id: hsm.key_id.clone(),
            issuer_public_key: hsm.public_key(),
            enrollment_recipient_key_id: "kms://pop/enrollment/primary".to_owned(),
            approval_quorum: 2,
            approval_signers: approvers
                .iter()
                .enumerate()
                .map(|(index, key)| PopApprovalSignerV1 {
                    signer_id: format!("approver-{index}"),
                    public_key: public_key(key),
                    revoked_at_epoch: None,
                })
                .collect(),
            max_pending_enrollments: 16,
            max_outbox_entries: 16,
            max_dead_letters: 16,
            max_seen_nullifiers: 16,
            max_submission_attempts: 2,
        }
    }

    fn scalar(value: u64) -> [u8; 32] {
        let mut output = [0; 32];
        output[..8].copy_from_slice(&value.to_le_bytes());
        output
    }

    fn nonce(value: u128) -> [u8; 32] {
        let mut output = [0; 32];
        output[..16].copy_from_slice(&value.to_le_bytes());
        output
    }

    fn private_enrollment(wallet: &HybridKeyPair) -> PopPrivateEnrollmentV1 {
        let attestation_payload = b"private biometric attestation".to_vec();
        let holder_commitment =
            derive_pop_holder_commitment_v1(scalar(0x1234), scalar(0x5678)).unwrap();
        PopPrivateEnrollmentV1 {
            request: PopEnrollmentRequestV1 {
                version: sorafs_manifest::POP_ENROLLMENT_REQUEST_VERSION_V1,
                request_id: [0x11; 32],
                applicant_id: "private-applicant-alias".to_owned(),
                requested_class: sorafs_manifest::PopEligibilityClassV1::General,
                requested_attributes: vec!["residency".to_owned()],
                attestation_digest: pop_enrollment_attestation_digest_v1(&attestation_payload),
                submitted_at_epoch: 10,
                expires_at_epoch: 100,
            },
            holder_commitment,
            wallet_x25519_public_key: wallet.public().x25519_bytes(),
            wallet_mlkem_public_key: wallet.public().kyber_bytes().to_vec(),
            attestation_payload,
        }
    }

    fn canonical_temp_root(temp: &TempDir) -> PathBuf {
        fs::canonicalize(temp.path()).expect("canonical temporary directory")
    }

    fn injected_sensitive_failure(bytes: &mut [u8]) -> Result<(), PopCredentialServiceError> {
        let _guard = SensitiveBytesGuard::new(bytes);
        Err(PopCredentialServiceError::Encryption)
    }

    #[test]
    fn production_runtime_handles_use_canonical_grammar() {
        for handle in [
            "hsm://sorafs/pop/issuer-primary",
            "kms://sorafs/pop/wallet-primary",
        ] {
            assert_eq!(
                bounded_production_runtime_handle("runtime_handle", handle),
                Ok(())
            );
        }
        for handle in [
            "hsm://sorafs/pop/issuer-test",
            "kms://pop/mock/wallet",
            "kms://pop/placeholder/enrollment",
            "kms://pop/private key",
            "kms://pop/ключ",
            "hsm://sorafs/pop/operator@issuer",
            "hsm://sorafs/pop/issuer?token",
            "hsm://sorafs/pop/issuer#fragment",
            "hsm://sorafs/pop/%69ssuer",
            "hsm://sorafs/pop/issuer\\primary",
        ] {
            assert!(matches!(
                bounded_production_runtime_handle("runtime_handle", handle),
                Err(PopCredentialServiceError::InvalidInput {
                    field: "runtime_handle"
                })
            ));
        }
    }

    #[test]
    fn sensitive_guard_scrubs_on_early_error() {
        let mut secret = vec![0xA5; 64];
        assert_eq!(
            injected_sensitive_failure(&mut secret),
            Err(PopCredentialServiceError::Encryption)
        );
        assert_eq!(secret, vec![0; 64]);
    }

    #[test]
    fn private_membership_witness_guard_scrubs_all_secret_material() {
        let mut guard = PrivateMembershipWitnessGuard::new(PopMembershipWitnessV1 {
            holder_secret: [0xA1; 32],
            credential_path: PopCredentialMerklePathV1 {
                siblings: vec![[0xB2; 32], [0xC3; 32]],
                directions: vec![true, true],
            },
            revocation_path: PopRevocationNonMembershipPathV1 {
                siblings: vec![[0xD4; 32]],
            },
        });

        guard.zeroize();

        assert_eq!(guard.witness.holder_secret, [0; 32]);
        assert!(
            guard
                .witness
                .credential_path
                .siblings
                .iter()
                .flatten()
                .all(|byte| *byte == 0)
        );
        assert!(
            guard
                .witness
                .credential_path
                .directions
                .iter()
                .all(|direction| !direction)
        );
        assert!(
            guard
                .witness
                .revocation_path
                .siblings
                .iter()
                .flatten()
                .all(|byte| *byte == 0)
        );
    }

    fn approval(
        signer_id: &str,
        keypair: &KeyPair,
        envelope: &PopEncryptedEnrollmentV1,
        policy: &PopCredentialServicePolicyV1,
        decision: PopApprovalDecisionV1,
    ) -> PopApprovalV1 {
        let mut approval = PopApprovalV1 {
            version: POP_APPROVAL_VERSION_V1,
            request_id: envelope.request_id,
            enrollment_envelope_digest: envelope.digest().expect("digest"),
            issuer_policy_digest: policy.issuer_policy_digest,
            decision,
            decided_at_epoch: 20,
            signer_id: signer_id.to_owned(),
            signature: Vec::new(),
        };
        approval.signature = Signature::try_new(
            keypair.private_key(),
            &approval.signature_digest().expect("digest"),
        )
        .expect("sign")
        .payload()
        .to_vec();
        approval
    }

    fn service_fixture() -> (
        TempDir,
        PopCredentialService,
        PopCredentialServicePolicyV1,
        HybridKeyPair,
        Vec<KeyPair>,
        PopEncryptedEnrollmentV1,
    ) {
        let temp = TempDir::new().expect("temp");
        let hsm = Arc::new(TestHsm {
            key_id: "hsm://sorafs/pop/issuer-primary".to_owned(),
            keypair: ed25519(1),
        });
        let approvers = vec![ed25519(2), ed25519(3), ed25519(4)];
        let policy = policy(&hsm, &approvers);
        let mut rng = ChaCha20Rng::from_seed([0x21; 32]);
        let issuer_encryption = HybridKeyPair::generate(&mut rng).expect("issuer encryption");
        let wallet = HybridKeyPair::generate(&mut rng).expect("wallet encryption");
        let enrollment = encrypt_pop_enrollment_v1(
            &private_enrollment(&wallet),
            &policy,
            issuer_encryption.public(),
            &mut rng,
        )
        .expect("encrypt");
        let service = PopCredentialService::open(
            canonical_temp_root(&temp),
            policy.clone(),
            test_recipient(&policy.enrollment_recipient_key_id, &issuer_encryption),
            hsm,
        )
        .expect("service");
        (temp, service, policy, wallet, approvers, enrollment)
    }

    #[test]
    fn enrollment_is_encrypted_and_debug_is_payload_free() {
        let (_temp, mut service, _policy, _wallet, _approvers, enrollment) = service_fixture();
        let bytes = encode_canonical(&enrollment).expect("encode");
        let rendered = format!("{enrollment:?}");
        assert!(!rendered.contains("private-applicant"));
        assert!(
            !bytes
                .windows(b"private-applicant".len())
                .any(|window| window == b"private-applicant")
        );
        let status = service.submit_enrollment(&bytes, 20).expect("submit");
        assert_eq!(status.state, PopEnrollmentStateV1::AwaitingApproval);
        let checkpoint = fs::read(service.checkpoint_path).expect("checkpoint");
        assert!(
            !checkpoint
                .windows(b"biometric".len())
                .any(|window| window == b"biometric")
        );
        assert!(
            !checkpoint
                .windows(b"private-applicant".len())
                .any(|window| window == b"private-applicant")
        );
    }

    #[test]
    fn enrollment_replay_is_idempotent_only_for_identical_ciphertext() {
        let (_temp, mut service, _policy, _wallet, _approvers, mut enrollment) = service_fixture();
        let bytes = encode_canonical(&enrollment).expect("encode");
        service.submit_enrollment(&bytes, 20).expect("first");
        service.submit_enrollment(&bytes, 21).expect("idempotent");
        enrollment.encrypted_payload.ciphertext[0] ^= 1;
        assert_eq!(
            service.submit_enrollment(&encode_canonical(&enrollment).unwrap(), 21),
            Err(PopCredentialServiceError::EnrollmentReplay)
        );
    }

    fn fail_before_checkpoint_rename(
        _path: &Path,
        _bytes: &[u8],
    ) -> Result<(), PopCheckpointPersistFailure> {
        Err(PopCheckpointPersistFailure {
            error: PopCredentialServiceError::CheckpointIo,
            committed: false,
        })
    }

    fn fail_checkpoint_parent_sync(_: &Path) -> io::Result<()> {
        Err(io::Error::other("injected parent sync failure"))
    }

    fn fail_after_checkpoint_rename(
        path: &Path,
        bytes: &[u8],
    ) -> Result<(), PopCheckpointPersistFailure> {
        crate::write_local_checkpoint_atomic_with_mode_and_parent_sync(
            path,
            bytes,
            true,
            fail_checkpoint_parent_sync,
        )
        .map_err(|error| PopCheckpointPersistFailure {
            error: if error.committed {
                PopCredentialServiceError::CheckpointDurabilityUncertain
            } else {
                PopCredentialServiceError::CheckpointIo
            },
            committed: error.committed,
        })
    }

    #[test]
    fn crash_before_and_after_rename_preserve_transaction_boundaries() {
        let (_temp, mut service, policy, _wallet, approvers, enrollment) = service_fixture();
        let canonical = encode_canonical(&enrollment).unwrap();
        let original_checkpoint = fs::read(&service.checkpoint_path).unwrap();

        service.checkpoint_writer = fail_before_checkpoint_rename;
        assert_eq!(
            service.submit_enrollment(&canonical, 20),
            Err(PopCredentialServiceError::CheckpointIo)
        );
        assert!(service.state.enrollments.is_empty());
        assert_eq!(
            fs::read(&service.checkpoint_path).unwrap(),
            original_checkpoint
        );

        service.checkpoint_writer = fail_after_checkpoint_rename;
        assert_eq!(
            service.submit_enrollment(&canonical, 20),
            Err(PopCredentialServiceError::CheckpointDurabilityUncertain)
        );
        assert_eq!(service.state.enrollments.len(), 1);
        let visible = fs::read(&service.checkpoint_path).unwrap();
        let restored: PopIssuerCheckpointV1 = decode_canonical(
            &visible,
            POP_ISSUER_CHECKPOINT_MAX_BYTES_V1,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .unwrap();
        assert_eq!(restored.enrollments.len(), 1);
        let approval = approval(
            "approver-0",
            &approvers[0],
            &enrollment,
            &policy,
            PopApprovalDecisionV1::Approve,
        );
        assert_eq!(
            service.record_approval(approval, 21),
            Err(PopCredentialServiceError::CheckpointDurabilityUncertain)
        );
    }

    #[test]
    fn dual_control_rejects_duplicates_wrong_policy_and_revoked_signer() {
        let (_temp, mut service, policy, _wallet, approvers, enrollment) = service_fixture();
        service
            .submit_enrollment(&encode_canonical(&enrollment).unwrap(), 20)
            .unwrap();
        let first = approval(
            "approver-0",
            &approvers[0],
            &enrollment,
            &policy,
            PopApprovalDecisionV1::Approve,
        );
        service.record_approval(first.clone(), 20).unwrap();
        assert_eq!(
            service.record_approval(first, 20),
            Err(PopCredentialServiceError::DuplicateApproval)
        );
        let mut wrong_policy = approval(
            "approver-1",
            &approvers[1],
            &enrollment,
            &policy,
            PopApprovalDecisionV1::Approve,
        );
        wrong_policy.issuer_policy_digest = [9; 32];
        assert_eq!(
            service.record_approval(wrong_policy, 20),
            Err(PopCredentialServiceError::ApprovalBinding)
        );
        service.policy.approval_signers[1].revoked_at_epoch = Some(20);
        let revoked = approval(
            "approver-1",
            &approvers[1],
            &enrollment,
            &policy,
            PopApprovalDecisionV1::Approve,
        );
        assert_eq!(
            service.record_approval(revoked, 20),
            Err(PopCredentialServiceError::SignerRevoked)
        );
    }

    #[test]
    fn approval_policy_rejects_duplicate_keys_under_distinct_ids() {
        let hsm = TestHsm {
            key_id: "hsm://sorafs/pop/issuer-primary".to_owned(),
            keypair: ed25519(1),
        };
        let approvers = vec![ed25519(2), ed25519(3)];
        let mut policy = policy(&hsm, &approvers);
        policy.approval_signers[1].public_key = policy.approval_signers[0].public_key;
        assert_eq!(
            policy.validate(),
            Err(PopCredentialServiceError::InvalidInput {
                field: "approval_signer_public_key"
            })
        );
    }

    #[derive(Debug)]
    struct FailingSubmitter;

    impl PopRegistrySubmitter for FailingSubmitter {
        fn submit(
            &self,
            _idempotency_key: [u8; 32],
            _operation: &PopRegistryOperationV1,
        ) -> Result<(), String> {
            Err("private upstream details".to_owned())
        }
    }

    #[test]
    fn retry_exhaustion_is_durable_and_payload_free() {
        let (_temp, mut service, policy, _wallet, _approvers, _enrollment) = service_fixture();
        let hsm = TestHsm {
            key_id: policy.issuer_hsm_key_id.clone(),
            keypair: ed25519(1),
        };
        let revocations =
            sign_revocation_with_hsm(unsigned_revocations(hsm.public_key(), scalar(101), 1), &hsm)
                .expect("signed revocations");
        let operation =
            PopRegistryOperationV1::new(PopRegistryOperationKindV1::PublishRevocationList {
                canonical_revocation_list: encode_canonical(&revocations).unwrap(),
                issuer_policy_digest: policy.issuer_policy_digest,
            })
            .expect("operation envelope");
        let digest = operation.operation_digest;
        service
            .transact(|state| {
                state.next_outbox_sequence = 2;
                state.outbox.push(PopRegistryOutboxEntryV1 {
                    sequence: 1,
                    idempotency_key: registry_idempotency_key(1, digest),
                    operation,
                    accepted_once: false,
                    attempt_count: 0,
                    last_attempt_epoch: None,
                });
                Ok(())
            })
            .unwrap();
        assert_eq!(
            service.submit_next(&FailingSubmitter, 30),
            Ok(PopOutboxSubmitOutcomeV1::RetryScheduled {
                operation_digest: digest
            })
        );
        assert_eq!(
            service.submit_next(&FailingSubmitter, 31),
            Ok(PopOutboxSubmitOutcomeV1::DeadLettered {
                operation_digest: digest
            })
        );
        assert!(service.state.outbox.is_empty());
        assert_eq!(service.state.dead_letters.len(), 1);
        let checkpoint = fs::read(&service.checkpoint_path).unwrap();
        assert!(
            !checkpoint
                .windows(b"private upstream details".len())
                .any(|window| window == b"private upstream details")
        );
    }

    #[test]
    fn nullifier_replay_cache_is_atomic_and_survives_restart() {
        let (temp, mut service, policy, _wallet, _approvers, _enrollment) = service_fixture();
        let enrollment_recipient = Arc::clone(&service.enrollment_recipient);
        let hsm = Arc::clone(&service.hsm);
        let nullifier = scalar(77);
        service
            .consume_verified_nullifier(nullifier)
            .expect("first consumption");
        assert_eq!(
            service.consume_verified_nullifier(nullifier),
            Err(PopCredentialServiceError::ReplayedProof)
        );
        drop(service);

        let mut restored = PopCredentialService::open(
            canonical_temp_root(&temp),
            policy,
            enrollment_recipient,
            hsm,
        )
        .unwrap();
        assert_eq!(
            restored.consume_verified_nullifier(nullifier),
            Err(PopCredentialServiceError::ReplayedProof)
        );
    }

    #[test]
    fn semantically_poisoned_checkpoint_policy_binding_fails_closed() {
        let (temp, mut service, policy, _wallet, _approvers, enrollment) = service_fixture();
        service
            .submit_enrollment(&encode_canonical(&enrollment).unwrap(), 20)
            .unwrap();
        let enrollment_recipient = Arc::clone(&service.enrollment_recipient);
        let hsm = Arc::clone(&service.hsm);
        let checkpoint_path = service.checkpoint_path.clone();
        let mut poisoned = service.state.clone();
        let record = poisoned.enrollments.first_mut().unwrap();
        let mut envelope: PopEncryptedEnrollmentV1 = decode_canonical(
            &record.canonical_encrypted_enrollment,
            POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
            POP_SERVICE_COLLECTION_MAX_V1,
        )
        .unwrap();
        envelope.issuer_id = "substituted-issuer".to_owned();
        record.canonical_encrypted_enrollment = encode_canonical(&envelope).unwrap();
        record.envelope_digest = envelope.digest().unwrap();
        let poisoned_bytes = encode_canonical(&poisoned).unwrap();
        drop(service);
        write_local_private_checkpoint_atomic(&checkpoint_path, &poisoned_bytes).unwrap();

        assert_eq!(
            PopCredentialService::open(
                canonical_temp_root(&temp),
                policy,
                enrollment_recipient,
                hsm,
            )
            .expect_err("semantic poison"),
            PopCredentialServiceError::PoisonedCheckpoint
        );
    }

    #[test]
    fn poisoned_checkpoint_and_symlink_target_fail_closed() {
        let (temp, service, policy, _wallet, _approvers, _enrollment) = service_fixture();
        let checkpoint = service.checkpoint_path.clone();
        drop(service);
        fs::write(&checkpoint, b"not norito").expect("poison");
        let mut rng = ChaCha20Rng::from_seed([0x22; 32]);
        let issuer_encryption = HybridKeyPair::generate(&mut rng).expect("key");
        let hsm = Arc::new(TestHsm {
            key_id: policy.issuer_hsm_key_id.clone(),
            keypair: ed25519(1),
        });
        assert_eq!(
            PopCredentialService::open(
                canonical_temp_root(&temp),
                policy.clone(),
                test_recipient(&policy.enrollment_recipient_key_id, &issuer_encryption),
                hsm.clone(),
            )
            .expect_err("poisoned"),
            PopCredentialServiceError::PoisonedCheckpoint
        );

        fs::remove_file(&checkpoint).expect("remove poison");
        let outside = temp.path().join("outside");
        fs::write(&outside, b"sentinel").expect("outside");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &checkpoint).expect("symlink");
        #[cfg(unix)]
        assert_eq!(
            PopCredentialService::open(
                canonical_temp_root(&temp),
                policy.clone(),
                test_recipient(&policy.enrollment_recipient_key_id, &issuer_encryption),
                hsm,
            )
            .expect_err("symlink"),
            PopCredentialServiceError::CheckpointIo
        );
        assert_eq!(fs::read(outside).unwrap(), b"sentinel");
    }

    fn empty_signature(key: [u8; 32]) -> PopSignatureV1 {
        PopSignatureV1 {
            algorithm: PopSignatureAlgorithmV1::Ed25519,
            public_key: key.to_vec(),
            signature: vec![1; 64],
        }
    }

    fn unsigned_root(
        key: [u8; 32],
        version: u64,
        previous: Option<[u8; 32]>,
    ) -> PopCommitmentRootV1 {
        PopCommitmentRootV1 {
            version: sorafs_manifest::POP_COMMITMENT_ROOT_VERSION_V1,
            root_digest: scalar(100 + version),
            tree_size: version,
            tree_depth: POP_CREDENTIAL_TREE_DEPTH_V1,
            tree_version: version,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch: 30 + version,
            previous_root_digest: previous,
            governance_event_digest: [0x61; 32],
            publisher_signature: empty_signature(key),
        }
    }

    fn unsigned_revocations(key: [u8; 32], root: [u8; 32], version: u64) -> PopRevocationListV1 {
        PopRevocationListV1 {
            version: sorafs_manifest::POP_REVOCATION_LIST_VERSION_V1,
            list_version: version,
            commitment_root: root,
            revocation_root: sorafs_manifest::pop_credentials::pop_revocation_root_v1(&[])
                .expect("root"),
            revocation_tree_depth: POP_REVOCATION_TREE_DEPTH_V1,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            published_at_epoch: 30 + version,
            entries: Vec::new(),
            publisher_signature: empty_signature(key),
        }
    }

    fn projection(
        hsm: &TestHsm,
        height: u64,
        previous_block_hash: Option<[u8; 32]>,
        root_version: u64,
        previous_root: Option<[u8; 32]>,
        policy_digest: [u8; 32],
    ) -> PopFinalizedRegistryProjectionV1 {
        let root = unsigned_root(hsm.public_key(), root_version, previous_root);
        let revocations = unsigned_revocations(hsm.public_key(), root.root_digest, root_version);
        let bundle = sign_bundle_with_hsm(
            PopCredentialV1 {
                version: sorafs_manifest::POP_CREDENTIAL_VERSION_V1,
                credential_id: scalar(1),
                holder_commitment: scalar(2),
                eligibility_class: sorafs_manifest::PopEligibilityClassV1::General,
                attributes: Vec::new(),
                issuer_id: "pop-issuer-sora-foundation".to_owned(),
                issued_at_epoch: 1,
                expires_at_epoch: 1000,
                renewal_at_epoch: 500,
                revocation_nonce: nonce(1),
                commitment_root: root.root_digest,
                commitment_tree_version: root.tree_version,
                revocation_list_version: revocations.list_version,
                issuer_signature: empty_signature(hsm.public_key()),
            },
            root,
            revocations,
            hsm,
        )
        .expect("sign");
        PopFinalizedRegistryProjectionV1 {
            version: POP_FINALIZED_REGISTRY_PROJECTION_VERSION_V1,
            cursor: PopFinalizedCursorV1 {
                block_height: height,
                block_hash: [height as u8; 32],
            },
            previous_block_hash,
            issuer_policy_digest: policy_digest,
            canonical_commitment_root: encode_canonical(&bundle.commitment_root).unwrap(),
            canonical_revocation_list: encode_canonical(&bundle.revocation_list).unwrap(),
            committed_operation_digests: Vec::new(),
            rejected_operation_digests: Vec::new(),
            revoked_issuer_public_keys: Vec::new(),
        }
    }

    #[test]
    fn finalized_sync_rejects_cursor_root_rollback_and_wrong_policy() {
        let hsm = TestHsm {
            key_id: "hsm://sorafs/pop/issuer-primary".to_owned(),
            keypair: ed25519(1),
        };
        let policy = policy(&hsm, &[ed25519(2), ed25519(3)]);
        let first = projection(&hsm, 1, None, 2, None, policy.issuer_policy_digest);
        validate_projection(None, &first, &policy).expect("first");

        let mut wrong_policy = first.clone();
        wrong_policy.issuer_policy_digest = [0x99; 32];
        assert_eq!(
            validate_projection(None, &wrong_policy, &policy),
            Err(PopCredentialServiceError::WrongPolicy)
        );

        let rollback = projection(
            &hsm,
            2,
            Some(first.cursor.block_hash),
            1,
            None,
            policy.issuer_policy_digest,
        );
        assert_eq!(
            validate_projection(Some(&first), &rollback, &policy),
            Err(PopCredentialServiceError::RootRollback)
        );

        let fork = projection(
            &hsm,
            2,
            Some([0xFF; 32]),
            3,
            Some(
                decode_canonical::<PopCommitmentRootV1>(
                    &first.canonical_commitment_root,
                    1_000_000,
                    100,
                )
                .unwrap()
                .root_digest,
            ),
            policy.issuer_policy_digest,
        );
        assert_eq!(
            validate_projection(Some(&first), &fork, &policy),
            Err(PopCredentialServiceError::RootRollback)
        );
    }

    #[test]
    fn wallet_vault_rejects_symlink_and_wrong_wrapping_key() {
        let temp = TempDir::new().unwrap();
        let mut recipient_rng = ChaCha20Rng::from_seed([0x77; 32]);
        let recipient_key =
            HybridKeyPair::generate(&mut recipient_rng).expect("wallet recipient key");
        let recipient = test_recipient("kms://wallet/recipient-one", &recipient_key);
        let wrapper = Arc::new(TestWrapper {
            key_id: "kms://wallet/one".to_owned(),
            key: [7; 32],
        });
        let vault =
            PopWalletVault::open(canonical_temp_root(&temp), recipient.clone(), wrapper).unwrap();
        let target = temp.path().join("outside");
        fs::write(&target, b"sentinel").unwrap();
        let credential = [0xAB; 32];
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, vault.credential_path(credential)).unwrap();
        let private = PopWalletVaultPlaintextV1 {
            bundle: PopIssuedCredentialBundleV1 {
                version: sorafs_manifest::POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1,
                credential: PopCredentialV1 {
                    version: 0,
                    credential_id: [0; 32],
                    holder_commitment: [0; 32],
                    eligibility_class: sorafs_manifest::PopEligibilityClassV1::General,
                    attributes: Vec::new(),
                    issuer_id: String::new(),
                    issued_at_epoch: 0,
                    expires_at_epoch: 0,
                    renewal_at_epoch: 0,
                    revocation_nonce: [0; 32],
                    commitment_root: [0; 32],
                    commitment_tree_version: 0,
                    revocation_list_version: 0,
                    issuer_signature: empty_signature([1; 32]),
                },
                commitment_root: unsigned_root([1; 32], 1, None),
                revocation_list: unsigned_revocations([1; 32], scalar(101), 1),
            },
            witness: PopPrivateWitnessEnvelopeV1 {
                holder_secret: [1; 32],
                credential_siblings: vec![[0; 32]; usize::from(POP_CREDENTIAL_TREE_DEPTH_V1)],
                credential_directions: vec![false; usize::from(POP_CREDENTIAL_TREE_DEPTH_V1)],
                revocation_siblings: vec![[0; 32]; usize::from(POP_REVOCATION_TREE_DEPTH_V1)],
            },
            finalized_operation_digest: [1; 32],
            witness_commitment_root: scalar(101),
            witness_commitment_tree_version: 1,
            active_revocation_list: unsigned_revocations([1; 32], scalar(101), 1),
        };
        #[cfg(unix)]
        assert_eq!(
            vault.persist_credential(credential, &private),
            Err(PopCredentialServiceError::CheckpointIo)
        );
        assert_eq!(fs::read(target).unwrap(), b"sentinel");
        #[cfg(unix)]
        fs::remove_file(vault.credential_path(credential)).unwrap();
        vault
            .persist_credential(credential, &private)
            .expect("encrypted vault");
        let wrong_wrapper = Arc::new(TestWrapper {
            key_id: "kms://wallet/two".to_owned(),
            key: [8; 32],
        });
        let wrong_vault =
            PopWalletVault::open(canonical_temp_root(&temp), recipient, wrong_wrapper).unwrap();
        assert_eq!(
            wrong_vault.load_credential(credential),
            Err(PopCredentialServiceError::KeyWrapping)
        );
    }

    #[test]
    fn sensitive_struct_debug_is_redacted() {
        let mut rng = ChaCha20Rng::from_seed([0x23; 32]);
        let wallet = HybridKeyPair::generate(&mut rng).unwrap();
        let private = private_enrollment(&wallet);
        let rendered = format!("{private:?}");
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("private-applicant-alias"));
        assert!(!rendered.contains("biometric"));
    }
}
