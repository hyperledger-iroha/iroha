//! Authenticate release receipt inputs using independently pinned policy and state-observer trust.
//!
//! The observer is responsible for authenticating consensus finality and the exact custody and
//! operation rows before signing. Its signature is an accountable observation, not a consensus
//! proof or device attestation. Policy/trust digests and verification time must come from the
//! release coordinator, independently of all candidate artifacts. Only public data crosses here.

use super::{
    custody::{
        SignerCustodyActiveHeadV1, SignerCustodyAnchorV1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyTrustV1, SignerCustodyUseContextV1,
    },
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1, valid_identity},
    receipt::{
        SignerCompletedOperationV1, SignerReceiptErrorV1, SignerReleaseManifestExpectedV1,
        VerifiedReleaseManifestSignerReceiptV1, signer_release_manifest_digest_v1,
        verify_release_manifest_signer_receipt_v1,
    },
};
use iroha_crypto::{Algorithm, PublicKey, Signature, sha256};
use norito::codec::{Decode, Encode};
use std::fmt;

/// Maximum canonical public policy, trust or signed observation frame.
pub const SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum age and lifetime of an independently signed current-state observation: five minutes.
pub const SIGNER_RELEASE_STATE_MAX_AGE_MS_V1: u64 = 300_000;
const POLICY_MAGIC: [u8; 8] = *b"IRSREP01";
const TRUST_MAGIC: [u8; 8] = *b"IRSRET01";
const STATE_MAGIC: [u8; 8] = *b"IRSRES01";
const STATE_DOMAIN: &[u8] = b"iroha.sorafs.release-manifest.finalized-state.v1\0";

/// Independently reviewed request and exact public signer identity.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::release_evidence::SignerReleaseEvidencePolicyV1")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerReleaseEvidencePolicyV1 {
    /// Sole V1 marker; obtain it with [`Self::magic`].
    pub magic: [u8; 8],
    /// Exact canonical release-manifest role, deployment, key and signing policy.
    pub binding: SignerCustodyBindingV1,
    /// Unique operation issued by the coordinator before signing.
    pub operation_id: [u8; 32],
    /// SHA-256 of the exact reviewed manifest bytes, without JSON reserialization.
    pub manifest_sha256: [u8; 32],
    /// Exact reviewed manifest byte count.
    pub manifest_size: u64,
    /// Independently known finalized lower bound, including its exact block at equal height.
    pub minimum_anchor: SignerCustodyAnchorV1,
}
impl SignerReleaseEvidencePolicyV1 {
    /// Sole canonical V1 policy marker.
    pub const fn magic() -> [u8; 8] {
        POLICY_MAGIC
    }
}

/// Independently pinned observer and attestation trust; contains no runtime credentials.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::release_evidence::SignerReleaseEvidenceTrustV1")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerReleaseEvidenceTrustV1 {
    /// Sole V1 marker; obtain it with [`Self::magic`].
    pub magic: [u8; 8],
    /// Exact independent hardware attestation authority and policy.
    pub custody_authority: SignerCustodyAuthorityV1,
    /// Pinned Ed25519 hardware attestation public key.
    pub custody_public_key: PublicKey,
    /// Inclusive beginning of attestation-key eligibility.
    pub custody_active_from_unix_ms: u64,
    /// Exclusive end of attestation-key eligibility.
    pub custody_active_until_unix_ms: u64,
    /// Maximum hardware record lifetime, checked by the custody verifier.
    pub custody_max_validity_ms: u64,
    /// Separate state-observation authority and governance policy.
    pub state_authority: SignerCustodyAuthorityV1,
    /// Pinned Ed25519 state observer key, distinct from signer and attester.
    pub state_public_key: PublicKey,
    /// Inclusive beginning of observer-key eligibility.
    pub state_active_from_unix_ms: u64,
    /// Exclusive end of observer-key eligibility.
    pub state_active_until_unix_ms: u64,
    /// Positive observation lifetime/age bound, at most five minutes.
    pub max_state_age_ms: u64,
}
impl SignerReleaseEvidenceTrustV1 {
    /// Sole canonical V1 trust marker.
    pub const fn magic() -> [u8; 8] {
        TRUST_MAGIC
    }
}

/// Exact current finalized state signed by the independently trusted observer.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::release_evidence::SignerReleaseStateObservationBodyV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerReleaseStateObservationBodyV1 {
    /// Sole V1 marker; obtain it with [`Self::magic`].
    pub magic: [u8; 8],
    /// SHA-256 of the exact independently reviewed canonical policy frame.
    pub reviewed_policy_sha256: [u8; 32],
    /// Exact trusted observer identity and policy generation.
    pub authority: SignerCustodyAuthorityV1,
    /// Canonical chain label verified against the reviewed policy.
    pub chain_id: String,
    /// Exact genesis-derived network identity.
    pub network_id: [u8; 32],
    /// Exact release deployment from the purpose-specific reviewed binding.
    pub deployment_id: String,
    /// When authoritative state was actually observed, not when this file was copied.
    pub observed_at_unix_ms: u64,
    /// Exclusive validity end, at most the independently pinned age bound after observation.
    pub expires_at_unix_ms: u64,
    /// Current genuinely finalized custody-control state.
    pub current_anchor: SignerCustodyAnchorV1,
    /// Exact authoritative ACTIVE custody head under that control state.
    pub active_head: SignerCustodyActiveHeadV1,
    /// Current authoritative signer revocation flag.
    pub signer_revoked: bool,
    /// Current authoritative hardware attester revocation flag.
    pub attester_revoked: bool,
    /// Exact completed operation, authenticated under its finalized journal anchor.
    pub completed_operation: SignerCompletedOperationV1,
}
impl SignerReleaseStateObservationBodyV1 {
    /// Sole canonical V1 state marker.
    pub const fn magic() -> [u8; 8] {
        STATE_MAGIC
    }

    /// Domain-separated exact canonical bytes for the independent observer to sign.
    ///
    /// # Errors
    /// Rejects malformed markers, identities, times or oversized canonical frames.
    pub fn signing_payload(&self) -> Result<Vec<u8>, SignerReleaseEvidenceErrorV1> {
        if self.magic != STATE_MAGIC
            || self.reviewed_policy_sha256 == [0; 32]
            || !valid_identity(&self.deployment_id)
            || iroha_primitives::chain_id::validate_chain_id(&self.chain_id).is_err()
            || !valid_identity(&self.authority.service_id)
            || !valid_identity(&self.authority.administrator_id)
            || self.authority.key_revision == 0
            || self.authority.policy_revision == 0
            || self.authority.policy_digest == [0; 32]
            || self.network_id == [0; 32]
            || self.expires_at_unix_ms <= self.observed_at_unix_ms
            || self.expires_at_unix_ms - self.observed_at_unix_ms
                > SIGNER_RELEASE_STATE_MAX_AGE_MS_V1
        {
            return Err(SignerReleaseEvidenceErrorV1::InvalidState);
        }
        // Every variable-width field is bounded above before serialization. The remaining
        // observation, custody and completed-operation members contain only fixed-width values.
        let frame_size = {
            let _canonical =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::encoded_frame_len(self)
                .map_err(|_| SignerReleaseEvidenceErrorV1::InvalidDocument)?
        };
        if frame_size > SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1 - STATE_DOMAIN.len() {
            return Err(SignerReleaseEvidenceErrorV1::InvalidDocument);
        }
        let frame = norito::encode_canonical(self)
            .map_err(|_| SignerReleaseEvidenceErrorV1::InvalidDocument)?;
        if frame.len() > SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1 - STATE_DOMAIN.len() {
            return Err(SignerReleaseEvidenceErrorV1::InvalidDocument);
        }
        let mut message = Vec::with_capacity(STATE_DOMAIN.len() + frame.len());
        message.extend_from_slice(STATE_DOMAIN);
        message.extend_from_slice(&frame);
        Ok(message)
    }
}

/// Canonical signed finalized-state observation; its signing key is never selected from this file.
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "sorafs_manifest::signer::release_evidence::SignerReleaseStateObservationV1"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerReleaseStateObservationV1 {
    /// Exact signed observation.
    pub body: SignerReleaseStateObservationBodyV1,
    /// Raw Ed25519 signature by the separately pinned observer key.
    pub signature: [u8; 64],
}

/// Independent source pins and trusted verification time supplied by the release coordinator.
///
/// No decoder/default is provided: these expectations must not be derived from a receipt.
#[derive(Clone, Copy, Debug)]
pub struct SignerReleaseEvidenceExpectedV1 {
    /// SHA-256 of the independently reviewed canonical policy file.
    pub policy_sha256: [u8; 32],
    /// SHA-256 of the independently reviewed canonical trust file.
    pub trust_sha256: [u8; 32],
    /// Independently pinned SHA-256 of the raw Ed25519 manifest public key.
    pub public_key_fingerprint_sha256: [u8; 32],
    /// Trusted current time, independent of the candidate state and receipt.
    pub now_unix_ms: u64,
}

/// Bounded, secret-free evidence verification failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerReleaseEvidenceErrorV1 {
    /// A document is malformed, oversized or noncanonical.
    InvalidDocument,
    /// Independent reviewed policy, trust, key or exact manifest bytes do not match.
    SourceMismatch,
    /// Observer trust is invalid or not independently administered.
    InvalidTrust,
    /// Observation signature, identity, finality lower bound, freshness or status is invalid.
    InvalidState,
    /// The authenticated state does not establish this exact hardware signing receipt.
    Receipt(SignerReceiptErrorV1),
}
impl fmt::Display for SignerReleaseEvidenceErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::InvalidDocument => "invalid canonical release evidence document",
            Self::SourceMismatch => "release evidence differs from independently reviewed inputs",
            Self::InvalidTrust => "release evidence lacks independent observer trust",
            Self::InvalidState => "release evidence lacks authenticated fresh finalized state",
            Self::Receipt(_) => {
                "release evidence does not verify the exact hardware signing receipt"
            }
        })
    }
}
impl std::error::Error for SignerReleaseEvidenceErrorV1 {}

fn decode<T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize>(
    bytes: &[u8],
) -> Result<T, SignerReleaseEvidenceErrorV1> {
    if bytes.is_empty() || bytes.len() > SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1 {
        return Err(SignerReleaseEvidenceErrorV1::InvalidDocument);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            4096,
            SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
            1024,
            256 * 1024,
            24,
        ),
    )
    .map_err(|_| SignerReleaseEvidenceErrorV1::InvalidDocument)
}

/// Authenticate independent state observations, then verify the complete purpose-specific receipt.
///
/// # Errors
/// Rejects candidate-selected trust, stale/forked/revoked/substituted state, altered reviewed bytes,
/// shared signer/observer/attester authority and any canonical receipt verification failure.
#[allow(clippy::too_many_arguments)]
pub fn verify_release_manifest_evidence_v1(
    policy_bytes: &[u8],
    trust_bytes: &[u8],
    state_bytes: &[u8],
    receipt_bytes: &[u8],
    manifest: &[u8],
    detached_signature: &[u8],
    raw_public_key: &[u8; 32],
    expected: &SignerReleaseEvidenceExpectedV1,
) -> Result<VerifiedReleaseManifestSignerReceiptV1, SignerReleaseEvidenceErrorV1> {
    // Bounds precede hashing and decoding, including independently supplied files.
    if [policy_bytes, trust_bytes, state_bytes]
        .iter()
        .any(|b| b.is_empty() || b.len() > SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1)
        || manifest.is_empty()
        || manifest.len() > super::protocol::SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1
    {
        return Err(SignerReleaseEvidenceErrorV1::InvalidDocument);
    }
    if expected.policy_sha256 == [0; 32]
        || expected.trust_sha256 == [0; 32]
        || sha256(policy_bytes) != expected.policy_sha256
        || sha256(trust_bytes) != expected.trust_sha256
        || sha256(raw_public_key) != expected.public_key_fingerprint_sha256
    {
        return Err(SignerReleaseEvidenceErrorV1::SourceMismatch);
    }
    let policy: SignerReleaseEvidencePolicyV1 = decode(policy_bytes)?;
    let trust: SignerReleaseEvidenceTrustV1 = decode(trust_bytes)?;
    let observation: SignerReleaseStateObservationV1 = decode(state_bytes)?;
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, raw_public_key)
        .map_err(|_| SignerReleaseEvidenceErrorV1::SourceMismatch)?;
    let SignerPurposeBindingV1::ReleaseManifest { deployment_id } = &policy.binding.purpose else {
        return Err(SignerReleaseEvidenceErrorV1::SourceMismatch);
    };
    if policy.magic != POLICY_MAGIC
        || policy.binding.role != SignerRoleV1::ReleaseManifest
        || policy.binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        || policy.binding.public_key != public_key
        || policy.operation_id == [0; 32]
        || policy.manifest_sha256 != sha256(manifest)
        || u64::try_from(manifest.len()).ok() != Some(policy.manifest_size)
        || policy.minimum_anchor.height == 0
        || policy.minimum_anchor.block_hash == [0; 32]
        || policy.minimum_anchor.state_digest == [0; 32]
    {
        return Err(SignerReleaseEvidenceErrorV1::SourceMismatch);
    }
    validate_trust(&trust, &policy.binding, expected.now_unix_ms)?;
    let body = &observation.body;
    let anchor = body.current_anchor;
    if body.reviewed_policy_sha256 != expected.policy_sha256
        || body.authority != trust.state_authority
        || body.chain_id != policy.binding.chain_id
        || body.network_id != policy.binding.network_id
        || &body.deployment_id != deployment_id
        || body.observed_at_unix_ms > expected.now_unix_ms
        || expected.now_unix_ms >= body.expires_at_unix_ms
        || expected.now_unix_ms - body.observed_at_unix_ms > trust.max_state_age_ms
        || body.expires_at_unix_ms <= body.observed_at_unix_ms
        || body.expires_at_unix_ms - body.observed_at_unix_ms > trust.max_state_age_ms
        || body.observed_at_unix_ms < trust.state_active_from_unix_ms
        || body.completed_operation.completed_at_unix_ms > body.observed_at_unix_ms
        || body.expires_at_unix_ms > trust.state_active_until_unix_ms
        || body.signer_revoked
        || body.attester_revoked
        || anchor.height < policy.minimum_anchor.height
        || (anchor.height == policy.minimum_anchor.height && anchor != policy.minimum_anchor)
        || anchor.block_hash == [0; 32]
        || anchor.state_digest == [0; 32]
    {
        return Err(SignerReleaseEvidenceErrorV1::InvalidState);
    }
    let state_message = body.signing_payload()?;
    let state_signature = Signature::try_from_bytes(&observation.signature)
        .map_err(|_| SignerReleaseEvidenceErrorV1::InvalidState)?;
    state_signature
        .verify(&trust.state_public_key, &state_message)
        .map_err(|_| SignerReleaseEvidenceErrorV1::InvalidState)?;
    let custody_trust = SignerCustodyTrustV1 {
        authority: trust.custody_authority,
        public_key: trust.custody_public_key,
        active_from_unix_ms: trust.custody_active_from_unix_ms,
        active_until_unix_ms: trust.custody_active_until_unix_ms,
        max_validity_ms: trust.custody_max_validity_ms,
        max_anchor_age_ms: trust.max_state_age_ms,
    };
    let current = SignerCustodyUseContextV1 {
        now_unix_ms: expected.now_unix_ms,
        anchor_observed_at_unix_ms: body.observed_at_unix_ms,
        current_anchor: anchor,
        active_head: body.active_head,
        signer_revoked: body.signer_revoked,
        attester_revoked: body.attester_revoked,
    };
    let verified = verify_release_manifest_signer_receipt_v1(
        receipt_bytes,
        manifest,
        detached_signature,
        &SignerReleaseManifestExpectedV1 {
            operation_id: policy.operation_id,
            manifest_digest: signer_release_manifest_digest_v1(manifest),
            manifest_size: policy.manifest_size,
        },
        &policy.binding,
        &custody_trust,
        &current,
        &body.completed_operation,
    )
    .map_err(SignerReleaseEvidenceErrorV1::Receipt)?;
    if verified.custody().statement().issued_at_unix_ms > body.observed_at_unix_ms {
        return Err(SignerReleaseEvidenceErrorV1::InvalidState);
    }
    Ok(verified)
}

fn validate_trust(
    trust: &SignerReleaseEvidenceTrustV1,
    binding: &SignerCustodyBindingV1,
    now: u64,
) -> Result<(), SignerReleaseEvidenceErrorV1> {
    let authority = &trust.state_authority;
    let other_identities = [
        binding.service_id.as_str(),
        binding.administrator_id.as_str(),
        trust.custody_authority.service_id.as_str(),
        trust.custody_authority.administrator_id.as_str(),
    ];
    if trust.magic != TRUST_MAGIC
        || !valid_identity(&authority.service_id)
        || !valid_identity(&authority.administrator_id)
        || authority.key_revision == 0
        || authority.policy_revision == 0
        || authority.policy_digest == [0; 32]
        || authority.service_id == authority.administrator_id
        || [
            authority.service_id.as_str(),
            authority.administrator_id.as_str(),
        ]
        .iter()
        .any(|identity| other_identities.contains(identity))
        || trust.state_public_key.algorithm() != Algorithm::Ed25519
        || trust.state_public_key == binding.public_key
        || trust.state_public_key == trust.custody_public_key
        || trust.max_state_age_ms == 0
        || trust.max_state_age_ms > SIGNER_RELEASE_STATE_MAX_AGE_MS_V1
        || now < trust.state_active_from_unix_ms
        || now >= trust.state_active_until_unix_ms
    {
        return Err(SignerReleaseEvidenceErrorV1::InvalidTrust);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
include!("release_evidence/captured_owner_identity_tests.rs");
