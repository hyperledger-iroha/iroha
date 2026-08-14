//! Proof-of-Retrievability (PoR) challenge, proof, and audit verdict schemas.
use std::collections::BTreeSet;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use blake3::Hasher;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use norito::core::NoritoSerialize as _;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use crate::{
    CapacityMetadataEntry, XorQuantity,
    capacity::{MAX_REPLICATION_ORDER_METADATA_BYTES, MAX_REPLICATION_ORDER_METADATA_ENTRIES},
    chunker_registry,
    provider_advert::{AdvertSignature, SignatureAlgorithm},
};
const POR_CHALLENGE_SEED_DOMAIN: &[u8] = b"sorafs:por:seed:v1";
const POR_CHALLENGE_ID_DOMAIN: &[u8] = b"sorafs:por:id:v1";
/// Domain separator used by provider PoR proof signatures.
pub const POR_PROOF_SIGNATURE_DOMAIN_V1: &str = "sorafs.por.proof.signature.v1";
/// Domain separator used by auditor verdict signatures.
pub const POR_VERDICT_SIGNATURE_DOMAIN_V1: &str = "sorafs.por.verdict.signature.v1";
/// Domain separator used by provider VRF submission signatures.
pub const POR_VRF_SUBMISSION_SIGNATURE_DOMAIN_V1: &str = "sorafs.por.vrf-submission.signature.v1";
/// Domain separator mixed into the BLS VRF input before chain binding.
pub const POR_VRF_INPUT_DOMAIN_V1: &[u8] = b"sorafs.por.provider-vrf.input.v1\0";
/// Current PoR challenge schema version.
pub const POR_CHALLENGE_VERSION_V1: u8 = 1;
/// Current PoR challenge governance-publication envelope version.
pub const POR_CHALLENGE_PUBLICATION_VERSION_V1: u8 = 1;
/// Current PoR proof schema version.
pub const POR_PROOF_VERSION_V1: u8 = 1;
/// Current audit verdict schema version.
pub const AUDIT_VERDICT_VERSION_V1: u8 = 1;
/// Current challenge status schema version.
pub const POR_CHALLENGE_STATUS_VERSION_V1: u8 = 1;
/// Current weekly report schema version.
pub const POR_WEEKLY_REPORT_VERSION_V1: u8 = 1;
/// Maximum provider success rate expressed in basis points (100%).
pub const POR_SUCCESS_RATE_BPS_MAX: u16 = 10_000;
/// Current provider VRF submission schema version.
pub const POR_VRF_SUBMISSION_VERSION_V1: u8 = 1;
/// Maximum exact canonical size of one provider VRF submission.
pub const PROVIDER_VRF_SUBMISSION_MAX_CANONICAL_BYTES_V1: usize = 4 * 1024;
/// Maximum exact canonical size of one V1 PoR challenge.
pub const POR_CHALLENGE_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
/// Maximum canonical chunker-profile handle bytes in one V1 PoR challenge.
pub const POR_CHALLENGE_PROFILE_MAX_BYTES_V1: usize = 128;
/// Maximum sample rows admitted in one V1 PoR challenge.
pub const POR_CHALLENGE_MAX_SAMPLES_V1: usize = 500;
/// Maximum exact canonical size of one V1 PoR challenge publication.
pub const POR_CHALLENGE_PUBLICATION_MAX_CANONICAL_BYTES_V1: usize = 128 * 1024;
/// Maximum exact canonical size of one V1 PoR proof.
pub const POR_PROOF_MAX_CANONICAL_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum sample rows accepted in one V1 PoR proof.
pub const POR_PROOF_MAX_SAMPLES_V1: usize = 500;
/// Maximum Merkle authentication-path depth accepted in one V1 PoR proof.
pub const POR_PROOF_MAX_AUTH_PATH_NODES_V1: usize = 64;
/// Maximum exact canonical size of one V1 audit verdict.
pub const AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
/// Maximum UTF-8 byte length of one audit failure reason.
pub const AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1: usize = 2 * 1024;
/// Maximum distinct Ed25519 auditor signatures in one verdict.
pub const AUDIT_VERDICT_MAX_SIGNATURES_V1: usize = 64;
/// Maximum metadata rows in one audit verdict.
pub const AUDIT_VERDICT_MAX_METADATA_ENTRIES_V1: usize = MAX_REPLICATION_ORDER_METADATA_ENTRIES;
/// Maximum aggregate metadata key/value bytes in one audit verdict.
pub const AUDIT_VERDICT_MAX_METADATA_BYTES_V1: usize = MAX_REPLICATION_ORDER_METADATA_BYTES;
/// Maximum exact canonical size of one V1 PoR challenge status.
pub const POR_CHALLENGE_STATUS_MAX_CANONICAL_BYTES_V1: usize = 16 * 1024;
/// Maximum canonical UTF-8 bytes in one V1 status failure reason.
pub const POR_CHALLENGE_STATUS_FAILURE_REASON_MAX_BYTES_V1: usize = 2 * 1024;
/// Maximum status records returned in one canonical V1 status page.
pub const POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1: usize = 1_000;
/// Maximum sum of canonical status-record bytes returned by one Torii V1 page.
pub const POR_CHALLENGE_STATUS_PAGE_MAX_RECORD_BYTES_V1: usize = 4 * 1024 * 1024;
/// Current opaque PoR status-page cursor schema version.
pub const POR_STATUS_CURSOR_VERSION_V1: u8 = 1;
/// Maximum canonical base64url bytes in one opaque PoR status-page cursor.
pub const POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1: usize = 256;
/// Maximum decoded canonical Norito bytes in one PoR status-page cursor.
pub const POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1: usize = 192;
/// Maximum exact canonical size of one V1 status page.
pub const POR_CHALLENGE_STATUS_PAGE_MAX_CANONICAL_BYTES_V1: usize =
    POR_CHALLENGE_STATUS_MAX_CANONICAL_BYTES_V1 * POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1
        + 64 * 1024;
/// Maximum exact canonical size of one V1 weekly PoR report.
pub const POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum slashing-event rows in one V1 weekly PoR report.
pub const POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1: usize = 65_536;
/// Maximum missing-VRF provider rows in one V1 weekly PoR report.
pub const POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1: usize = 65_536;
/// Maximum UTF-8 byte length of ticket and verdict-CID identifiers.
pub const POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1: usize = 256;
/// Maximum UTF-8 byte length of weekly governance notes.
pub const POR_WEEKLY_REPORT_NOTES_MAX_BYTES_V1: usize = 4 * 1024;
/// Build the canonical provider VRF input for one manifest and drand round.
#[must_use]
pub fn provider_vrf_input(
    provider_id: &[u8; 32],
    manifest_digest: &[u8; 32],
    epoch_id: u64,
    drand_round: u64,
) -> Vec<u8> {
    let mut input = Vec::with_capacity(POR_VRF_INPUT_DOMAIN_V1.len() + 32 + 32 + 8 + 8);
    input.extend_from_slice(POR_VRF_INPUT_DOMAIN_V1);
    input.extend_from_slice(provider_id);
    input.extend_from_slice(manifest_digest);
    input.extend_from_slice(&epoch_id.to_be_bytes());
    input.extend_from_slice(&drand_round.to_be_bytes());
    input
}
/// Authenticated provider submission carrying one admission-bound BLS VRF proof.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ProviderVrfSubmissionV1 {
    /// Schema version (`POR_VRF_SUBMISSION_VERSION_V1`).
    pub version: u8,
    /// Exact 32-byte genesis-derived network identity.
    pub network_id: [u8; 32],
    /// Governance-controlled provider identifier.
    pub provider_id: [u8; 32],
    /// Manifest digest for which the provider generated the proof.
    pub manifest_digest: [u8; 32],
    /// PoR epoch identifier.
    pub epoch_id: u64,
    /// Verified drand round mixed into the proof input.
    pub drand_round: u64,
    /// VRF output derived from `proof`.
    pub output: [u8; 32],
    /// Variant-tagged, fixed-size BLS VRF proof.
    pub proof: iroha_crypto::vrf::VrfProof,
    /// Strictly increasing provider sequence used for durable replay rejection.
    pub sequence: u64,
    /// Unix timestamp at which the provider signed this submission.
    pub issued_at: u64,
    /// Current admission-approved Ed25519 advert key signature.
    pub signature: AdvertSignature,
}
#[derive(Debug, Clone, NoritoSerialize)]
struct ProviderVrfSubmissionSigningPayloadV1 {
    domain: String,
    version: u8,
    network_id: [u8; 32],
    provider_id: [u8; 32],
    manifest_digest: [u8; 32],
    epoch_id: u64,
    drand_round: u64,
    output: [u8; 32],
    proof: iroha_crypto::vrf::VrfProof,
    sequence: u64,
    issued_at: u64,
}
impl From<&ProviderVrfSubmissionV1> for ProviderVrfSubmissionSigningPayloadV1 {
    fn from(submission: &ProviderVrfSubmissionV1) -> Self {
        Self {
            domain: POR_VRF_SUBMISSION_SIGNATURE_DOMAIN_V1.to_owned(),
            version: submission.version,
            network_id: submission.network_id,
            provider_id: submission.provider_id,
            manifest_digest: submission.manifest_digest,
            epoch_id: submission.epoch_id,
            drand_round: submission.drand_round,
            output: submission.output,
            proof: submission.proof,
            sequence: submission.sequence,
            issued_at: submission.issued_at,
        }
    }
}
impl ProviderVrfSubmissionV1 {
    /// Validate bounded structural fields before admission and proof checks.
    pub fn validate(&self) -> Result<(), ProviderVrfSubmissionValidationError> {
        preflight_provider_vrf_submission_len(
            self,
            PROVIDER_VRF_SUBMISSION_MAX_CANONICAL_BYTES_V1,
        )?;
        if self.version != POR_VRF_SUBMISSION_VERSION_V1 {
            return Err(ProviderVrfSubmissionValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.network_id.iter().all(|byte| *byte == 0) {
            return Err(ProviderVrfSubmissionValidationError::InvalidNetworkId);
        }
        if self.provider_id.iter().all(|byte| *byte == 0) {
            return Err(ProviderVrfSubmissionValidationError::InvalidProviderId);
        }
        if self.manifest_digest.iter().all(|byte| *byte == 0) {
            return Err(ProviderVrfSubmissionValidationError::InvalidManifestDigest);
        }
        if self.epoch_id == 0 {
            return Err(ProviderVrfSubmissionValidationError::InvalidEpoch);
        }
        if self.drand_round == 0 {
            return Err(ProviderVrfSubmissionValidationError::InvalidDrandRound);
        }
        if self.output.iter().all(|byte| *byte == 0) {
            return Err(ProviderVrfSubmissionValidationError::InvalidOutput);
        }
        let proof_is_inert = match &self.proof {
            iroha_crypto::vrf::VrfProof::SigInG1(bytes) => bytes.iter().all(|byte| *byte == 0),
            iroha_crypto::vrf::VrfProof::SigInG2(bytes) => bytes.iter().all(|byte| *byte == 0),
        };
        if proof_is_inert {
            return Err(ProviderVrfSubmissionValidationError::InvalidProof);
        }
        if self.sequence == 0 {
            return Err(ProviderVrfSubmissionValidationError::InvalidSequence);
        }
        if self.issued_at == 0 {
            return Err(ProviderVrfSubmissionValidationError::InvalidIssuedAt);
        }
        if self.signature.algorithm != SignatureAlgorithm::Ed25519 {
            return Err(ProviderVrfSubmissionValidationError::UnsupportedSignatureAlgorithm);
        }
        if self.signature.public_key.len() != PUBLIC_KEY_LENGTH {
            return Err(
                ProviderVrfSubmissionValidationError::InvalidSignaturePublicKeyLength {
                    found: self.signature.public_key.len(),
                    expected: PUBLIC_KEY_LENGTH,
                },
            );
        }
        if self.signature.signature.len() != SIGNATURE_LENGTH {
            return Err(
                ProviderVrfSubmissionValidationError::InvalidSignatureLength {
                    found: self.signature.signature.len(),
                    expected: SIGNATURE_LENGTH,
                },
            );
        }
        if crate::inert_bytes(&self.signature.public_key)
            || crate::inert_bytes(&self.signature.signature)
        {
            return Err(ProviderVrfSubmissionValidationError::InvalidSignature);
        }
        Ok(())
    }
    /// Return canonical domain-separated bytes signed by the provider advert key.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&ProviderVrfSubmissionSigningPayloadV1::from(self))
    }
    /// Verify the Ed25519 signature and bind it to the current admitted advert key.
    pub fn verify_signature_for_provider(
        &self,
        admitted_provider_key: &[u8],
    ) -> Result<(), PorSignatureVerificationError> {
        if admitted_provider_key.len() != PUBLIC_KEY_LENGTH {
            return Err(PorSignatureVerificationError::InvalidPublicKeyLength {
                length: admitted_provider_key.len(),
            });
        }
        if self.signature.public_key != admitted_provider_key {
            return Err(PorSignatureVerificationError::ProviderSignerMismatch);
        }
        let admitted: [u8; PUBLIC_KEY_LENGTH] = admitted_provider_key
            .try_into()
            .expect("length checked above");
        crate::checked_ed25519_verifying_key_from_bytes(&admitted)
            .map_err(|reason| PorSignatureVerificationError::InvalidPublicKey { reason })?;
        let payload = self.signature_payload_bytes().map_err(|error| {
            PorSignatureVerificationError::PayloadEncoding {
                reason: error.to_string(),
            }
        })?;
        verify_ed25519_signature(&self.signature, &payload)?;
        Ok(())
    }
}
/// Structural validation failures for provider VRF submissions.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProviderVrfSubmissionValidationError {
    /// Exact canonical encoded length could not be computed.
    #[error("provider VRF submission exact canonical encoded length is unavailable")]
    CanonicalLengthUnavailable,
    /// Canonical payload exceeds the V1 byte ceiling.
    #[error("provider VRF submission has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    /// The submission version is unsupported.
    #[error("unsupported provider VRF submission version {found}")]
    UnsupportedVersion {
        /// Version decoded from the cursor payload.
        found: u8,
    },
    /// Exact network identity is inert.
    #[error("provider VRF submission network id must be non-zero")]
    InvalidNetworkId,
    /// Provider identifier is inert.
    #[error("provider VRF submission provider id must be non-zero")]
    InvalidProviderId,
    /// Manifest digest is inert.
    #[error("provider VRF submission manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Epoch zero is reserved.
    #[error("provider VRF submission epoch must be non-zero")]
    InvalidEpoch,
    /// Drand round zero is invalid.
    #[error("provider VRF submission drand round must be non-zero")]
    InvalidDrandRound,
    /// Output is inert.
    #[error("provider VRF submission output must be non-zero")]
    InvalidOutput,
    /// Proof is inert.
    #[error("provider VRF submission proof must be non-zero")]
    InvalidProof,
    /// Sequence zero is reserved.
    #[error("provider VRF submission sequence must be non-zero")]
    InvalidSequence,
    /// Issued timestamp is invalid.
    #[error("provider VRF submission issued_at must be non-zero")]
    InvalidIssuedAt,
    /// Signature fields are missing or unsupported.
    #[error("provider VRF submission signature material must not be inert")]
    InvalidSignature,
    /// The reserved signature algorithm is not admitted in V1.
    #[error("provider VRF submission signature must use Ed25519")]
    UnsupportedSignatureAlgorithm,
    /// Ed25519 public key length is not exact.
    #[error("provider VRF submission public key has {found} bytes; expected {expected}")]
    InvalidSignaturePublicKeyLength { found: usize, expected: usize },
    /// Ed25519 signature length is not exact.
    #[error("provider VRF submission signature has {found} bytes; expected {expected}")]
    InvalidSignatureLength { found: usize, expected: usize },
}
fn preflight_provider_vrf_submission_len(
    submission: &ProviderVrfSubmissionV1,
    maximum: usize,
) -> Result<usize, ProviderVrfSubmissionValidationError> {
    let found = submission
        .encoded_len_exact()
        .ok_or(ProviderVrfSubmissionValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(ProviderVrfSubmissionValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Decode and validate one bounded canonical provider VRF submission.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid submission bytes.
pub fn decode_provider_vrf_submission_v1(
    bytes: &[u8],
) -> Result<ProviderVrfSubmissionV1, norito::core::Error> {
    let submission: ProviderVrfSubmissionV1 = decode_bounded_canonical_por_payload(
        "provider VRF submission",
        bytes,
        PROVIDER_VRF_SUBMISSION_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            SIGNATURE_LENGTH,
            256,
            512,
            PROVIDER_VRF_SUBMISSION_MAX_CANONICAL_BYTES_V1 * 4,
            32,
        ),
    )?;
    submission
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(submission)
}
/// Derives the PoR challenge seed by mixing drand randomness, provider VRF output,
/// manifest digest, and epoch identifier.
#[must_use]
pub fn derive_challenge_seed(
    drand_randomness: &[u8; 32],
    vrf_output: Option<&[u8; 32]>,
    manifest_digest: &[u8; 32],
    epoch_id: u64,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_CHALLENGE_SEED_DOMAIN);
    hasher.update(drand_randomness);
    match vrf_output {
        Some(output) => {
            hasher.update(output);
        }
        None => {
            hasher.update(&[0u8; 32]);
        }
    }
    hasher.update(manifest_digest);
    hasher.update(&epoch_id.to_le_bytes());
    hasher.finalize().into()
}
/// Derives a canonical challenge identifier from seed material and metadata.
#[must_use]
pub fn derive_challenge_id(
    seed: &[u8; 32],
    manifest_digest: &[u8; 32],
    provider_id: &[u8; 32],
    epoch_id: u64,
    drand_round: u64,
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_CHALLENGE_ID_DOMAIN);
    hasher.update(seed);
    hasher.update(manifest_digest);
    hasher.update(provider_id);
    hasher.update(&epoch_id.to_le_bytes());
    hasher.update(&drand_round.to_le_bytes());
    hasher.finalize().into()
}
/// PoR challenge issued to a storage provider.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorChallengeV1 {
    /// Schema version (`POR_CHALLENGE_VERSION_V1`).
    pub version: u8,
    /// Unique challenge identifier (BLAKE3-256 digest).
    pub challenge_id: [u8; 32],
    /// Manifest digest (BLAKE3-256) for the target asset.
    pub manifest_digest: [u8; 32],
    /// Provider identifier authorised by governance.
    pub provider_id: [u8; 32],
    /// Epoch identifier used for randomness mixing.
    #[norito(default)]
    pub epoch_id: u64,
    /// drand round number sourced for this challenge.
    #[norito(default)]
    pub drand_round: u64,
    /// drand randomness bytes used to derive the seed.
    #[norito(default)]
    pub drand_randomness: [u8; 32],
    /// drand BLS signature covering the randomness.
    pub drand_signature: [u8; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
    /// Provider VRF output for this manifest/epoch (optional when forced).
    #[norito(default)]
    pub vrf_output: Option<[u8; 32]>,
    /// Variant-tagged provider VRF proof (absent only when forced).
    #[norito(default)]
    pub vrf_proof: Option<iroha_crypto::vrf::VrfProof>,
    /// Whether the coordinator forced the challenge due to missing VRF.
    #[norito(default)]
    pub forced: bool,
    /// Canonical chunking profile (`namespace.name@semver`).
    pub chunking_profile: String,
    /// Pseudo-random challenge seed (32 bytes).
    pub seed: [u8; 32],
    /// Sampling tier (tracks sample strategy).
    pub sample_tier: u16,
    /// Number of samples requested.
    pub sample_count: u16,
    /// Sample indices (leaf offsets) selected for verification.
    pub sample_indices: Vec<u64>,
    /// Unix timestamp (seconds) when the challenge was issued.
    pub issued_at: u64,
    /// Unix timestamp (seconds) when the proof must be submitted.
    pub deadline_at: u64,
}
impl PorChallengeV1 {
    /// Validates the challenge payload.
    pub fn validate(&self) -> Result<(), PorChallengeValidationError> {
        preflight_por_challenge_len(self, POR_CHALLENGE_MAX_CANONICAL_BYTES_V1)?;
        if self.version != POR_CHALLENGE_VERSION_V1 {
            return Err(PorChallengeValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeValidationError::InvalidProviderId);
        }
        if self.challenge_id.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeValidationError::InvalidChallengeId);
        }
        validate_por_challenge_profile(&self.chunking_profile)?;
        chunker_registry::lookup_by_handle(&self.chunking_profile).ok_or_else(|| {
            PorChallengeValidationError::UnknownChunkerHandle {
                handle: self.chunking_profile.clone(),
            }
        })?;
        if self.epoch_id == 0 {
            return Err(PorChallengeValidationError::MissingEpochId);
        }
        if self.drand_round == 0 {
            return Err(PorChallengeValidationError::MissingDrandRound);
        }
        if self.drand_randomness.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeValidationError::InvalidDrandRandomness);
        }
        if self.drand_signature.iter().all(|byte| *byte == 0) {
            return Err(PorChallengeValidationError::InvalidDrandSignature);
        }
        match (&self.vrf_output, &self.vrf_proof, self.forced) {
            (Some(output), Some(proof), false) => {
                if output.iter().all(|&byte| byte == 0) {
                    return Err(PorChallengeValidationError::InvalidVrfOutput);
                }
                let inert = match proof {
                    iroha_crypto::vrf::VrfProof::SigInG1(bytes) => {
                        bytes.iter().all(|byte| *byte == 0)
                    }
                    iroha_crypto::vrf::VrfProof::SigInG2(bytes) => {
                        bytes.iter().all(|byte| *byte == 0)
                    }
                };
                if inert {
                    return Err(PorChallengeValidationError::InvalidVrfProof);
                }
            }
            (None, None, true) => {}
            (Some(_), Some(_), true) => {
                return Err(PorChallengeValidationError::ForcedWithVrf);
            }
            (None, Some(_), true) => {
                return Err(PorChallengeValidationError::ForcedWithOrphanProof);
            }
            (Some(_), None, _) => {
                return Err(PorChallengeValidationError::MissingVrfProof);
            }
            (None, _, false) => {
                return Err(PorChallengeValidationError::MissingVrfOutput);
            }
        }
        let expected_seed = derive_challenge_seed(
            &self.drand_randomness,
            self.vrf_output.as_ref(),
            &self.manifest_digest,
            self.epoch_id,
        );
        if expected_seed != self.seed {
            return Err(PorChallengeValidationError::SeedMismatch);
        }
        let expected_id = derive_challenge_id(
            &self.seed,
            &self.manifest_digest,
            &self.provider_id,
            self.epoch_id,
            self.drand_round,
        );
        if expected_id != self.challenge_id {
            return Err(PorChallengeValidationError::ChallengeIdMismatch);
        }
        if self.sample_tier == 0 {
            return Err(PorChallengeValidationError::InvalidSampleTier);
        }
        if self.sample_count == 0 {
            return Err(PorChallengeValidationError::ZeroSampleCount);
        }
        if usize::from(self.sample_count) > POR_CHALLENGE_MAX_SAMPLES_V1 {
            return Err(PorChallengeValidationError::TooManyDeclaredSamples {
                found: self.sample_count,
                maximum: POR_CHALLENGE_MAX_SAMPLES_V1,
            });
        }
        if self.sample_indices.len() > POR_CHALLENGE_MAX_SAMPLES_V1 {
            return Err(PorChallengeValidationError::TooManySampleIndices {
                found: self.sample_indices.len(),
                maximum: POR_CHALLENGE_MAX_SAMPLES_V1,
            });
        }
        if self.sample_indices.len() != usize::from(self.sample_count) {
            return Err(PorChallengeValidationError::SampleCountMismatch {
                expected: self.sample_count,
                actual: u16::try_from(self.sample_indices.len())
                    .expect("bounded challenge sample inventory fits u16"),
            });
        }
        if self.issued_at >= self.deadline_at {
            return Err(PorChallengeValidationError::InvalidDeadline {
                issued_at: self.issued_at,
                deadline_at: self.deadline_at,
            });
        }
        Ok(())
    }
}
/// Validation errors for [`PorChallengeV1`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PorChallengeValidationError {
    #[error("PoR challenge does not expose an exact canonical encoded length")]
    CanonicalLengthUnavailable,
    #[error("PoR challenge has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    #[error("unsupported challenge version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("epoch id must be non-zero")]
    MissingEpochId,
    #[error("drand round must be non-zero")]
    MissingDrandRound,
    #[error("drand randomness must be non-zero")]
    InvalidDrandRandomness,
    #[error("drand signature must not be all zero")]
    InvalidDrandSignature,
    #[error("provider VRF output required unless challenge marked forced")]
    MissingVrfOutput,
    #[error("provider VRF output must be non-zero")]
    InvalidVrfOutput,
    #[error("provider VRF proof required when VRF output supplied")]
    MissingVrfProof,
    #[error("provider VRF proof must be a non-identity canonical proof")]
    InvalidVrfProof,
    #[error("forced challenge must not contain a provider VRF output/proof")]
    ForcedWithVrf,
    #[error("forced challenge must not contain an orphan provider VRF proof")]
    ForcedWithOrphanProof,
    #[error("seed does not match deterministic derivation")]
    SeedMismatch,
    #[error("challenge id does not match deterministic derivation")]
    ChallengeIdMismatch,
    #[error("chunker profile is noncanonical or has {found} bytes; maximum is {maximum}")]
    InvalidChunkerProfile { found: usize, maximum: usize },
    #[error("unknown chunker profile handle: {handle}")]
    UnknownChunkerHandle { handle: String },
    #[error("sample tier must be non-zero")]
    InvalidSampleTier,
    #[error("challenge must contain at least one sample")]
    ZeroSampleCount,
    #[error("challenge declares {found} samples; maximum is {maximum}")]
    TooManyDeclaredSamples { found: u16, maximum: usize },
    #[error("challenge carries {found} sample indices; maximum is {maximum}")]
    TooManySampleIndices { found: usize, maximum: usize },
    #[error("sample count mismatch (expected {expected}, actual {actual})")]
    SampleCountMismatch { expected: u16, actual: u16 },
    #[error("deadline {deadline_at} must be greater than issued_at {issued_at}")]
    InvalidDeadline { issued_at: u64, deadline_at: u64 },
}
fn validate_por_challenge_profile(profile: &str) -> Result<(), PorChallengeValidationError> {
    if profile.is_empty()
        || profile.len() > POR_CHALLENGE_PROFILE_MAX_BYTES_V1
        || profile.trim() != profile
        || profile.chars().any(char::is_control)
    {
        return Err(PorChallengeValidationError::InvalidChunkerProfile {
            found: profile.len(),
            maximum: POR_CHALLENGE_PROFILE_MAX_BYTES_V1,
        });
    }
    Ok(())
}
fn preflight_por_challenge_len(
    challenge: &PorChallengeV1,
    maximum: usize,
) -> Result<usize, PorChallengeValidationError> {
    let found = challenge
        .encoded_len_exact()
        .ok_or(PorChallengeValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(PorChallengeValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Canonical Governance DAG publication envelope for one PoR challenge.
///
/// `duplicate_samples` is encoded as a fixed-width integer and must exactly
/// match the duplicate count in the validated challenge sample inventory.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorChallengePublicationV1 {
    /// Schema version (`POR_CHALLENGE_PUBLICATION_VERSION_V1`).
    pub version: u8,
    /// Canonical PoR challenge.
    pub challenge: PorChallengeV1,
    /// Number of repeated sample indices after unique leaves were exhausted.
    pub duplicate_samples: u16,
}
impl PorChallengePublicationV1 {
    /// Construct and validate a canonical challenge-publication envelope.
    pub fn try_new(
        challenge: PorChallengeV1,
        duplicate_samples: usize,
    ) -> Result<Self, PorChallengePublicationValidationError> {
        let duplicate_samples = u16::try_from(duplicate_samples).map_err(|_| {
            PorChallengePublicationValidationError::DuplicateSampleCountOutOfRange {
                count: duplicate_samples,
            }
        })?;
        let publication = Self {
            version: POR_CHALLENGE_PUBLICATION_VERSION_V1,
            challenge,
            duplicate_samples,
        };
        publication.validate()?;
        Ok(publication)
    }
    /// Validate the envelope and its exact duplicate-sample binding.
    pub fn validate(&self) -> Result<(), PorChallengePublicationValidationError> {
        preflight_por_challenge_publication_len(
            self,
            POR_CHALLENGE_PUBLICATION_MAX_CANONICAL_BYTES_V1,
        )?;
        if self.version != POR_CHALLENGE_PUBLICATION_VERSION_V1 {
            return Err(PorChallengePublicationValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        self.challenge
            .validate()
            .map_err(PorChallengePublicationValidationError::InvalidChallenge)?;
        if self.duplicate_samples > self.challenge.sample_count {
            return Err(
                PorChallengePublicationValidationError::DuplicateSampleCountExceedsSampleCount {
                    duplicate_samples: self.duplicate_samples,
                    sample_count: self.challenge.sample_count,
                },
            );
        }
        let unique_samples = self
            .challenge
            .sample_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len();
        let actual = self
            .challenge
            .sample_indices
            .len()
            .saturating_sub(unique_samples);
        let actual = u16::try_from(actual).map_err(|_| {
            PorChallengePublicationValidationError::DuplicateSampleCountOutOfRange { count: actual }
        })?;
        if self.duplicate_samples != actual {
            return Err(
                PorChallengePublicationValidationError::DuplicateSampleCountMismatch {
                    declared: self.duplicate_samples,
                    actual,
                },
            );
        }
        Ok(())
    }
}
/// Validation failures for [`PorChallengePublicationV1`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PorChallengePublicationValidationError {
    /// Canonical encoder cannot provide an allocation-free exact length.
    #[error("PoR challenge publication has no exact canonical encoded length")]
    CanonicalLengthUnavailable,
    /// Canonical publication bytes exceed the V1 ceiling.
    #[error("PoR challenge publication has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    /// The publication version is unsupported.
    #[error("unsupported PoR challenge publication version {found}")]
    UnsupportedVersion { found: u8 },
    /// The embedded challenge is invalid.
    #[error("invalid published PoR challenge: {0}")]
    InvalidChallenge(#[source] PorChallengeValidationError),
    /// The runtime duplicate counter cannot be represented canonically.
    #[error("PoR duplicate sample count {count} exceeds the canonical u16 range")]
    DuplicateSampleCountOutOfRange { count: usize },
    /// The declared duplicate count cannot exceed the total sample count.
    #[error(
        "PoR duplicate sample count {duplicate_samples} exceeds challenge sample count {sample_count}"
    )]
    DuplicateSampleCountExceedsSampleCount {
        duplicate_samples: u16,
        sample_count: u16,
    },
    /// The declared duplicate count does not match the sample inventory.
    #[error("PoR duplicate sample count mismatch (declared {declared}, actual {actual})")]
    DuplicateSampleCountMismatch { declared: u16, actual: u16 },
}
fn preflight_por_challenge_publication_len(
    publication: &PorChallengePublicationV1,
    maximum: usize,
) -> Result<usize, PorChallengePublicationValidationError> {
    let found = publication
        .encoded_len_exact()
        .ok_or(PorChallengePublicationValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(PorChallengePublicationValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Decode and validate one bounded canonical V1 PoR challenge.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid challenge bytes.
pub fn decode_por_challenge_v1(bytes: &[u8]) -> Result<PorChallengeV1, norito::core::Error> {
    let challenge: PorChallengeV1 = decode_bounded_canonical_por_payload(
        "PoR challenge",
        bytes,
        POR_CHALLENGE_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            POR_CHALLENGE_MAX_SAMPLES_V1,
            POR_CHALLENGE_MAX_CANONICAL_BYTES_V1,
            2_048,
            POR_CHALLENGE_MAX_CANONICAL_BYTES_V1 * 4,
            32,
        ),
    )?;
    challenge
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(challenge)
}
/// Decode and validate one bounded canonical V1 PoR challenge publication.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid publication bytes.
pub fn decode_por_challenge_publication_v1(
    bytes: &[u8],
) -> Result<PorChallengePublicationV1, norito::core::Error> {
    let publication: PorChallengePublicationV1 = decode_bounded_canonical_por_payload(
        "PoR challenge publication",
        bytes,
        POR_CHALLENGE_PUBLICATION_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            POR_CHALLENGE_MAX_SAMPLES_V1,
            POR_CHALLENGE_PUBLICATION_MAX_CANONICAL_BYTES_V1,
            2_048,
            POR_CHALLENGE_PUBLICATION_MAX_CANONICAL_BYTES_V1 * 4,
            32,
        ),
    )?;
    publication
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(publication)
}
fn decode_bounded_canonical_por_payload<T>(
    payload: &'static str,
    bytes: &[u8],
    maximum: usize,
    limits: norito::DecodeLimits,
) -> Result<T, norito::core::Error>
where
    T: for<'decode> norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    if bytes.len() > maximum {
        return Err(norito::core::Error::Message(format!(
            "{payload} has {} canonical bytes; maximum is {maximum}",
            bytes.len()
        )));
    }
    let value: T = norito::decode_from_bytes_with_limits(bytes, limits)?;
    let exact = value.encoded_len_exact().ok_or_else(|| {
        norito::core::Error::Message(format!(
            "{payload} does not expose an exact canonical encoded length"
        ))
    })?;
    if exact > maximum {
        return Err(norito::core::Error::Message(format!(
            "{payload} has {exact} canonical bytes; maximum is {maximum}"
        )));
    }
    let canonical = norito::to_bytes(&value)?;
    if canonical != bytes {
        return Err(norito::core::Error::Message(format!(
            "{payload} is not canonically encoded"
        )));
    }
    Ok(value)
}
/// Sample proof attached to a PoR response.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorProofSampleV1 {
    /// Leaf index sampled by the challenge.
    pub sample_index: u64,
    /// Manifest byte offset for the sampled chunk.
    pub chunk_offset: u64,
    /// Size of the sampled chunk (bytes).
    pub chunk_size: u32,
    /// Blake3 digest of the chunk.
    pub chunk_digest: [u8; 32],
    /// Blake3 digest of the leaf node (post alignment).
    pub leaf_digest: [u8; 32],
}
impl PorProofSampleV1 {
    fn validate(&self) -> Result<(), PorProofValidationError> {
        if self.chunk_size == 0 {
            return Err(PorProofValidationError::InvalidChunkSize {
                sample_index: self.sample_index,
            });
        }
        if self.chunk_digest.iter().all(|&byte| byte == 0) {
            return Err(PorProofValidationError::InvalidChunkDigest {
                sample_index: self.sample_index,
            });
        }
        if self.leaf_digest.iter().all(|&byte| byte == 0) {
            return Err(PorProofValidationError::InvalidLeafDigest {
                sample_index: self.sample_index,
            });
        }
        Ok(())
    }
}
/// PoR proof submitted by the provider.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorProofV1 {
    /// Schema version (`POR_PROOF_VERSION_V1`).
    pub version: u8,
    /// Challenge identifier this proof responds to.
    pub challenge_id: [u8; 32],
    /// Manifest digest (BLAKE3-256).
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Samples proven in this response.
    pub samples: Vec<PorProofSampleV1>,
    /// Merkle authentication path covering the sampled leaves.
    pub auth_path: Vec<[u8; 32]>,
    /// Provider signature over the canonical domain-separated unsigned proof payload.
    pub signature: AdvertSignature,
    /// Unix timestamp (seconds) when the proof was submitted.
    pub submitted_at: u64,
}
#[derive(Debug, Clone, NoritoSerialize)]
struct PorProofSigningPayloadV1 {
    domain: String,
    version: u8,
    challenge_id: [u8; 32],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    samples: Vec<PorProofSampleV1>,
    auth_path: Vec<[u8; 32]>,
    submitted_at: u64,
}
mod borrowed_norito {
    use norito::core::NoritoSerialize;
    /// Borrowed string that preserves the owned `String` wire representation.
    pub(super) struct String<'a>(pub(super) &'a str);
    impl NoritoSerialize for String<'_> {
        fn schema_hash() -> [u8; 16] {
            <std::string::String>::schema_hash()
        }
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            self.0.serialize(writer)
        }
        fn encoded_len_hint(&self) -> std::option::Option<usize> {
            self.0.encoded_len_hint()
        }
        fn encoded_len_exact(&self) -> std::option::Option<usize> {
            self.0.encoded_len_exact()
        }
    }
    /// Borrowed vector that preserves the owned `Vec<T>` wire representation.
    pub(super) struct Vec<'a, T>(pub(super) &'a std::vec::Vec<T>);
    impl<T: NoritoSerialize> NoritoSerialize for Vec<'_, T> {
        fn schema_hash() -> [u8; 16] {
            <std::vec::Vec<T>>::schema_hash()
        }
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            self.0.serialize(writer)
        }
        fn encoded_len_hint(&self) -> std::option::Option<usize> {
            self.0.encoded_len_hint()
        }
        fn encoded_len_exact(&self) -> std::option::Option<usize> {
            self.0.encoded_len_exact()
        }
    }
    /// Borrowed option that preserves the owned `Option<T>` wire representation.
    pub(super) struct Option<'a, T>(pub(super) &'a std::option::Option<T>);
    impl<T: NoritoSerialize> NoritoSerialize for Option<'_, T> {
        fn schema_hash() -> [u8; 16] {
            <std::option::Option<T>>::schema_hash()
        }
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            self.0.serialize(writer)
        }
        fn encoded_len_hint(&self) -> std::option::Option<usize> {
            self.0.encoded_len_hint()
        }
        fn encoded_len_exact(&self) -> std::option::Option<usize> {
            self.0.encoded_len_exact()
        }
    }
}
#[derive(NoritoSerialize)]
struct PorProofSigningPayloadViewWireV1<'a> {
    domain: borrowed_norito::String<'a>,
    version: u8,
    challenge_id: [u8; 32],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    samples: borrowed_norito::Vec<'a, PorProofSampleV1>,
    auth_path: borrowed_norito::Vec<'a, [u8; 32]>,
    submitted_at: u64,
}
struct PorProofSigningPayloadViewV1<'a>(PorProofSigningPayloadViewWireV1<'a>);
impl<'a> From<&'a PorProofV1> for PorProofSigningPayloadViewV1<'a> {
    fn from(proof: &'a PorProofV1) -> Self {
        Self(PorProofSigningPayloadViewWireV1 {
            domain: borrowed_norito::String(POR_PROOF_SIGNATURE_DOMAIN_V1),
            version: proof.version,
            challenge_id: proof.challenge_id,
            manifest_digest: proof.manifest_digest,
            provider_id: proof.provider_id,
            samples: borrowed_norito::Vec(&proof.samples),
            auth_path: borrowed_norito::Vec(&proof.auth_path),
            submitted_at: proof.submitted_at,
        })
    }
}
impl norito::core::NoritoSerialize for PorProofSigningPayloadViewV1<'_> {
    fn schema_hash() -> [u8; 16] {
        PorProofSigningPayloadV1::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
#[cfg(test)]
impl From<&PorProofV1> for PorProofSigningPayloadV1 {
    fn from(proof: &PorProofV1) -> Self {
        Self {
            domain: POR_PROOF_SIGNATURE_DOMAIN_V1.to_owned(),
            version: proof.version,
            challenge_id: proof.challenge_id,
            manifest_digest: proof.manifest_digest,
            provider_id: proof.provider_id,
            samples: proof.samples.clone(),
            auth_path: proof.auth_path.clone(),
            submitted_at: proof.submitted_at,
        }
    }
}
impl PorProofV1 {
    /// Validates the proof payload.
    pub fn validate(&self) -> Result<(), PorProofValidationError> {
        preflight_por_proof_len(self, POR_PROOF_MAX_CANONICAL_BYTES_V1)?;
        if self.version != POR_PROOF_VERSION_V1 {
            return Err(PorProofValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.challenge_id.iter().all(|&byte| byte == 0) {
            return Err(PorProofValidationError::InvalidChallengeId);
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(PorProofValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(PorProofValidationError::InvalidProviderId);
        }
        if self.samples.is_empty() {
            return Err(PorProofValidationError::MissingSamples);
        }
        if self.samples.len() > POR_PROOF_MAX_SAMPLES_V1 {
            return Err(PorProofValidationError::TooManySamples {
                found: self.samples.len(),
                maximum: POR_PROOF_MAX_SAMPLES_V1,
            });
        }
        for sample in &self.samples {
            sample.validate()?;
        }
        if self.auth_path.is_empty() {
            return Err(PorProofValidationError::MissingAuthPath);
        }
        if self.auth_path.len() > POR_PROOF_MAX_AUTH_PATH_NODES_V1 {
            return Err(PorProofValidationError::AuthPathTooDeep {
                found: self.auth_path.len(),
                maximum: POR_PROOF_MAX_AUTH_PATH_NODES_V1,
            });
        }
        if self.submitted_at == 0 {
            return Err(PorProofValidationError::InvalidSubmittedAt);
        }
        if self.signature.algorithm != SignatureAlgorithm::Ed25519 {
            return Err(PorProofValidationError::InvalidSignature);
        }
        if self.signature.public_key.len() != PUBLIC_KEY_LENGTH {
            return Err(PorProofValidationError::InvalidPublicKeyLength {
                found: self.signature.public_key.len(),
                expected: PUBLIC_KEY_LENGTH,
            });
        }
        if self.signature.signature.len() != SIGNATURE_LENGTH {
            return Err(PorProofValidationError::InvalidSignatureLength {
                found: self.signature.signature.len(),
                expected: SIGNATURE_LENGTH,
            });
        }
        if crate::inert_bytes(&self.signature.public_key)
            || crate::inert_bytes(&self.signature.signature)
        {
            return Err(PorProofValidationError::InvalidSignature);
        }
        Ok(())
    }
    /// Returns canonical, domain-separated bytes signed by the provider.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the canonical payload cannot be
    /// serialized.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        preflight_por_proof_len(self, POR_PROOF_MAX_CANONICAL_BYTES_V1)
            .map_err(|error| norito::core::Error::Message(error.to_string()))?;
        let payload = PorProofSigningPayloadViewV1::from(self);
        let exact = payload.encoded_len_exact().ok_or_else(|| {
            norito::core::Error::Message(
                "PoR proof signing payload has no exact canonical length".to_owned(),
            )
        })?;
        if exact > POR_PROOF_MAX_CANONICAL_BYTES_V1 {
            return Err(norito::core::Error::Message(format!(
                "PoR proof signing payload has {exact} bytes; maximum is {POR_PROOF_MAX_CANONICAL_BYTES_V1}"
            )));
        }
        norito::to_bytes(&payload)
    }
    /// Cryptographically verifies the provider signature.
    ///
    /// Structural validation alone intentionally does not imply authenticity;
    /// callers crossing a trust boundary must invoke this method as well.
    ///
    /// # Errors
    ///
    /// Returns [`PorSignatureVerificationError`] for unsupported algorithms,
    /// malformed key/signature material, canonical encoding failures, or an
    /// invalid signature.
    pub fn verify_signature(&self) -> Result<(), PorSignatureVerificationError> {
        let payload = self.signature_payload_bytes().map_err(|error| {
            PorSignatureVerificationError::PayloadEncoding {
                reason: error.to_string(),
            }
        })?;
        verify_ed25519_signature(&self.signature, &payload)
    }
    /// Verifies the proof signature and binds it to the admitted provider key.
    ///
    /// # Errors
    ///
    /// Returns [`PorSignatureVerificationError`] when the signature is invalid
    /// or its embedded key is not the trusted key supplied by provider
    /// admission.
    pub fn verify_signature_for_provider(
        &self,
        admitted_provider_key: &[u8],
    ) -> Result<(), PorSignatureVerificationError> {
        self.verify_signature()?;
        if self.signature.public_key != admitted_provider_key {
            return Err(PorSignatureVerificationError::ProviderSignerMismatch);
        }
        Ok(())
    }
    /// Computes the canonical digest of the proof payload (without signature).
    #[must_use]
    pub fn proof_digest(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(POR_PROOF_SIGNATURE_DOMAIN_V1.as_bytes());
        hasher.update(&[self.version]);
        hasher.update(&self.challenge_id);
        hasher.update(&self.manifest_digest);
        hasher.update(&self.provider_id);
        let sample_count = u64::try_from(self.samples.len()).unwrap_or(u64::MAX);
        hasher.update(&sample_count.to_le_bytes());
        for sample in &self.samples {
            hasher.update(&sample.sample_index.to_be_bytes());
            hasher.update(&sample.chunk_offset.to_be_bytes());
            hasher.update(&sample.chunk_size.to_be_bytes());
            hasher.update(&sample.chunk_digest);
            hasher.update(&sample.leaf_digest);
        }
        let auth_path_count = u64::try_from(self.auth_path.len()).unwrap_or(u64::MAX);
        hasher.update(&auth_path_count.to_le_bytes());
        for node in &self.auth_path {
            hasher.update(node);
        }
        hasher.update(&self.submitted_at.to_le_bytes());
        hasher.finalize().into()
    }
}
/// Validation errors for [`PorProofV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorProofValidationError {
    #[error("PoR proof does not expose an exact canonical encoded length")]
    CanonicalLengthUnavailable,
    #[error("PoR proof has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    #[error("unsupported proof version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("proof must contain at least one sample")]
    MissingSamples,
    #[error("proof has {found} samples; maximum is {maximum}")]
    TooManySamples { found: usize, maximum: usize },
    #[error("authentication path must not be empty")]
    MissingAuthPath,
    #[error("authentication path has {found} nodes; maximum is {maximum}")]
    AuthPathTooDeep { found: usize, maximum: usize },
    #[error("sample {sample_index} has invalid chunk size")]
    InvalidChunkSize { sample_index: u64 },
    #[error("sample {sample_index} has invalid chunk digest")]
    InvalidChunkDigest { sample_index: u64 },
    #[error("sample {sample_index} has invalid leaf digest")]
    InvalidLeafDigest { sample_index: u64 },
    #[error("signature must include algorithm-specific public key and signature bytes")]
    InvalidSignature,
    #[error("Ed25519 public key has {found} bytes; expected {expected}")]
    InvalidPublicKeyLength { found: usize, expected: usize },
    #[error("Ed25519 signature has {found} bytes; expected {expected}")]
    InvalidSignatureLength { found: usize, expected: usize },
    #[error("proof submitted_at must be non-zero")]
    InvalidSubmittedAt,
}
fn preflight_por_proof_len(
    proof: &PorProofV1,
    maximum: usize,
) -> Result<usize, PorProofValidationError> {
    let found = proof
        .encoded_len_exact()
        .ok_or(PorProofValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(PorProofValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Decode and validate one bounded canonical V1 PoR proof.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid proof bytes.
pub fn decode_por_proof_v1(bytes: &[u8]) -> Result<PorProofV1, norito::core::Error> {
    let proof: PorProofV1 = decode_bounded_canonical_por_payload(
        "PoR proof",
        bytes,
        POR_PROOF_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            POR_PROOF_MAX_SAMPLES_V1,
            POR_PROOF_MAX_CANONICAL_BYTES_V1,
            16_384,
            POR_PROOF_MAX_CANONICAL_BYTES_V1 * 4,
            64,
        ),
    )?;
    proof
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(proof)
}
/// Outcome recorded after challenge verification.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum AuditOutcomeV1 {
    /// Proof verified successfully.
    Success = 1,
    /// Proof failed verification.
    Failed = 2,
    /// Proof failed initially but recovered after repair.
    Repaired = 3,
}
/// Audit verdict logged into the governance DAG.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct AuditVerdictV1 {
    /// Schema version (`AUDIT_VERDICT_VERSION_V1`).
    pub version: u8,
    /// Manifest digest (BLAKE3-256).
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Referenced challenge identifier.
    pub challenge_id: [u8; 32],
    /// Digest of the proof payload (optional when proof missing).
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
    /// Audit outcome.
    pub outcome: AuditOutcomeV1,
    /// Optional failure reason when `outcome` is not success.
    #[norito(default)]
    pub failure_reason: Option<String>,
    /// Unix timestamp (seconds) when the verdict was issued.
    pub decided_at: u64,
    /// Auditor signatures over the canonical domain-separated unsigned verdict payload.
    pub auditor_signatures: Vec<AdvertSignature>,
    /// Optional metadata entries for downstream systems.
    #[norito(default)]
    pub metadata: Vec<CapacityMetadataEntry>,
}
#[derive(Debug, Clone, NoritoSerialize)]
struct AuditVerdictSigningPayloadV1 {
    domain: String,
    version: u8,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    challenge_id: [u8; 32],
    proof_digest: Option<[u8; 32]>,
    outcome: AuditOutcomeV1,
    failure_reason: Option<String>,
    decided_at: u64,
    metadata: Vec<CapacityMetadataEntry>,
}
#[derive(NoritoSerialize)]
struct AuditVerdictSigningPayloadViewWireV1<'a> {
    domain: borrowed_norito::String<'a>,
    version: u8,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    challenge_id: [u8; 32],
    proof_digest: Option<[u8; 32]>,
    outcome: AuditOutcomeV1,
    failure_reason: borrowed_norito::Option<'a, String>,
    decided_at: u64,
    metadata: borrowed_norito::Vec<'a, CapacityMetadataEntry>,
}
struct AuditVerdictSigningPayloadViewV1<'a>(AuditVerdictSigningPayloadViewWireV1<'a>);
impl<'a> From<&'a AuditVerdictV1> for AuditVerdictSigningPayloadViewV1<'a> {
    fn from(verdict: &'a AuditVerdictV1) -> Self {
        Self(AuditVerdictSigningPayloadViewWireV1 {
            domain: borrowed_norito::String(POR_VERDICT_SIGNATURE_DOMAIN_V1),
            version: verdict.version,
            manifest_digest: verdict.manifest_digest,
            provider_id: verdict.provider_id,
            challenge_id: verdict.challenge_id,
            proof_digest: verdict.proof_digest,
            outcome: verdict.outcome,
            failure_reason: borrowed_norito::Option(&verdict.failure_reason),
            decided_at: verdict.decided_at,
            metadata: borrowed_norito::Vec(&verdict.metadata),
        })
    }
}
impl norito::core::NoritoSerialize for AuditVerdictSigningPayloadViewV1<'_> {
    fn schema_hash() -> [u8; 16] {
        AuditVerdictSigningPayloadV1::schema_hash()
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.0.serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
#[cfg(test)]
impl From<&AuditVerdictV1> for AuditVerdictSigningPayloadV1 {
    fn from(verdict: &AuditVerdictV1) -> Self {
        Self {
            domain: POR_VERDICT_SIGNATURE_DOMAIN_V1.to_owned(),
            version: verdict.version,
            manifest_digest: verdict.manifest_digest,
            provider_id: verdict.provider_id,
            challenge_id: verdict.challenge_id,
            proof_digest: verdict.proof_digest,
            outcome: verdict.outcome,
            failure_reason: verdict.failure_reason.clone(),
            decided_at: verdict.decided_at,
            metadata: verdict.metadata.clone(),
        }
    }
}
impl AuditVerdictV1 {
    /// Validates the verdict payload.
    pub fn validate(&self) -> Result<(), AuditVerdictValidationError> {
        preflight_audit_verdict_len(self, AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1)?;
        if self.version != AUDIT_VERDICT_VERSION_V1 {
            return Err(AuditVerdictValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(AuditVerdictValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(AuditVerdictValidationError::InvalidProviderId);
        }
        if self.challenge_id.iter().all(|&byte| byte == 0) {
            return Err(AuditVerdictValidationError::InvalidChallengeId);
        }
        if self.decided_at == 0 {
            return Err(AuditVerdictValidationError::InvalidDecidedAt);
        }
        if let Some(reason) = self.failure_reason.as_ref() {
            if reason.len() > AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1 {
                return Err(AuditVerdictValidationError::FailureReasonTooLong {
                    found: reason.len(),
                    maximum: AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1,
                });
            }
            if reason.trim().is_empty()
                || reason.trim() != reason
                || reason.chars().any(char::is_control)
            {
                return Err(AuditVerdictValidationError::InvalidFailureReason);
            }
        }
        match self.outcome {
            AuditOutcomeV1::Success => {
                if self.failure_reason.is_some() {
                    return Err(AuditVerdictValidationError::UnexpectedFailureReason);
                }
            }
            AuditOutcomeV1::Failed | AuditOutcomeV1::Repaired => {
                if self
                    .failure_reason
                    .as_ref()
                    .is_none_or(|reason| reason.trim().is_empty())
                {
                    return Err(AuditVerdictValidationError::MissingFailureReason);
                }
            }
        }
        if self.auditor_signatures.is_empty() {
            return Err(AuditVerdictValidationError::MissingSignatures);
        }
        if self.auditor_signatures.len() > AUDIT_VERDICT_MAX_SIGNATURES_V1 {
            return Err(AuditVerdictValidationError::TooManySignatures {
                found: self.auditor_signatures.len(),
                maximum: AUDIT_VERDICT_MAX_SIGNATURES_V1,
            });
        }
        let mut signer_keys = BTreeSet::new();
        for (index, signature) in self.auditor_signatures.iter().enumerate() {
            if signature.algorithm != SignatureAlgorithm::Ed25519 {
                return Err(AuditVerdictValidationError::InvalidSignature);
            }
            if signature.public_key.len() != PUBLIC_KEY_LENGTH {
                return Err(AuditVerdictValidationError::InvalidPublicKeyLength {
                    index,
                    found: signature.public_key.len(),
                    expected: PUBLIC_KEY_LENGTH,
                });
            }
            if signature.signature.len() != SIGNATURE_LENGTH {
                return Err(AuditVerdictValidationError::InvalidSignatureLength {
                    index,
                    found: signature.signature.len(),
                    expected: SIGNATURE_LENGTH,
                });
            }
            if crate::inert_bytes(&signature.public_key) || crate::inert_bytes(&signature.signature)
            {
                return Err(AuditVerdictValidationError::InvalidSignature);
            }
            if !signer_keys.insert(signature.public_key.as_slice()) {
                return Err(AuditVerdictValidationError::DuplicateAuditorSigner { index });
            }
        }
        if self.metadata.len() > AUDIT_VERDICT_MAX_METADATA_ENTRIES_V1 {
            return Err(AuditVerdictValidationError::TooManyMetadataEntries {
                found: self.metadata.len(),
                maximum: AUDIT_VERDICT_MAX_METADATA_ENTRIES_V1,
            });
        }
        let mut metadata_keys = BTreeSet::new();
        let mut metadata_bytes = 0_usize;
        for (index, entry) in self.metadata.iter().enumerate() {
            entry
                .validate()
                .map_err(|_| AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "capacity metadata validation failed",
                })?;
            if !metadata_keys.insert(entry.key.as_str()) {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata keys must be unique",
                });
            }
            metadata_bytes = metadata_bytes
                .checked_add(entry.key.len())
                .and_then(|bytes| bytes.checked_add(entry.value.len()))
                .ok_or(AuditVerdictValidationError::MetadataBytesOverflow)?;
            if metadata_bytes > AUDIT_VERDICT_MAX_METADATA_BYTES_V1 {
                return Err(AuditVerdictValidationError::MetadataTooLarge {
                    found: metadata_bytes,
                    maximum: AUDIT_VERDICT_MAX_METADATA_BYTES_V1,
                });
            }
        }
        Ok(())
    }
    /// Returns canonical, domain-separated bytes signed by every auditor.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the canonical payload cannot be
    /// serialized.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        preflight_audit_verdict_len(self, AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1)
            .map_err(|error| norito::core::Error::Message(error.to_string()))?;
        let payload = AuditVerdictSigningPayloadViewV1::from(self);
        let exact = payload.encoded_len_exact().ok_or_else(|| {
            norito::core::Error::Message(
                "audit verdict signing payload has no exact canonical length".to_owned(),
            )
        })?;
        if exact > AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1 {
            return Err(norito::core::Error::Message(format!(
                "audit verdict signing payload has {exact} bytes; maximum is {AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1}"
            )));
        }
        norito::to_bytes(&payload)
    }
    /// Cryptographically verifies every unique auditor signature.
    ///
    /// Duplicate signer keys are rejected so a single auditor cannot pad a
    /// future threshold calculation by repeating the same signature.
    ///
    /// # Errors
    ///
    /// Returns [`PorSignatureVerificationError`] when encoding fails, signature
    /// material is malformed, a signature is invalid, or a signer is repeated.
    pub fn verify_signatures(&self) -> Result<(), PorSignatureVerificationError> {
        let payload = self.signature_payload_bytes().map_err(|error| {
            PorSignatureVerificationError::PayloadEncoding {
                reason: error.to_string(),
            }
        })?;
        let mut signers = BTreeSet::new();
        for signature in &self.auditor_signatures {
            if !signers.insert(signature.public_key.as_slice()) {
                return Err(PorSignatureVerificationError::DuplicateSigner);
            }
            verify_ed25519_signature(signature, &payload)?;
        }
        Ok(())
    }
    /// Verifies that every verdict signer is trusted and that the configured
    /// auditor threshold is met.
    ///
    /// All embedded signatures must belong to the trusted set. This prevents
    /// an attacker from padding a threshold verdict with arbitrary self-signed
    /// keys even when one trusted signer is present.
    ///
    /// # Errors
    ///
    /// Returns [`PorSignatureVerificationError`] when the trust policy is
    /// empty or inconsistent, a signature is invalid, an untrusted signer is
    /// present, or fewer than `threshold` unique trusted auditors signed.
    pub fn verify_signatures_with_policy(
        &self,
        trusted_auditor_keys: &[Vec<u8>],
        threshold: usize,
    ) -> Result<(), PorSignatureVerificationError> {
        let trusted: BTreeSet<&[u8]> = trusted_auditor_keys.iter().map(Vec::as_slice).collect();
        if trusted.is_empty() {
            return Err(PorSignatureVerificationError::EmptyTrustedAuditorSet);
        }
        if threshold == 0 || threshold > trusted.len() {
            return Err(PorSignatureVerificationError::InvalidAuditorThreshold {
                threshold,
                trusted: trusted.len(),
            });
        }
        self.verify_signatures()?;
        for signature in &self.auditor_signatures {
            if !trusted.contains(signature.public_key.as_slice()) {
                return Err(PorSignatureVerificationError::UntrustedAuditorSigner);
            }
        }
        if self.auditor_signatures.len() < threshold {
            return Err(
                PorSignatureVerificationError::InsufficientTrustedAuditorSignatures {
                    actual: self.auditor_signatures.len(),
                    required: threshold,
                },
            );
        }
        Ok(())
    }
    /// Returns `true` when an auditor signature advertises `public_key`.
    #[must_use]
    pub fn has_signer(&self, public_key: &[u8]) -> bool {
        self.auditor_signatures
            .iter()
            .any(|signature| signature.public_key == public_key)
    }
}
/// Validation errors for [`AuditVerdictV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AuditVerdictValidationError {
    #[error("audit verdict does not expose an exact canonical encoded length")]
    CanonicalLengthUnavailable,
    #[error("audit verdict has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    #[error("unsupported verdict version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    #[error("verdict decided_at must be non-zero")]
    InvalidDecidedAt,
    #[error("failure reason required for non-success outcomes")]
    MissingFailureReason,
    #[error("failure reason must be canonical non-empty UTF-8 without control characters")]
    InvalidFailureReason,
    #[error("failure reason must be absent for success outcomes")]
    UnexpectedFailureReason,
    #[error("failure reason has {found} bytes; maximum is {maximum}")]
    FailureReasonTooLong { found: usize, maximum: usize },
    #[error("at least one auditor signature is required")]
    MissingSignatures,
    #[error("verdict has {found} auditor signatures; maximum is {maximum}")]
    TooManySignatures { found: usize, maximum: usize },
    #[error("auditor signature is missing key or signature bytes")]
    InvalidSignature,
    #[error("auditor signature #{index} public key has {found} bytes; expected {expected}")]
    InvalidPublicKeyLength {
        index: usize,
        found: usize,
        expected: usize,
    },
    #[error("auditor signature #{index} has {found} bytes; expected {expected}")]
    InvalidSignatureLength {
        index: usize,
        found: usize,
        expected: usize,
    },
    #[error("auditor signature #{index} repeats an earlier signer")]
    DuplicateAuditorSigner { index: usize },
    #[error("verdict has {found} metadata rows; maximum is {maximum}")]
    TooManyMetadataEntries { found: usize, maximum: usize },
    #[error("metadata entry {index} invalid: {reason}")]
    InvalidMetadata { index: usize, reason: &'static str },
    #[error("audit verdict metadata byte count overflow")]
    MetadataBytesOverflow,
    #[error("audit verdict metadata has {found} bytes; maximum is {maximum}")]
    MetadataTooLarge { found: usize, maximum: usize },
}
fn preflight_audit_verdict_len(
    verdict: &AuditVerdictV1,
    maximum: usize,
) -> Result<usize, AuditVerdictValidationError> {
    let found = verdict
        .encoded_len_exact()
        .ok_or(AuditVerdictValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(AuditVerdictValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Decode and validate one bounded canonical V1 PoR audit verdict.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid verdict bytes.
pub fn decode_audit_verdict_v1(bytes: &[u8]) -> Result<AuditVerdictV1, norito::core::Error> {
    let verdict: AuditVerdictV1 = decode_bounded_canonical_por_payload(
        "PoR audit verdict",
        bytes,
        AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            crate::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES,
            crate::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES,
            16_384,
            AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1 * 4,
            64,
        ),
    )?;
    verdict
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(verdict)
}
/// Errors raised while verifying PoR provider or auditor signatures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PorSignatureVerificationError {
    /// Only Ed25519 is supported for first-release PoR artefacts.
    #[error("unsupported PoR signature algorithm {0:?}")]
    UnsupportedAlgorithm(SignatureAlgorithm),
    /// Public key material has the wrong size.
    #[error("invalid Ed25519 public key length {length}; expected 32")]
    InvalidPublicKeyLength {
        /// Actual public key length.
        length: usize,
    },
    /// Signature material has the wrong size.
    #[error("invalid Ed25519 signature length {length}; expected 64")]
    InvalidSignatureLength {
        /// Actual signature length.
        length: usize,
    },
    /// Public key material is malformed, non-canonical, or weak.
    #[error("invalid Ed25519 public key: {reason}")]
    InvalidPublicKey {
        /// Validation failure detail.
        reason: String,
    },
    /// Signature material is malformed, non-canonical, or weak.
    #[error("invalid Ed25519 signature: {reason}")]
    InvalidSignature {
        /// Validation failure detail.
        reason: String,
    },
    /// Canonical signing payload encoding failed.
    #[error("failed to encode PoR signature payload: {reason}")]
    PayloadEncoding {
        /// Encoding failure detail.
        reason: String,
    },
    /// Cryptographic signature verification failed.
    #[error("PoR signature verification failed: {reason}")]
    Verification {
        /// Verification failure detail.
        reason: String,
    },
    /// An auditor key appeared more than once.
    #[error("duplicate PoR auditor signer")]
    DuplicateSigner,
    /// The proof signer is not the key admitted for the provider.
    #[error("PoR proof signer does not match the admitted provider key")]
    ProviderSignerMismatch,
    /// No trusted auditor keys were configured.
    #[error("trusted PoR auditor set must not be empty")]
    EmptyTrustedAuditorSet,
    /// The configured threshold cannot be satisfied by the trusted set.
    #[error("invalid PoR auditor threshold {threshold} for {trusted} trusted auditors")]
    InvalidAuditorThreshold {
        /// Requested number of signatures.
        threshold: usize,
        /// Number of unique trusted keys.
        trusted: usize,
    },
    /// A cryptographically valid signature came from an untrusted key.
    #[error("PoR verdict contains an untrusted auditor signer")]
    UntrustedAuditorSigner,
    /// Fewer trusted auditors signed than policy requires.
    #[error("PoR verdict has {actual} trusted signatures; {required} required")]
    InsufficientTrustedAuditorSignatures {
        /// Unique trusted signatures present.
        actual: usize,
        /// Required unique signatures.
        required: usize,
    },
}
fn verify_ed25519_signature(
    signature: &AdvertSignature,
    payload: &[u8],
) -> Result<(), PorSignatureVerificationError> {
    if signature.algorithm != SignatureAlgorithm::Ed25519 {
        return Err(PorSignatureVerificationError::UnsupportedAlgorithm(
            signature.algorithm,
        ));
    }
    if signature.public_key.len() != PUBLIC_KEY_LENGTH {
        return Err(PorSignatureVerificationError::InvalidPublicKeyLength {
            length: signature.public_key.len(),
        });
    }
    if signature.signature.len() != SIGNATURE_LENGTH {
        return Err(PorSignatureVerificationError::InvalidSignatureLength {
            length: signature.signature.len(),
        });
    }
    let mut public_key = [0_u8; PUBLIC_KEY_LENGTH];
    public_key.copy_from_slice(&signature.public_key);
    let verifying_key = crate::checked_ed25519_verifying_key_from_bytes(&public_key)
        .map_err(|reason| PorSignatureVerificationError::InvalidPublicKey { reason })?;
    let mut signature_bytes = [0_u8; SIGNATURE_LENGTH];
    signature_bytes.copy_from_slice(&signature.signature);
    let signature = crate::checked_ed25519_signature_from_bytes(&signature_bytes)
        .map_err(|reason| PorSignatureVerificationError::InvalidSignature { reason })?;
    verifying_key
        .verify_strict(payload, &signature)
        .map_err(|error| PorSignatureVerificationError::Verification {
            reason: error.to_string(),
        })
}
/// Canonical payload carried by opaque PoR status and export cursors.
///
/// The cursor binds one exact projection generation, normalized selection,
/// and complete `(epoch, issued_at, challenge_id)` boundary. Both servers and
/// clients use this codec so accepting a syntactically valid but structurally
/// different base64 payload is impossible.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct PorStatusCursorV1 {
    /// Cursor schema version.
    pub version: u8,
    /// Exact authoritative projection generation.
    pub snapshot_generation: u64,
    /// Domain-separated digest of the normalized query selection.
    pub selection_digest: [u8; 32],
    /// Epoch component of the exact consumed boundary.
    pub last_epoch_id: u64,
    /// Issuance-time component of the exact consumed boundary.
    pub last_issued_at: u64,
    /// Challenge component of the exact consumed boundary.
    pub last_challenge_id: [u8; 32],
}
impl PorStatusCursorV1 {
    /// Validate the fixed first-release cursor shape.
    pub fn validate(self) -> Result<(), PorStatusCursorValidationError> {
        if self.version != POR_STATUS_CURSOR_VERSION_V1 {
            return Err(PorStatusCursorValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.snapshot_generation == 0 {
            return Err(PorStatusCursorValidationError::InvalidGeneration);
        }
        if self.selection_digest == [0; 32] {
            return Err(PorStatusCursorValidationError::InvalidSelectionDigest);
        }
        if self.last_epoch_id == 0 {
            return Err(PorStatusCursorValidationError::InvalidEpoch);
        }
        if self.last_issued_at == 0 {
            return Err(PorStatusCursorValidationError::InvalidIssuedAt);
        }
        if self.last_challenge_id == [0; 32] {
            return Err(PorStatusCursorValidationError::InvalidChallengeId);
        }
        Ok(())
    }
    /// Encode this validated cursor as unique unpadded base64url.
    pub fn encode_opaque(self) -> Result<String, PorStatusCursorCodecError> {
        self.validate()?;
        let bytes = norito::to_bytes(&self)
            .map_err(|error| PorStatusCursorCodecError::Canonical(error.to_string()))?;
        if bytes.len() > POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1 {
            return Err(PorStatusCursorCodecError::CanonicalTooLarge {
                found: bytes.len(),
                maximum: POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1,
            });
        }
        let opaque = URL_SAFE_NO_PAD.encode(bytes);
        if opaque.len() > POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1 {
            return Err(PorStatusCursorCodecError::EncodedTooLarge {
                found: opaque.len(),
                maximum: POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1,
            });
        }
        Ok(opaque)
    }
    /// Decode one bounded, canonical unpadded-base64url cursor.
    pub fn decode_opaque(cursor: &str) -> Result<Self, PorStatusCursorCodecError> {
        if cursor.is_empty() || cursor.len() > POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1 {
            return Err(PorStatusCursorCodecError::EncodedLength {
                found: cursor.len(),
                maximum: POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1,
            });
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(cursor.as_bytes())
            .map_err(|_| PorStatusCursorCodecError::Base64)?;
        if URL_SAFE_NO_PAD.encode(&bytes) != cursor {
            return Err(PorStatusCursorCodecError::Base64);
        }
        let value: Self = decode_bounded_canonical_por_payload(
            "PoR status cursor",
            &bytes,
            POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1,
            norito::DecodeLimits::new(
                1,
                POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1,
                8,
                POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1 * 2,
                8,
            ),
        )
        .map_err(|error| PorStatusCursorCodecError::Canonical(error.to_string()))?;
        value.validate()?;
        Ok(value)
    }
}
/// Validation failure for a decoded [`PorStatusCursorV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorStatusCursorValidationError {
    /// The cursor version is not the first-release version.
    #[error("unsupported PoR status cursor version {found}")]
    UnsupportedVersion { found: u8 },
    /// The authoritative generation must be non-zero.
    #[error("PoR status cursor generation must be non-zero")]
    InvalidGeneration,
    /// The normalized selection digest must be non-zero.
    #[error("PoR status cursor selection digest must be non-zero")]
    InvalidSelectionDigest,
    /// The boundary epoch must be non-zero.
    #[error("PoR status cursor epoch must be non-zero")]
    InvalidEpoch,
    /// The boundary issuance time must be non-zero.
    #[error("PoR status cursor issued_at must be non-zero")]
    InvalidIssuedAt,
    /// The boundary challenge identity must be non-zero.
    #[error("PoR status cursor challenge id must be non-zero")]
    InvalidChallengeId,
}
/// Bounded canonical cursor encoding or decoding failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PorStatusCursorCodecError {
    /// The opaque cursor length is outside the admitted bound.
    #[error("opaque PoR status cursor has {found} bytes; expected 1..={maximum}")]
    EncodedLength {
        /// Opaque bytes supplied by the caller.
        found: usize,
        /// First-release opaque-byte ceiling.
        maximum: usize,
    },
    /// Encoding unexpectedly exceeded the advertised opaque bound.
    #[error("encoded PoR status cursor has {found} bytes; maximum is {maximum}")]
    EncodedTooLarge {
        /// Opaque bytes produced by canonical encoding.
        found: usize,
        /// First-release opaque-byte ceiling.
        maximum: usize,
    },
    /// The opaque representation is not unique unpadded base64url.
    #[error("PoR status cursor is not canonical unpadded base64url")]
    Base64,
    /// The decoded Norito payload is malformed or non-canonical.
    #[error("invalid canonical PoR status cursor payload: {0}")]
    Canonical(String),
    /// The canonical cursor payload unexpectedly exceeded its fixed bound.
    #[error("canonical PoR status cursor has {found} bytes; maximum is {maximum}")]
    CanonicalTooLarge {
        /// Canonical bytes produced or decoded.
        found: usize,
        /// First-release canonical-byte ceiling.
        maximum: usize,
    },
    /// The decoded fixed fields violate the first-release cursor contract.
    #[error(transparent)]
    Validation(#[from] PorStatusCursorValidationError),
}
/// Lifecycle states emitted by the PoR coordinator.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[norito(tag = "outcome")]
#[repr(u8)]
pub enum PorChallengeOutcome {
    /// Challenge is awaiting its first provider proof.
    #[norito(rename = "awaiting_proof")]
    AwaitingProof = 1,
    /// An authenticated provider proof was submitted and awaits a verdict.
    #[norito(rename = "proof_submitted")]
    ProofSubmitted = 2,
    /// Proof verified successfully.
    #[norito(rename = "verified")]
    Verified = 3,
    /// Proof failed and awaits remediation.
    #[norito(rename = "failed")]
    Failed = 4,
    /// Proof recovered after repair.
    #[norito(rename = "repaired")]
    Repaired = 5,
}
impl PorChallengeOutcome {
    /// Human-readable label for reporting.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingProof => "awaiting_proof",
            Self::ProofSubmitted => "proof_submitted",
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::Repaired => "repaired",
        }
    }
    /// Parses a label into an outcome.
    pub fn parse(label: &str) -> Result<Self, PorChallengeOutcomeParseError> {
        match label.trim().to_ascii_lowercase().as_str() {
            "awaiting_proof" => Ok(Self::AwaitingProof),
            "proof_submitted" => Ok(Self::ProofSubmitted),
            "verified" => Ok(Self::Verified),
            "failed" => Ok(Self::Failed),
            "repaired" => Ok(Self::Repaired),
            other => Err(PorChallengeOutcomeParseError {
                label: other.to_string(),
            }),
        }
    }
}
impl From<PorChallengeOutcome> for u8 {
    fn from(value: PorChallengeOutcome) -> Self {
        value as u8
    }
}
impl TryFrom<u8> for PorChallengeOutcome {
    type Error = PorChallengeOutcomeParseError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::AwaitingProof),
            2 => Ok(Self::ProofSubmitted),
            3 => Ok(Self::Verified),
            4 => Ok(Self::Failed),
            5 => Ok(Self::Repaired),
            other => Err(PorChallengeOutcomeParseError {
                label: other.to_string(),
            }),
        }
    }
}
/// Error raised when converting outcome labels.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unsupported PoR challenge outcome: {label}")]
pub struct PorChallengeOutcomeParseError {
    label: String,
}
impl norito::json::JsonSerialize for PorChallengeOutcome {
    fn json_serialize(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.as_str(), out);
    }
}
/// Status snapshot returned by the PoR coordinator.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorChallengeStatusV1 {
    /// Schema version (`POR_CHALLENGE_STATUS_VERSION_V1`).
    pub version: u8,
    /// Challenge identifier (BLAKE3-256 digest).
    pub challenge_id: [u8; 32],
    /// Manifest digest.
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Epoch identifier.
    pub epoch_id: u64,
    /// drand round used for the challenge.
    pub drand_round: u64,
    /// Outcome recorded for the challenge.
    pub status: PorChallengeOutcome,
    /// Number of samples requested.
    pub sample_count: u16,
    /// Whether the coordinator forced the challenge (missing VRF).
    #[norito(default)]
    pub forced: bool,
    /// Unix timestamp when the challenge was issued.
    pub issued_at: u64,
    /// Unix timestamp when the proof was received.
    #[norito(default)]
    pub responded_at: Option<u64>,
    /// Optional proof digest.
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
    /// Chain-authoritative repair task identifier for a failed challenge.
    ///
    /// This field is intentionally not defaulted: the canonical V1 wire shape
    /// rejects pre-release snapshots that omitted the field or encoded the
    /// former process-local 16-byte identifier.
    pub repair_task_id: Option<[u8; 32]>,
    /// Optional failure reason when the challenge was unsuccessful.
    #[norito(default)]
    pub failure_reason: Option<String>,
    /// Optional verifier latency in milliseconds.
    #[norito(default)]
    pub verifier_latency_ms: Option<u32>,
}
impl PorChallengeStatusV1 {
    /// Validates the status snapshot.
    pub fn validate(&self) -> Result<(), PorChallengeStatusValidationError> {
        preflight_por_challenge_status_len(self, POR_CHALLENGE_STATUS_MAX_CANONICAL_BYTES_V1)?;
        if self.version != POR_CHALLENGE_STATUS_VERSION_V1 {
            return Err(PorChallengeStatusValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.challenge_id.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeStatusValidationError::InvalidChallengeId);
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeStatusValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(PorChallengeStatusValidationError::InvalidProviderId);
        }
        if self.epoch_id == 0 {
            return Err(PorChallengeStatusValidationError::InvalidEpoch);
        }
        if self.drand_round == 0 {
            return Err(PorChallengeStatusValidationError::InvalidDrandRound);
        }
        if self.sample_count == 0 {
            return Err(PorChallengeStatusValidationError::InvalidSampleCount);
        }
        if usize::from(self.sample_count) > POR_CHALLENGE_MAX_SAMPLES_V1 {
            return Err(PorChallengeStatusValidationError::TooManySamples {
                found: self.sample_count,
                maximum: POR_CHALLENGE_MAX_SAMPLES_V1,
            });
        }
        if self.issued_at == 0 {
            return Err(PorChallengeStatusValidationError::InvalidIssuedAt);
        }
        if let Some(responded_at) = self.responded_at
            && responded_at < self.issued_at
        {
            return Err(PorChallengeStatusValidationError::InvalidResponseTimestamp);
        }
        match (self.responded_at, self.proof_digest) {
            (Some(_), Some(digest)) => {
                if digest == [0; 32] {
                    return Err(PorChallengeStatusValidationError::InvalidProofDigest);
                }
            }
            (None, None) => {}
            _ => return Err(PorChallengeStatusValidationError::InconsistentProofMaterial),
        }
        if self.verifier_latency_ms.is_some() && self.proof_digest.is_none() {
            return Err(PorChallengeStatusValidationError::UnexpectedVerifierLatency);
        }
        if self.failure_reason.as_ref().is_some_and(|reason| {
            reason.trim().is_empty()
                || reason.trim() != reason
                || reason.len() > POR_CHALLENGE_STATUS_FAILURE_REASON_MAX_BYTES_V1
                || reason.chars().any(char::is_control)
        }) {
            return Err(PorChallengeStatusValidationError::InvalidFailureReason);
        }
        match self.status {
            PorChallengeOutcome::AwaitingProof => {
                if self.responded_at.is_some() {
                    return Err(PorChallengeStatusValidationError::UnexpectedProofMaterial);
                }
                if self.failure_reason.is_some()
                    || self.repair_task_id.is_some()
                    || self.verifier_latency_ms.is_some()
                {
                    return Err(PorChallengeStatusValidationError::UnexpectedOutcomeMaterial);
                }
            }
            PorChallengeOutcome::ProofSubmitted => {
                if self.proof_digest.is_none() {
                    return Err(PorChallengeStatusValidationError::MissingProofMaterial);
                }
                if self.failure_reason.is_some()
                    || self.repair_task_id.is_some()
                    || self.verifier_latency_ms.is_some()
                {
                    return Err(PorChallengeStatusValidationError::UnexpectedOutcomeMaterial);
                }
            }
            PorChallengeOutcome::Verified => {
                if self.proof_digest.is_none() {
                    return Err(PorChallengeStatusValidationError::MissingProofMaterial);
                }
                if self.failure_reason.is_some() {
                    return Err(PorChallengeStatusValidationError::UnexpectedFailureReason);
                }
                if self.repair_task_id.is_some() {
                    return Err(PorChallengeStatusValidationError::UnexpectedRepairTaskId);
                }
            }
            PorChallengeOutcome::Failed => {
                if self.failure_reason.is_none() {
                    return Err(PorChallengeStatusValidationError::MissingFailureReason);
                }
                match self.repair_task_id {
                    None => return Err(PorChallengeStatusValidationError::MissingRepairTaskId),
                    Some(task_id) if task_id == [0; 32] => {
                        return Err(PorChallengeStatusValidationError::InvalidRepairTaskId);
                    }
                    Some(_) => {}
                }
            }
            PorChallengeOutcome::Repaired => {
                if self.proof_digest.is_none() {
                    return Err(PorChallengeStatusValidationError::MissingProofMaterial);
                }
                if self.failure_reason.is_none() {
                    return Err(PorChallengeStatusValidationError::MissingFailureReason);
                }
                if self.repair_task_id.is_some() {
                    return Err(PorChallengeStatusValidationError::UnexpectedRepairTaskId);
                }
            }
        }
        Ok(())
    }
}
/// Validation errors for [`PorChallengeStatusV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorChallengeStatusValidationError {
    #[error("PoR challenge status does not expose an exact canonical encoded length")]
    CanonicalLengthUnavailable,
    #[error("PoR challenge status has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    #[error("unsupported status version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("epoch id must be non-zero")]
    InvalidEpoch,
    #[error("drand round must be non-zero")]
    InvalidDrandRound,
    #[error("sample count must be greater than zero")]
    InvalidSampleCount,
    #[error("status declares {found} samples; maximum is {maximum}")]
    TooManySamples { found: u16, maximum: usize },
    #[error("issued_at must be non-zero")]
    InvalidIssuedAt,
    #[error("responded_at and proof_digest must be present or absent together")]
    InconsistentProofMaterial,
    #[error("terminal success/recovery status requires proof response material")]
    MissingProofMaterial,
    #[error("proof digest must be non-zero")]
    InvalidProofDigest,
    #[error("awaiting-proof status must not claim proof response material")]
    UnexpectedProofMaterial,
    #[error("verifier latency requires proof response material")]
    UnexpectedVerifierLatency,
    #[error("status contains material not admitted for its outcome")]
    UnexpectedOutcomeMaterial,
    #[error("failure reason must be provided for failed/repaired outcomes")]
    MissingFailureReason,
    #[error("failure reason must be absent for non-failure outcomes")]
    UnexpectedFailureReason,
    #[error("failure reason must not be empty")]
    InvalidFailureReason,
    #[error("failed challenge must reference a chain-authoritative repair task")]
    MissingRepairTaskId,
    #[error("repair task id must be non-zero")]
    InvalidRepairTaskId,
    #[error("only failed challenges may reference a repair task")]
    UnexpectedRepairTaskId,
    #[error("responded_at must not precede issued_at")]
    InvalidResponseTimestamp,
}
fn preflight_por_challenge_status_len(
    status: &PorChallengeStatusV1,
    maximum: usize,
) -> Result<usize, PorChallengeStatusValidationError> {
    let found = status
        .encoded_len_exact()
        .ok_or(PorChallengeStatusValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(PorChallengeStatusValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Decode and validate one bounded canonical V1 PoR challenge status.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid status bytes.
pub fn decode_por_challenge_status_v1(
    bytes: &[u8],
) -> Result<PorChallengeStatusV1, norito::core::Error> {
    let status: PorChallengeStatusV1 = decode_bounded_canonical_por_payload(
        "PoR challenge status",
        bytes,
        POR_CHALLENGE_STATUS_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            8,
            POR_CHALLENGE_STATUS_FAILURE_REASON_MAX_BYTES_V1,
            256,
            POR_CHALLENGE_STATUS_MAX_CANONICAL_BYTES_V1 * 4,
            16,
        ),
    )?;
    status
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(status)
}
/// Decode and validate one bounded canonical page of V1 PoR challenge statuses.
///
/// `maximum_records` may further restrict, but never raise, the protocol page
/// ceiling.
///
/// # Errors
///
/// Returns a Norito error for an invalid requested bound or for oversized,
/// noncanonical, malformed, or structurally invalid status bytes.
pub fn decode_por_challenge_status_page_v1(
    bytes: &[u8],
    maximum_records: usize,
) -> Result<Vec<PorChallengeStatusV1>, norito::core::Error> {
    if maximum_records > POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 {
        return Err(norito::core::Error::Message(format!(
            "PoR challenge status page requested {maximum_records} records; protocol maximum is {POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1}"
        )));
    }
    let statuses: Vec<PorChallengeStatusV1> = decode_bounded_canonical_por_payload(
        "PoR challenge status page",
        bytes,
        POR_CHALLENGE_STATUS_PAGE_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
            POR_CHALLENGE_STATUS_FAILURE_REASON_MAX_BYTES_V1,
            POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 * 64,
            POR_CHALLENGE_STATUS_PAGE_MAX_CANONICAL_BYTES_V1 * 4,
            32,
        ),
    )?;
    if statuses.len() > maximum_records {
        return Err(norito::core::Error::Message(format!(
            "PoR challenge status page has {} records; requested maximum is {maximum_records}",
            statuses.len()
        )));
    }
    for (index, status) in statuses.iter().enumerate() {
        status.validate().map_err(|error| {
            norito::core::Error::Message(format!(
                "PoR challenge status page record #{index} is invalid: {error}"
            ))
        })?;
    }
    Ok(statuses)
}
/// ISO-8601 week identifier used by PoR reports.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct PorReportIsoWeek {
    /// Calendar year (ISO week date).
    pub year: u16,
    /// ISO week number (1-53).
    pub week: u8,
}
impl PorReportIsoWeek {
    /// Validates the ISO-8601 components.
    pub fn validate(&self) -> Result<(), PorReportIsoWeekValidationError> {
        if self.year < 2000 {
            return Err(PorReportIsoWeekValidationError::InvalidYear { year: self.year });
        }
        if !(1..=53).contains(&self.week) {
            return Err(PorReportIsoWeekValidationError::InvalidWeek { week: self.week });
        }
        Ok(())
    }
}
impl std::fmt::Display for PorReportIsoWeek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-W{:02}", self.year, self.week)
    }
}
/// Validation errors for [`PorReportIsoWeek`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorReportIsoWeekValidationError {
    #[error("invalid ISO year {year}; expected >= 2000")]
    InvalidYear { year: u16 },
    #[error("invalid ISO week {week}; expected 1-53")]
    InvalidWeek { week: u8 },
}
/// Aggregated provider summary used by weekly reports.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorProviderSummaryV1 {
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Number of manifests served by this provider.
    pub manifest_count: u32,
    /// Number of challenges issued.
    pub challenges: u32,
    /// Number of successful challenges.
    #[norito(default)]
    pub successes: u32,
    /// Number of failed challenges.
    #[norito(default)]
    pub failures: u32,
    /// Number of challenges issued without a provider VRF.
    ///
    /// This is an orthogonal scheduling property and may overlap with either
    /// successful or failed challenge outcomes.
    #[norito(default)]
    pub forced: u32,
    /// Success rate in basis points (`0..=10_000`).
    #[norito(default)]
    pub success_rate_bps: u16,
    /// ISO-8601 timestamp (seconds) when the first failure occurred.
    #[norito(default)]
    pub first_failure_at: Option<u64>,
    /// 95th percentile latency for successful proofs (milliseconds).
    #[norito(default)]
    pub last_success_latency_ms_p95: Option<u32>,
    /// Whether a repair task was dispatched.
    #[norito(default)]
    pub repair_dispatched: bool,
    /// Number of repairs currently outstanding.
    #[norito(default)]
    pub pending_repairs: u32,
    /// Optional ticket identifier tracking remediation.
    #[norito(default)]
    pub ticket_id: Option<String>,
}
impl PorProviderSummaryV1 {
    /// Validates the provider summary entry.
    pub fn validate(&self) -> Result<(), PorProviderSummaryValidationError> {
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(PorProviderSummaryValidationError::InvalidProviderId);
        }
        if self.manifest_count == 0 {
            return Err(PorProviderSummaryValidationError::InvalidManifestCount);
        }
        if self.challenges == 0 {
            return Err(PorProviderSummaryValidationError::InvalidChallengeCount);
        }
        if self.success_rate_bps > POR_SUCCESS_RATE_BPS_MAX {
            return Err(PorProviderSummaryValidationError::InvalidSuccessRateBps {
                rate: self.success_rate_bps,
            });
        }
        if self.successes > self.challenges {
            return Err(PorProviderSummaryValidationError::InconsistentCounts);
        }
        if self.failures > self.challenges {
            return Err(PorProviderSummaryValidationError::InconsistentCounts);
        }
        if u64::from(self.successes) + u64::from(self.failures) > u64::from(self.challenges)
            || self.forced > self.challenges
        {
            return Err(PorProviderSummaryValidationError::InconsistentCounts);
        }
        let scaled = u64::from(self.successes) * u64::from(POR_SUCCESS_RATE_BPS_MAX);
        let expected_success_rate_bps =
            u16::try_from(scaled / u64::from(self.challenges)).unwrap_or(POR_SUCCESS_RATE_BPS_MAX);
        if self.success_rate_bps != expected_success_rate_bps {
            return Err(
                PorProviderSummaryValidationError::InconsistentSuccessRateBps {
                    expected: expected_success_rate_bps,
                    actual: self.success_rate_bps,
                },
            );
        }
        if self.ticket_id.as_ref().is_some_and(|ticket| {
            ticket.trim().is_empty()
                || ticket.trim() != ticket
                || ticket.len() > POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1
                || ticket.chars().any(char::is_control)
        }) {
            return Err(PorProviderSummaryValidationError::InvalidTicketId);
        }
        Ok(())
    }
}
/// Validation errors for [`PorProviderSummaryV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorProviderSummaryValidationError {
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("provider summary must cover at least one manifest")]
    InvalidManifestCount,
    #[error("provider summary must cover at least one challenge")]
    InvalidChallengeCount,
    #[error("success rate must be within 0..=10,000 basis points, got {rate}")]
    InvalidSuccessRateBps { rate: u16 },
    #[error(
        "success rate is inconsistent with challenge counts: expected {expected} bps, got {actual} bps"
    )]
    InconsistentSuccessRateBps { expected: u16, actual: u16 },
    #[error("challenge counts are inconsistent")]
    InconsistentCounts,
    #[error(
        "ticket identifier must be canonical non-empty UTF-8 of at most {POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1} bytes"
    )]
    InvalidTicketId,
}
/// Slashing event recorded during the reporting period.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorSlashingEventV1 {
    /// Provider identifier that was penalised.
    pub provider_id: [u8; 32],
    /// Manifest digest associated with the penalty.
    pub manifest_digest: [u8; 32],
    /// Penalty amount denominated in XOR micro-units.
    pub penalty_xor: XorQuantity,
    /// Governance verdict CID anchoring the slashing decision.
    pub verdict_cid: String,
    /// Timestamp when the decision was finalised (seconds since Unix epoch).
    pub decided_at: u64,
}
impl PorSlashingEventV1 {
    /// Validates the slashing entry.
    pub fn validate(&self) -> Result<(), PorSlashingEventValidationError> {
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(PorSlashingEventValidationError::InvalidProviderId);
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(PorSlashingEventValidationError::InvalidManifestDigest);
        }
        if self.verdict_cid.trim().is_empty()
            || self.verdict_cid.trim() != self.verdict_cid
            || self.verdict_cid.len() > POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1
            || self.verdict_cid.chars().any(char::is_control)
        {
            return Err(PorSlashingEventValidationError::InvalidVerdictCid);
        }
        if self.decided_at == 0 {
            return Err(PorSlashingEventValidationError::InvalidDecisionTimestamp);
        }
        Ok(())
    }
}
/// Validation errors for [`PorSlashingEventV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum PorSlashingEventValidationError {
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error(
        "verdict CID must be canonical non-empty UTF-8 of at most {POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1} bytes"
    )]
    InvalidVerdictCid,
    #[error("decision timestamp must be non-zero")]
    InvalidDecisionTimestamp,
}
/// Weekly PoR health report produced by the coordinator.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PorWeeklyReportV1 {
    /// Schema version (`POR_WEEKLY_REPORT_VERSION_V1`).
    pub version: u8,
    /// Reporting cycle identifier.
    pub cycle: PorReportIsoWeek,
    /// Timestamp when the report was generated.
    pub generated_at: u64,
    /// Total number of challenges during the cycle.
    pub challenges_total: u32,
    /// Number of verified challenges.
    pub challenges_verified: u32,
    /// Number of failed challenges.
    #[norito(default)]
    pub challenges_failed: u32,
    /// Number of forced challenges.
    #[norito(default)]
    pub forced_challenges: u32,
    /// Number of repair tasks enqueued.
    #[norito(default)]
    pub repairs_enqueued: u32,
    /// Number of repairs completed.
    #[norito(default)]
    pub repairs_completed: u32,
    /// Optional mean latency (milliseconds) across verified challenges.
    #[norito(default)]
    pub mean_latency_ms: Option<u64>,
    /// Optional P95 latency (milliseconds) across verified challenges.
    #[norito(default)]
    pub p95_latency_ms: Option<u64>,
    /// Slashing events recorded in the cycle, ordered strictly by
    /// `(decided_at, provider_id, manifest_digest, verdict_cid)`.
    #[norito(default)]
    pub slashing_events: Vec<PorSlashingEventV1>,
    /// Providers missing VRF submissions.
    #[norito(default)]
    pub providers_missing_vrf: Vec<[u8; 32]>,
    /// Top offending providers.
    #[norito(default)]
    pub top_offenders: Vec<PorProviderSummaryV1>,
    /// Optional notes for governance review.
    #[norito(default)]
    pub notes: Option<String>,
}
impl PorWeeklyReportV1 {
    /// Validates the weekly report payload.
    pub fn validate(&self) -> Result<(), PorWeeklyReportValidationError> {
        preflight_por_weekly_report_len(self, POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1)?;
        if self.version != POR_WEEKLY_REPORT_VERSION_V1 {
            return Err(PorWeeklyReportValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        self.cycle
            .validate()
            .map_err(PorWeeklyReportValidationError::InvalidIsoWeek)?;
        if self.generated_at == 0 {
            return Err(PorWeeklyReportValidationError::InvalidGeneratedAt);
        }
        let verified = u64::from(self.challenges_verified);
        let failed = u64::from(self.challenges_failed);
        let total = u64::from(self.challenges_total);
        if verified + failed > total {
            return Err(PorWeeklyReportValidationError::InvalidChallengeTotals);
        }
        if self.forced_challenges > self.challenges_total {
            return Err(PorWeeklyReportValidationError::InvalidForcedChallengeTotal);
        }
        if self.repairs_completed > self.repairs_enqueued {
            return Err(PorWeeklyReportValidationError::InvalidRepairTotals);
        }
        if self
            .mean_latency_ms
            .zip(self.p95_latency_ms)
            .is_some_and(|(mean, p95)| p95 < mean)
        {
            return Err(PorWeeklyReportValidationError::InvalidLatencyOrder);
        }
        if self.top_offenders.len() > 10 {
            return Err(PorWeeklyReportValidationError::TooManyTopOffenders {
                count: self.top_offenders.len(),
            });
        }
        if self.slashing_events.len() > POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1 {
            return Err(PorWeeklyReportValidationError::TooManySlashingEvents {
                found: self.slashing_events.len(),
                maximum: POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1,
            });
        }
        if self.slashing_events.len()
            > usize::try_from(self.challenges_failed).unwrap_or(usize::MAX)
        {
            return Err(
                PorWeeklyReportValidationError::SlashingEventsExceedFailures {
                    slashing_events: self.slashing_events.len(),
                    failed_challenges: self.challenges_failed,
                },
            );
        }
        if self.providers_missing_vrf.len() > POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1 {
            return Err(PorWeeklyReportValidationError::TooManyMissingVrfProviders {
                found: self.providers_missing_vrf.len(),
                maximum: POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1,
            });
        }
        if self.providers_missing_vrf.len()
            > usize::try_from(self.forced_challenges).unwrap_or(usize::MAX)
        {
            return Err(
                PorWeeklyReportValidationError::MissingVrfProvidersExceedForcedChallenges {
                    providers: self.providers_missing_vrf.len(),
                    forced_challenges: self.forced_challenges,
                },
            );
        }
        for (index, provider) in self.top_offenders.iter().enumerate() {
            provider.validate().map_err(|err| {
                PorWeeklyReportValidationError::InvalidProviderSummary { index, source: err }
            })?;
            if self.top_offenders[..index]
                .iter()
                .any(|prior| prior.provider_id == provider.provider_id)
            {
                return Err(PorWeeklyReportValidationError::DuplicateTopOffender { index });
            }
            if index > 0 {
                let prior = &self.top_offenders[index - 1];
                let out_of_order = prior.failures < provider.failures
                    || (prior.failures == provider.failures && prior.forced < provider.forced)
                    || (prior.failures == provider.failures
                        && prior.forced == provider.forced
                        && prior.provider_id >= provider.provider_id);
                if out_of_order {
                    return Err(PorWeeklyReportValidationError::UnsortedTopOffenders { index });
                }
            }
        }
        for (index, event) in self.slashing_events.iter().enumerate() {
            event.validate().map_err(|err| {
                PorWeeklyReportValidationError::InvalidSlashingEvent { index, source: err }
            })?;
            if index > 0 {
                let prior = &self.slashing_events[index - 1];
                let prior_key = (
                    prior.decided_at,
                    prior.provider_id,
                    prior.manifest_digest,
                    prior.verdict_cid.as_str(),
                );
                let current_key = (
                    event.decided_at,
                    event.provider_id,
                    event.manifest_digest,
                    event.verdict_cid.as_str(),
                );
                if prior_key >= current_key {
                    return Err(
                        PorWeeklyReportValidationError::UnsortedOrDuplicateSlashingEvent { index },
                    );
                }
            }
        }
        for (index, provider) in self.providers_missing_vrf.iter().enumerate() {
            if provider.iter().all(|&byte| byte == 0) {
                return Err(PorWeeklyReportValidationError::InvalidMissingVrfProvider { index });
            }
            if index > 0 && self.providers_missing_vrf[index - 1] >= *provider {
                return Err(PorWeeklyReportValidationError::UnsortedMissingVrfProviders { index });
            }
        }
        if self.notes.as_ref().is_some_and(|notes| {
            notes.trim().is_empty()
                || notes.trim() != notes
                || notes.len() > POR_WEEKLY_REPORT_NOTES_MAX_BYTES_V1
                || notes.chars().any(char::is_control)
        }) {
            return Err(PorWeeklyReportValidationError::InvalidNotes);
        }
        Ok(())
    }
}
/// Validation errors for [`PorWeeklyReportV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorWeeklyReportValidationError {
    #[error("weekly PoR report does not expose an exact canonical encoded length")]
    CanonicalLengthUnavailable,
    #[error("weekly PoR report has {found} canonical bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    #[error("unsupported weekly report version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("invalid ISO week specified")]
    InvalidIsoWeek(#[from] PorReportIsoWeekValidationError),
    #[error("generated_at timestamp must be non-zero")]
    InvalidGeneratedAt,
    #[error("challenge totals are inconsistent")]
    InvalidChallengeTotals,
    #[error("forced challenge total exceeds total challenges")]
    InvalidForcedChallengeTotal,
    #[error("completed repair total exceeds enqueued repairs")]
    InvalidRepairTotals,
    #[error("p95 latency must be greater than or equal to mean latency")]
    InvalidLatencyOrder,
    #[error("weekly report contains {count} top offenders; at most 10 are allowed")]
    TooManyTopOffenders { count: usize },
    #[error("weekly report contains {found} slashing events; maximum is {maximum}")]
    TooManySlashingEvents { found: usize, maximum: usize },
    #[error(
        "weekly report contains {slashing_events} slashing events but only {failed_challenges} failed challenges"
    )]
    SlashingEventsExceedFailures {
        slashing_events: usize,
        failed_challenges: u32,
    },
    #[error("weekly report contains {found} missing-VRF providers; maximum is {maximum}")]
    TooManyMissingVrfProviders { found: usize, maximum: usize },
    #[error(
        "weekly report contains {providers} missing-VRF providers but only {forced_challenges} forced challenges"
    )]
    MissingVrfProvidersExceedForcedChallenges {
        providers: usize,
        forced_challenges: u32,
    },
    #[error("provider summary #{index} invalid: {source}")]
    InvalidProviderSummary {
        index: usize,
        source: PorProviderSummaryValidationError,
    },
    #[error("top offender entry #{index} duplicates an earlier provider")]
    DuplicateTopOffender { index: usize },
    #[error("top offender entry #{index} is not in canonical order")]
    UnsortedTopOffenders { index: usize },
    #[error("slashing event #{index} invalid: {source}")]
    InvalidSlashingEvent {
        index: usize,
        source: PorSlashingEventValidationError,
    },
    #[error(
        "slashing event #{index} is duplicate or not ordered by decided_at/provider/manifest/verdict CID"
    )]
    UnsortedOrDuplicateSlashingEvent { index: usize },
    #[error("providers_missing_vrf entry #{index} must be non-zero")]
    InvalidMissingVrfProvider { index: usize },
    #[error("providers_missing_vrf entry #{index} is duplicate or not in canonical order")]
    UnsortedMissingVrfProviders { index: usize },
    #[error(
        "notes must be canonical non-empty UTF-8 of at most {POR_WEEKLY_REPORT_NOTES_MAX_BYTES_V1} bytes"
    )]
    InvalidNotes,
}
fn preflight_por_weekly_report_len(
    report: &PorWeeklyReportV1,
    maximum: usize,
) -> Result<usize, PorWeeklyReportValidationError> {
    let found = report
        .encoded_len_exact()
        .ok_or(PorWeeklyReportValidationError::CanonicalLengthUnavailable)?;
    if found > maximum {
        return Err(PorWeeklyReportValidationError::PayloadTooLarge { found, maximum });
    }
    Ok(found)
}
/// Decode and validate one bounded canonical V1 weekly PoR report.
///
/// # Errors
///
/// Returns a Norito error for oversized, noncanonical, malformed, or
/// structurally invalid report bytes.
pub fn decode_por_weekly_report_v1(bytes: &[u8]) -> Result<PorWeeklyReportV1, norito::core::Error> {
    let report: PorWeeklyReportV1 = decode_bounded_canonical_por_payload(
        "weekly PoR report",
        bytes,
        POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1,
        norito::DecodeLimits::new(
            POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1
                .max(POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1),
            POR_WEEKLY_REPORT_NOTES_MAX_BYTES_V1,
            300_000,
            POR_WEEKLY_REPORT_MAX_CANONICAL_BYTES_V1 * 4,
            64,
        ),
    )?;
    report
        .validate()
        .map_err(|error| norito::core::Error::Message(error.to_string()))?;
    Ok(report)
}
#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};
    use super::*;
    fn encode_bare_with_flags<T: norito::core::NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
        let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
        let mut bytes = Vec::new();
        norito::core::serialize_to_buffer(value, &mut bytes).expect("serialize explicit layout");
        bytes
    }
    fn encode_frame_with_flags<T: norito::core::NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
        let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
        norito::to_bytes(value).expect("serialize explicit canonical frame")
    }
    fn supported_layouts() -> [u8; 8] {
        use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
        [
            0,
            COMPACT_LEN,
            PACKED_SEQ,
            PACKED_SEQ | COMPACT_LEN,
            PACKED_STRUCT,
            PACKED_STRUCT | COMPACT_LEN,
            PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
            PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        ]
    }
    #[derive(norito::derive::NoritoSerialize)]
    struct LegacyPorChallengeStatusV1 {
        version: u8,
        challenge_id: [u8; 32],
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
        epoch_id: u64,
        drand_round: u64,
        status: PorChallengeOutcome,
        sample_count: u16,
        forced: bool,
        issued_at: u64,
        responded_at: Option<u64>,
        proof_digest: Option<[u8; 32]>,
        repair_task_id: Option<[u8; 16]>,
        failure_reason: Option<String>,
        verifier_latency_ms: Option<u32>,
    }
    #[derive(norito::derive::NoritoSerialize)]
    struct MissingRepairTaskFieldStatusV1 {
        version: u8,
        challenge_id: [u8; 32],
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
        epoch_id: u64,
        drand_round: u64,
        status: PorChallengeOutcome,
        sample_count: u16,
        forced: bool,
        issued_at: u64,
        responded_at: Option<u64>,
        proof_digest: Option<[u8; 32]>,
        failure_reason: Option<String>,
        verifier_latency_ms: Option<u32>,
    }
    fn challenge_fixture(sample_indices: Vec<u64>) -> PorChallengeV1 {
        let manifest_digest = [2; 32];
        let provider_id = [3; 32];
        let epoch_id = 7;
        let drand_round = 42;
        let drand_randomness = [0x44; 32];
        let vrf_output = [0x55; 32];
        let seed = derive_challenge_seed(
            &drand_randomness,
            Some(&vrf_output),
            &manifest_digest,
            epoch_id,
        );
        let challenge_id =
            derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
        PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id,
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: [0x66; 48],
            vrf_output: Some(vrf_output),
            vrf_proof: Some(iroha_crypto::vrf::VrfProof::SigInG1([0x77; 48])),
            forced: false,
            chunking_profile: "sorafs.sf1@1.0.0".to_string(),
            seed,
            sample_tier: 2,
            sample_count: u16::try_from(sample_indices.len()).expect("bounded sample fixture"),
            sample_indices,
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_900,
        }
    }
    fn provider_vrf_submission_fixture() -> ProviderVrfSubmissionV1 {
        ProviderVrfSubmissionV1 {
            version: POR_VRF_SUBMISSION_VERSION_V1,
            network_id: [0x31; 32],
            provider_id: [1; 32],
            manifest_digest: [2; 32],
            epoch_id: 3,
            drand_round: 4,
            output: [5; 32],
            proof: iroha_crypto::vrf::VrfProof::SigInG1([6; 48]),
            sequence: 7,
            issued_at: 8,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![9; PUBLIC_KEY_LENGTH],
                signature: vec![10; SIGNATURE_LENGTH],
            },
        }
    }
    fn proof_fixture() -> PorProofV1 {
        PorProofV1 {
            version: POR_PROOF_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            samples: vec![PorProofSampleV1 {
                sample_index: 10,
                chunk_offset: 0,
                chunk_size: 65_536,
                chunk_digest: [4; 32],
                leaf_digest: [5; 32],
            }],
            auth_path: vec![[6; 32], [7; 32]],
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![8; 32],
                signature: vec![9; 64],
            },
            submitted_at: 1_700_000_100,
        }
    }
    fn sign_proof(proof: &mut PorProofV1, signing_key: &SigningKey) {
        proof.signature.public_key = signing_key.verifying_key().to_bytes().to_vec();
        let payload = proof
            .signature_payload_bytes()
            .expect("encode proof signing payload");
        proof.signature.signature = signing_key.sign(&payload).to_bytes().to_vec();
    }
    fn verdict_fixture() -> AuditVerdictV1 {
        AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: [1; 32],
            provider_id: [2; 32],
            challenge_id: [3; 32],
            proof_digest: Some([4; 32]),
            outcome: AuditOutcomeV1::Success,
            failure_reason: None,
            decided_at: 1_700_000_500,
            auditor_signatures: Vec::new(),
            metadata: Vec::new(),
        }
    }
    fn add_verdict_signature(verdict: &mut AuditVerdictV1, signing_key: &SigningKey) {
        let payload = verdict
            .signature_payload_bytes()
            .expect("encode verdict signing payload");
        verdict.auditor_signatures.push(AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signing_key.sign(&payload).to_bytes().to_vec(),
        });
    }
    #[test]
    fn seed_derivation_is_stable() {
        let drand = [0xAA; 32];
        let vrf = [0xBB; 32];
        let manifest = [0xCC; 32];
        let epoch = 42;
        let seed_a = derive_challenge_seed(&drand, Some(&vrf), &manifest, epoch);
        let seed_b = derive_challenge_seed(&drand, Some(&vrf), &manifest, epoch);
        assert_eq!(seed_a, seed_b, "seed derivation must be deterministic");
        let seed_forced = derive_challenge_seed(&drand, None, &manifest, epoch);
        assert_ne!(
            seed_a, seed_forced,
            "missing VRF output should alter the derived seed"
        );
    }
    include!("por/provider_vrf_tests.rs");
    #[test]
    fn challenge_id_reflects_epoch_and_round() {
        let seed = [0x11; 32];
        let manifest = [0x22; 32];
        let provider = [0x33; 32];
        let id_epoch_10 = derive_challenge_id(&seed, &manifest, &provider, 10, 100);
        let id_epoch_11 = derive_challenge_id(&seed, &manifest, &provider, 11, 100);
        assert_ne!(
            id_epoch_10, id_epoch_11,
            "epoch should influence challenge identifier"
        );
        let id_round_101 = derive_challenge_id(&seed, &manifest, &provider, 10, 101);
        assert_ne!(
            id_epoch_10, id_round_101,
            "drand round must influence challenge identifier"
        );
    }
    #[test]
    fn challenge_validation_succeeds() {
        let manifest_digest = [2; 32];
        let provider_id = [3; 32];
        let epoch_id = 7;
        let drand_round = 42;
        let drand_randomness = [0x44; 32];
        let vrf_output = [0x55; 32];
        let seed = derive_challenge_seed(
            &drand_randomness,
            Some(&vrf_output),
            &manifest_digest,
            epoch_id,
        );
        let challenge_id =
            derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
        let challenge = PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id,
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: [0x66; 48],
            vrf_output: Some(vrf_output),
            vrf_proof: Some(iroha_crypto::vrf::VrfProof::SigInG1([0x77; 48])),
            forced: false,
            chunking_profile: "sorafs.sf1@1.0.0".to_string(),
            seed,
            sample_tier: 2,
            sample_count: 2,
            sample_indices: vec![10, 42],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_900,
        };
        assert!(challenge.validate().is_ok());
    }
    #[test]
    fn challenge_publication_binds_duplicate_count_and_roundtrips_canonically() {
        let publication =
            PorChallengePublicationV1::try_new(challenge_fixture(vec![10, 42, 42]), 1)
                .expect("valid challenge publication");
        assert!(publication.validate().is_ok());
        let encoded = norito::to_bytes(&publication).expect("encode challenge publication");
        let decoded: PorChallengePublicationV1 =
            norito::decode_from_bytes(&encoded).expect("decode challenge publication");
        assert_eq!(decoded, publication);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode challenge publication"),
            encoded
        );
    }
    #[test]
    fn challenge_and_publication_bounds_preflight_before_nested_work() {
        let mut challenge = challenge_fixture(
            (0..u64::try_from(POR_CHALLENGE_MAX_SAMPLES_V1)
                .expect("challenge sample ceiling fits u64"))
                .collect(),
        );
        challenge
            .validate()
            .expect("exact challenge sample boundary validates");
        let challenge_exact = challenge
            .encoded_len_exact()
            .expect("challenge exposes exact canonical length");
        assert_eq!(
            preflight_por_challenge_len(&challenge, challenge_exact),
            Ok(challenge_exact)
        );
        assert_eq!(
            preflight_por_challenge_len(&challenge, challenge_exact - 1),
            Err(PorChallengeValidationError::PayloadTooLarge {
                found: challenge_exact,
                maximum: challenge_exact - 1,
            })
        );
        validate_por_challenge_profile(&"p".repeat(POR_CHALLENGE_PROFILE_MAX_BYTES_V1))
            .expect("exact profile boundary");
        assert_eq!(
            validate_por_challenge_profile(&"p".repeat(POR_CHALLENGE_PROFILE_MAX_BYTES_V1 + 1)),
            Err(PorChallengeValidationError::InvalidChunkerProfile {
                found: POR_CHALLENGE_PROFILE_MAX_BYTES_V1 + 1,
                maximum: POR_CHALLENGE_PROFILE_MAX_BYTES_V1,
            })
        );
        let encoded = norito::to_bytes(&challenge).expect("encode bounded challenge");
        assert_eq!(
            decode_por_challenge_v1(&encoded).expect("bounded challenge decoder"),
            challenge
        );
        assert!(
            decode_por_challenge_v1(&vec![0; POR_CHALLENGE_MAX_CANONICAL_BYTES_V1 + 1]).is_err()
        );
        challenge.sample_count += 1;
        assert_eq!(
            challenge.validate(),
            Err(PorChallengeValidationError::TooManyDeclaredSamples {
                found: u16::try_from(POR_CHALLENGE_MAX_SAMPLES_V1 + 1)
                    .expect("sample ceiling plus one fits u16"),
                maximum: POR_CHALLENGE_MAX_SAMPLES_V1,
            })
        );
        challenge.sample_count -= 1;
        challenge.sample_indices.push(999);
        assert_eq!(
            challenge.validate(),
            Err(PorChallengeValidationError::TooManySampleIndices {
                found: POR_CHALLENGE_MAX_SAMPLES_V1 + 1,
                maximum: POR_CHALLENGE_MAX_SAMPLES_V1,
            })
        );
        let publication = PorChallengePublicationV1::try_new(challenge_fixture(vec![10, 42]), 0)
            .expect("bounded challenge publication");
        let publication_exact = publication
            .encoded_len_exact()
            .expect("publication exposes exact canonical length");
        assert_eq!(
            preflight_por_challenge_publication_len(&publication, publication_exact),
            Ok(publication_exact)
        );
        assert_eq!(
            preflight_por_challenge_publication_len(&publication, publication_exact - 1),
            Err(PorChallengePublicationValidationError::PayloadTooLarge {
                found: publication_exact,
                maximum: publication_exact - 1,
            })
        );
        let encoded = norito::to_bytes(&publication).expect("encode challenge publication");
        assert_eq!(
            decode_por_challenge_publication_v1(&encoded)
                .expect("bounded challenge-publication decoder"),
            publication
        );
        assert!(
            decode_por_challenge_publication_v1(&vec![
                0;
                POR_CHALLENGE_PUBLICATION_MAX_CANONICAL_BYTES_V1
                    + 1
            ])
            .is_err()
        );
    }
    #[test]
    fn challenge_publication_rejects_version_and_duplicate_count_tampering() {
        let publication =
            PorChallengePublicationV1::try_new(challenge_fixture(vec![10, 42, 42]), 1)
                .expect("valid challenge publication");
        let mut invalid = publication.clone();
        invalid.version = 2;
        assert_eq!(
            invalid.validate(),
            Err(PorChallengePublicationValidationError::UnsupportedVersion { found: 2 })
        );
        let mut invalid = publication.clone();
        invalid.duplicate_samples = 0;
        assert_eq!(
            invalid.validate(),
            Err(
                PorChallengePublicationValidationError::DuplicateSampleCountMismatch {
                    declared: 0,
                    actual: 1,
                }
            )
        );
        let mut invalid = publication;
        invalid.duplicate_samples = invalid.challenge.sample_count + 1;
        assert!(matches!(
            invalid.validate(),
            Err(
                PorChallengePublicationValidationError::DuplicateSampleCountExceedsSampleCount { .. }
            )
        ));
    }
    #[test]
    fn challenge_publication_rejects_architecture_dependent_duplicate_count() {
        let error = PorChallengePublicationV1::try_new(
            challenge_fixture(vec![10]),
            usize::from(u16::MAX) + 1,
        )
        .expect_err("oversized duplicate count must fail before publication");
        assert!(matches!(
            error,
            PorChallengePublicationValidationError::DuplicateSampleCountOutOfRange { .. }
        ));
    }
    #[test]
    fn challenge_validation_allows_forced_without_vrf() {
        let manifest_digest = [4; 32];
        let provider_id = [5; 32];
        let epoch_id = 8;
        let drand_round = 51;
        let drand_randomness = [0xAB; 32];
        let seed = derive_challenge_seed(&drand_randomness, None, &manifest_digest, epoch_id);
        let challenge_id =
            derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
        let challenge = PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id,
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: [0xCD; 48],
            vrf_output: None,
            vrf_proof: None,
            forced: true,
            chunking_profile: "sorafs.sf1@1.0.0".to_string(),
            seed,
            sample_tier: 1,
            sample_count: 1,
            sample_indices: vec![0],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_600,
        };
        assert!(challenge.validate().is_ok());
        let mut with_vrf = challenge.clone();
        with_vrf.vrf_output = Some([0x31; 32]);
        with_vrf.vrf_proof = Some(iroha_crypto::vrf::VrfProof::SigInG1([0x32; 48]));
        assert_eq!(
            with_vrf.validate(),
            Err(PorChallengeValidationError::ForcedWithVrf)
        );
        let mut orphan_proof = challenge;
        orphan_proof.vrf_proof = Some(iroha_crypto::vrf::VrfProof::SigInG1([0x33; 48]));
        assert_eq!(
            orphan_proof.validate(),
            Err(PorChallengeValidationError::ForcedWithOrphanProof)
        );
    }
    #[test]
    fn challenge_validation_rejects_missing_randomness() {
        let manifest_digest = [2; 32];
        let provider_id = [3; 32];
        let mut challenge = PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest,
            provider_id,
            epoch_id: 0,
            drand_round: 0,
            drand_randomness: [0; 32],
            drand_signature: [0; 48],
            vrf_output: None,
            vrf_proof: None,
            forced: true,
            chunking_profile: "sorafs.sf1@1.0.0".to_string(),
            seed: [9; 32],
            sample_tier: 1,
            sample_count: 1,
            sample_indices: vec![0],
            issued_at: 1,
            deadline_at: 2,
        };
        assert_eq!(
            challenge.validate(),
            Err(PorChallengeValidationError::MissingEpochId)
        );
        challenge.epoch_id = 5;
        assert_eq!(
            challenge.validate(),
            Err(PorChallengeValidationError::MissingDrandRound)
        );
        challenge.drand_round = 7;
        assert_eq!(
            challenge.validate(),
            Err(PorChallengeValidationError::InvalidDrandRandomness)
        );
        challenge.drand_randomness = [1; 32];
        assert_eq!(
            challenge.validate(),
            Err(PorChallengeValidationError::InvalidDrandSignature)
        );
    }
    #[test]
    fn proof_validation_succeeds() {
        let proof = proof_fixture();
        assert!(proof.validate().is_ok());
    }
    #[test]
    fn proof_bounds_accept_boundaries_and_reject_one_over() {
        let mut proof = proof_fixture();
        let sample = proof.samples[0].clone();
        proof.samples = vec![sample; POR_PROOF_MAX_SAMPLES_V1];
        proof.auth_path = vec![[6; 32]; POR_PROOF_MAX_AUTH_PATH_NODES_V1];
        assert!(proof.validate().is_ok());
        proof.samples.push(proof.samples[0].clone());
        assert_eq!(
            proof.validate(),
            Err(PorProofValidationError::TooManySamples {
                found: POR_PROOF_MAX_SAMPLES_V1 + 1,
                maximum: POR_PROOF_MAX_SAMPLES_V1,
            })
        );
        proof.samples.truncate(POR_PROOF_MAX_SAMPLES_V1);
        proof.auth_path.push([7; 32]);
        assert_eq!(
            proof.validate(),
            Err(PorProofValidationError::AuthPathTooDeep {
                found: POR_PROOF_MAX_AUTH_PATH_NODES_V1 + 1,
                maximum: POR_PROOF_MAX_AUTH_PATH_NODES_V1,
            })
        );
        let proof = proof_fixture();
        let exact = proof
            .encoded_len_exact()
            .expect("PoR proof exposes an exact canonical length");
        assert_eq!(preflight_por_proof_len(&proof, exact), Ok(exact));
        assert_eq!(
            preflight_por_proof_len(&proof, exact.saturating_sub(1)),
            Err(PorProofValidationError::PayloadTooLarge {
                found: exact,
                maximum: exact.saturating_sub(1),
            })
        );
        let encoded = norito::to_bytes(&proof).expect("encode bounded proof");
        assert_eq!(
            decode_por_proof_v1(&encoded).expect("decode bounded canonical proof"),
            proof
        );
        let mut allocation_bomb = proof_fixture();
        allocation_bomb.signature.signature = vec![9; POR_PROOF_MAX_CANONICAL_BYTES_V1];
        let error = allocation_bomb
            .signature_payload_bytes()
            .expect_err("oversized proof must fail before constructing a signing payload");
        assert!(error.to_string().contains("maximum"));
    }
    #[test]
    fn borrowed_por_proof_signing_view_is_byte_exact_for_every_layout() {
        let proof = proof_fixture();
        let owned = PorProofSigningPayloadV1::from(&proof);
        let borrowed = PorProofSigningPayloadViewV1::from(&proof);
        assert_eq!(
            <PorProofSigningPayloadViewV1<'_> as norito::core::NoritoSerialize>::schema_hash(),
            PorProofSigningPayloadV1::schema_hash()
        );
        assert_eq!(
            norito::to_bytes(&borrowed).expect("encode borrowed proof signing payload"),
            norito::to_bytes(&owned).expect("encode historical owned proof signing payload")
        );
        assert_eq!(
            proof
                .signature_payload_bytes()
                .expect("encode proof signing payload"),
            norito::to_bytes(&owned).expect("encode historical proof signing payload")
        );
        for flags in supported_layouts() {
            let owned_bytes = encode_bare_with_flags(&owned, flags);
            let borrowed_bytes = encode_bare_with_flags(&borrowed, flags);
            assert_eq!(
                borrowed_bytes, owned_bytes,
                "borrowed PoR proof signing bytes changed for flags 0x{flags:02x}"
            );
            let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
            assert_eq!(
                borrowed.encoded_len_exact(),
                owned.encoded_len_exact(),
                "borrowed PoR proof signing size changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                norito::core::encoded_payload_len(&borrowed)
                    .expect("borrowed proof length must be countable"),
                borrowed_bytes.len()
            );
            let owned_frame = encode_frame_with_flags(&owned, flags);
            assert_eq!(
                encode_frame_with_flags(&borrowed, flags),
                owned_frame,
                "borrowed PoR proof canonical frame or layout flags changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                proof
                    .signature_payload_bytes()
                    .expect("encode borrowed proof signing payload"),
                owned_frame,
                "PoR proof signature frame changed for flags 0x{flags:02x}"
            );
        }
    }
    #[test]
    fn proof_validation_requires_exact_ed25519_lengths() {
        let mut proof = proof_fixture();
        proof.signature.public_key.pop();
        assert_eq!(
            proof.validate(),
            Err(PorProofValidationError::InvalidPublicKeyLength {
                found: PUBLIC_KEY_LENGTH - 1,
                expected: PUBLIC_KEY_LENGTH,
            })
        );
        let mut proof = proof_fixture();
        proof.signature.signature.push(9);
        assert_eq!(
            proof.validate(),
            Err(PorProofValidationError::InvalidSignatureLength {
                found: SIGNATURE_LENGTH + 1,
                expected: SIGNATURE_LENGTH,
            })
        );
    }
    #[test]
    fn proof_signature_covers_provider_timestamp_and_payload() {
        let signing_key = SigningKey::from_bytes(&[0x41; 32]);
        let mut proof = proof_fixture();
        sign_proof(&mut proof, &signing_key);
        proof.verify_signature().expect("valid proof signature");
        let original_digest = proof.proof_digest();
        for mutation in 0..6 {
            let mut tampered = proof.clone();
            match mutation {
                0 => tampered.provider_id[0] ^= 1,
                1 => tampered.manifest_digest[0] ^= 1,
                2 => tampered.challenge_id[0] ^= 1,
                3 => tampered.samples[0].chunk_offset ^= 1,
                4 => tampered.auth_path[0][0] ^= 1,
                5 => tampered.submitted_at += 1,
                _ => unreachable!(),
            }
            assert_ne!(tampered.proof_digest(), original_digest);
            assert!(
                matches!(
                    tampered.verify_signature(),
                    Err(PorSignatureVerificationError::Verification { .. })
                ),
                "mutation {mutation} must invalidate the signature"
            );
        }
    }
    #[test]
    fn proof_signature_rejects_wrong_key_and_malformed_material() {
        let signing_key = SigningKey::from_bytes(&[0x42; 32]);
        let mut proof = proof_fixture();
        sign_proof(&mut proof, &signing_key);
        proof.signature.public_key = SigningKey::from_bytes(&[0x43; 32])
            .verifying_key()
            .to_bytes()
            .to_vec();
        assert!(matches!(
            proof.verify_signature(),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
        proof.signature.public_key = vec![1; 31];
        assert_eq!(
            proof.verify_signature(),
            Err(PorSignatureVerificationError::InvalidPublicKeyLength { length: 31 })
        );
        proof.signature.public_key = signing_key.verifying_key().to_bytes().to_vec();
        proof.signature.signature = vec![1; 63];
        assert_eq!(
            proof.verify_signature(),
            Err(PorSignatureVerificationError::InvalidSignatureLength { length: 63 })
        );
    }
    #[test]
    fn proof_signature_must_match_admitted_provider_key() {
        let signing_key = SigningKey::from_bytes(&[0x44; 32]);
        let other_key = SigningKey::from_bytes(&[0x45; 32]);
        let mut proof = proof_fixture();
        sign_proof(&mut proof, &signing_key);
        proof
            .verify_signature_for_provider(&signing_key.verifying_key().to_bytes())
            .expect("admitted provider signature");
        assert_eq!(
            proof.verify_signature_for_provider(&other_key.verifying_key().to_bytes()),
            Err(PorSignatureVerificationError::ProviderSignerMismatch)
        );
    }
    #[test]
    fn proof_validation_rejects_all_zero_signature_material() {
        let mut proof = PorProofV1 {
            version: POR_PROOF_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            samples: vec![PorProofSampleV1 {
                sample_index: 10,
                chunk_offset: 0,
                chunk_size: 65_536,
                chunk_digest: [4; 32],
                leaf_digest: [5; 32],
            }],
            auth_path: vec![[6; 32], [7; 32]],
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![8; 32],
                signature: vec![0; 64],
            },
            submitted_at: 1_700_000_100,
        };
        assert_eq!(
            proof.validate(),
            Err(PorProofValidationError::InvalidSignature)
        );
        proof.signature.signature = vec![9; 64];
        proof.signature.public_key = vec![0; 32];
        assert_eq!(
            proof.validate(),
            Err(PorProofValidationError::InvalidSignature)
        );
    }
    #[test]
    fn verdict_requires_signatures() {
        let verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: [1; 32],
            provider_id: [2; 32],
            challenge_id: [3; 32],
            proof_digest: Some([4; 32]),
            outcome: AuditOutcomeV1::Success,
            failure_reason: None,
            decided_at: 1_700_000_500,
            auditor_signatures: vec![AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![5; 32],
                signature: vec![6; 64],
            }],
            metadata: Vec::new(),
        };
        assert!(verdict.validate().is_ok());
    }
    #[test]
    fn verdict_bounds_accept_boundaries_and_reject_one_over() {
        let mut verdict = verdict_fixture();
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("x".repeat(AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1));
        verdict.auditor_signatures = (1..=AUDIT_VERDICT_MAX_SIGNATURES_V1)
            .map(|index| AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![u8::try_from(index).expect("signature index fits u8"); 32],
                signature: vec![u8::try_from(index).expect("signature index fits u8"); 64],
            })
            .collect();
        verdict.metadata = (0..AUDIT_VERDICT_MAX_METADATA_ENTRIES_V1)
            .map(|index| CapacityMetadataEntry {
                key: format!("row.{index:02}"),
                value: "v".to_owned(),
            })
            .collect();
        assert!(verdict.validate().is_ok());
        let mut too_many_signatures = verdict.clone();
        too_many_signatures
            .auditor_signatures
            .push(AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0xA5; 32],
                signature: vec![0xA5; 64],
            });
        assert_eq!(
            too_many_signatures.validate(),
            Err(AuditVerdictValidationError::TooManySignatures {
                found: AUDIT_VERDICT_MAX_SIGNATURES_V1 + 1,
                maximum: AUDIT_VERDICT_MAX_SIGNATURES_V1,
            })
        );
        let mut reason_too_long = verdict.clone();
        reason_too_long.auditor_signatures.truncate(1);
        reason_too_long.failure_reason =
            Some("x".repeat(AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1 + 1));
        assert_eq!(
            reason_too_long.validate(),
            Err(AuditVerdictValidationError::FailureReasonTooLong {
                found: AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1 + 1,
                maximum: AUDIT_VERDICT_FAILURE_REASON_MAX_BYTES_V1,
            })
        );
        for invalid_reason in [" padded", "padded ", "line\nbreak", "\u{7f}"] {
            let mut invalid = verdict.clone();
            invalid.auditor_signatures.truncate(1);
            invalid.failure_reason = Some(invalid_reason.to_owned());
            assert_eq!(
                invalid.validate(),
                Err(AuditVerdictValidationError::InvalidFailureReason)
            );
        }
        let mut too_many_metadata = verdict;
        too_many_metadata.auditor_signatures.truncate(1);
        too_many_metadata.metadata.push(CapacityMetadataEntry {
            key: "overflow".to_owned(),
            value: "v".to_owned(),
        });
        assert_eq!(
            too_many_metadata.validate(),
            Err(AuditVerdictValidationError::TooManyMetadataEntries {
                found: AUDIT_VERDICT_MAX_METADATA_ENTRIES_V1 + 1,
                maximum: AUDIT_VERDICT_MAX_METADATA_ENTRIES_V1,
            })
        );
    }
    #[test]
    fn verdict_reuses_capacity_metadata_bounds_and_rejects_duplicate_signers() {
        let mut verdict = verdict_fixture();
        verdict.auditor_signatures = vec![
            AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![5; 32],
                signature: vec![6; 64],
            },
            AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![5; 32],
                signature: vec![7; 64],
            },
        ];
        assert_eq!(
            verdict.validate(),
            Err(AuditVerdictValidationError::DuplicateAuditorSigner { index: 1 })
        );
        verdict.auditor_signatures.truncate(1);
        verdict.metadata = vec![CapacityMetadataEntry {
            key: "audit.note".to_owned(),
            value: "x".repeat(crate::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES + 1),
        }];
        assert!(matches!(
            verdict.validate(),
            Err(AuditVerdictValidationError::InvalidMetadata { index: 0, .. })
        ));
        verdict.metadata = (0..16)
            .map(|index| CapacityMetadataEntry {
                key: format!("row.{index:02}"),
                value: "x".repeat(crate::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES - 6),
            })
            .collect();
        assert_eq!(
            verdict
                .metadata
                .iter()
                .map(|entry| entry.key.len() + entry.value.len())
                .sum::<usize>(),
            AUDIT_VERDICT_MAX_METADATA_BYTES_V1
        );
        verdict
            .validate()
            .expect("exact audit metadata aggregate boundary validates");
        verdict.metadata[0].value.push('x');
        assert_eq!(
            verdict.validate(),
            Err(AuditVerdictValidationError::MetadataTooLarge {
                found: AUDIT_VERDICT_MAX_METADATA_BYTES_V1 + 1,
                maximum: AUDIT_VERDICT_MAX_METADATA_BYTES_V1,
            })
        );
    }
    #[test]
    fn verdict_size_preflight_accepts_boundary_and_rejects_one_over() {
        let mut verdict = verdict_fixture();
        verdict.auditor_signatures.push(AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![5; 32],
            signature: vec![6; 64],
        });
        let exact = verdict
            .encoded_len_exact()
            .expect("audit verdict exposes an exact canonical length");
        assert_eq!(preflight_audit_verdict_len(&verdict, exact), Ok(exact));
        assert_eq!(
            preflight_audit_verdict_len(&verdict, exact.saturating_sub(1)),
            Err(AuditVerdictValidationError::PayloadTooLarge {
                found: exact,
                maximum: exact.saturating_sub(1),
            })
        );
        let encoded = norito::to_bytes(&verdict).expect("encode bounded verdict");
        assert_eq!(
            decode_audit_verdict_v1(&encoded).expect("decode bounded canonical verdict"),
            verdict
        );
        let mut allocation_bomb = verdict_fixture();
        allocation_bomb.auditor_signatures.push(AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![5; 32],
            signature: vec![6; 64],
        });
        allocation_bomb.auditor_signatures[0].signature =
            vec![9; AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1];
        let error = allocation_bomb
            .signature_payload_bytes()
            .expect_err("oversized verdict must fail before constructing a signing payload");
        assert!(error.to_string().contains("maximum"));
    }
    #[test]
    fn borrowed_audit_verdict_signing_view_is_byte_exact_for_every_layout() {
        let mut verdict = verdict_fixture();
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("provider missed the challenge deadline".to_owned());
        verdict.metadata.push(CapacityMetadataEntry {
            key: "repair.ticket".to_owned(),
            value: "ticket-7".to_owned(),
        });
        let owned = AuditVerdictSigningPayloadV1::from(&verdict);
        let borrowed = AuditVerdictSigningPayloadViewV1::from(&verdict);
        assert_eq!(
            <AuditVerdictSigningPayloadViewV1<'_> as norito::core::NoritoSerialize>::schema_hash(),
            AuditVerdictSigningPayloadV1::schema_hash()
        );
        assert_eq!(
            norito::to_bytes(&borrowed).expect("encode borrowed verdict signing payload"),
            norito::to_bytes(&owned).expect("encode historical owned verdict signing payload")
        );
        assert_eq!(
            verdict
                .signature_payload_bytes()
                .expect("encode verdict signing payload"),
            norito::to_bytes(&owned).expect("encode historical verdict signing payload")
        );
        for flags in supported_layouts() {
            let owned_bytes = encode_bare_with_flags(&owned, flags);
            let borrowed_bytes = encode_bare_with_flags(&borrowed, flags);
            assert_eq!(
                borrowed_bytes, owned_bytes,
                "borrowed audit-verdict signing bytes changed for flags 0x{flags:02x}"
            );
            let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
            assert_eq!(
                borrowed.encoded_len_exact(),
                owned.encoded_len_exact(),
                "borrowed audit-verdict signing size changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                norito::core::encoded_payload_len(&borrowed)
                    .expect("borrowed audit verdict length must be countable"),
                borrowed_bytes.len()
            );
            let owned_frame = encode_frame_with_flags(&owned, flags);
            assert_eq!(
                encode_frame_with_flags(&borrowed, flags),
                owned_frame,
                "borrowed audit-verdict canonical frame or layout flags changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                verdict
                    .signature_payload_bytes()
                    .expect("encode borrowed verdict signing payload"),
                owned_frame,
                "audit-verdict signature frame changed for flags 0x{flags:02x}"
            );
        }
    }
    #[test]
    fn verdict_signatures_cover_every_decision_field() {
        let signing_key = SigningKey::from_bytes(&[0x51; 32]);
        let mut verdict = verdict_fixture();
        add_verdict_signature(&mut verdict, &signing_key);
        verdict
            .verify_signatures()
            .expect("valid auditor signature");
        assert!(verdict.has_signer(&signing_key.verifying_key().to_bytes()));
        let mut tampered = verdict.clone();
        tampered.outcome = AuditOutcomeV1::Failed;
        tampered.failure_reason = Some("forged failure".to_owned());
        assert!(matches!(
            tampered.verify_signatures(),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
        let mut cross_provider = verdict.clone();
        cross_provider.provider_id[0] ^= 1;
        assert!(matches!(
            cross_provider.verify_signatures(),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
        let mut metadata_tamper = verdict.clone();
        metadata_tamper.metadata.push(CapacityMetadataEntry {
            key: "audit.note".to_owned(),
            value: "tampered".to_owned(),
        });
        assert!(matches!(
            metadata_tamper.verify_signatures(),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
    }
    #[test]
    fn verdict_rejects_duplicate_auditor_signer() {
        let signing_key = SigningKey::from_bytes(&[0x52; 32]);
        let mut verdict = verdict_fixture();
        add_verdict_signature(&mut verdict, &signing_key);
        verdict
            .auditor_signatures
            .push(verdict.auditor_signatures[0].clone());
        assert_eq!(
            verdict.verify_signatures(),
            Err(PorSignatureVerificationError::DuplicateSigner)
        );
    }
    #[test]
    fn verdict_policy_rejects_untrusted_padding_and_enforces_threshold() {
        let first = SigningKey::from_bytes(&[0x53; 32]);
        let second = SigningKey::from_bytes(&[0x54; 32]);
        let attacker = SigningKey::from_bytes(&[0x55; 32]);
        let trusted = vec![
            first.verifying_key().to_bytes().to_vec(),
            second.verifying_key().to_bytes().to_vec(),
        ];
        let mut one_signature = verdict_fixture();
        add_verdict_signature(&mut one_signature, &first);
        assert_eq!(
            one_signature.verify_signatures_with_policy(&trusted, 2),
            Err(
                PorSignatureVerificationError::InsufficientTrustedAuditorSignatures {
                    actual: 1,
                    required: 2,
                }
            )
        );
        let mut threshold_verdict = verdict_fixture();
        add_verdict_signature(&mut threshold_verdict, &first);
        add_verdict_signature(&mut threshold_verdict, &second);
        threshold_verdict
            .verify_signatures_with_policy(&trusted, 2)
            .expect("two trusted auditors satisfy threshold");
        let mut padded = verdict_fixture();
        add_verdict_signature(&mut padded, &first);
        add_verdict_signature(&mut padded, &attacker);
        assert_eq!(
            padded.verify_signatures_with_policy(&trusted, 2),
            Err(PorSignatureVerificationError::UntrustedAuditorSigner)
        );
        assert_eq!(
            threshold_verdict.verify_signatures_with_policy(&[], 1),
            Err(PorSignatureVerificationError::EmptyTrustedAuditorSet)
        );
        assert_eq!(
            threshold_verdict.verify_signatures_with_policy(&trusted, 3),
            Err(PorSignatureVerificationError::InvalidAuditorThreshold {
                threshold: 3,
                trusted: 2,
            })
        );
        assert_eq!(
            threshold_verdict.verify_signatures_with_policy(&trusted, 0),
            Err(PorSignatureVerificationError::InvalidAuditorThreshold {
                threshold: 0,
                trusted: 2,
            })
        );
    }
    #[test]
    fn verdict_rejects_all_zero_auditor_signature_material() {
        let mut verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: [1; 32],
            provider_id: [2; 32],
            challenge_id: [3; 32],
            proof_digest: Some([4; 32]),
            outcome: AuditOutcomeV1::Success,
            failure_reason: None,
            decided_at: 1_700_000_500,
            auditor_signatures: vec![AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![5; 32],
                signature: vec![0; 64],
            }],
            metadata: Vec::new(),
        };
        assert_eq!(
            verdict.validate(),
            Err(AuditVerdictValidationError::InvalidSignature)
        );
        verdict.auditor_signatures[0].signature = vec![6; 64];
        verdict.auditor_signatures[0].public_key = vec![0; 32];
        assert_eq!(
            verdict.validate(),
            Err(AuditVerdictValidationError::InvalidSignature)
        );
    }
    #[test]
    fn challenge_outcome_parse_roundtrip() {
        for outcome in [
            PorChallengeOutcome::AwaitingProof,
            PorChallengeOutcome::ProofSubmitted,
            PorChallengeOutcome::Verified,
            PorChallengeOutcome::Failed,
            PorChallengeOutcome::Repaired,
        ] {
            let label = outcome.as_str();
            let parsed = PorChallengeOutcome::parse(label).expect("parse outcome");
            assert_eq!(outcome, parsed);
            let numeric: u8 = outcome.into();
            let from_numeric = PorChallengeOutcome::try_from(numeric).expect("from numeric");
            assert_eq!(outcome, from_numeric);
        }
    }
    #[test]
    fn status_cursor_codec_is_bounded_canonical_and_complete() {
        let cursor = PorStatusCursorV1 {
            version: POR_STATUS_CURSOR_VERSION_V1,
            snapshot_generation: 17,
            selection_digest: [1; 32],
            last_epoch_id: 42,
            last_issued_at: 1_700_000_000,
            last_challenge_id: [2; 32],
        };
        let opaque = cursor.encode_opaque().expect("encode canonical cursor");
        assert!(opaque.len() <= POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1);
        assert_eq!(PorStatusCursorV1::decode_opaque(&opaque).unwrap(), cursor);
        let mut noncanonical = opaque.clone();
        noncanonical.push('=');
        assert!(matches!(
            PorStatusCursorV1::decode_opaque(&noncanonical),
            Err(PorStatusCursorCodecError::Base64)
                | Err(PorStatusCursorCodecError::EncodedLength { .. })
        ));
        assert!(matches!(
            PorStatusCursorV1::decode_opaque(
                &"A".repeat(POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1 + 1)
            ),
            Err(PorStatusCursorCodecError::EncodedLength { .. })
        ));
        let invalid = PorStatusCursorV1 {
            last_epoch_id: 0,
            ..cursor
        };
        assert_eq!(
            invalid.validate(),
            Err(PorStatusCursorValidationError::InvalidEpoch)
        );
    }
    #[test]
    fn challenge_status_requires_failure_reason() {
        let status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 99,
            status: PorChallengeOutcome::Failed,
            sample_count: 64,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: None,
            proof_digest: None,
            repair_task_id: Some([4; 32]),
            failure_reason: None,
            verifier_latency_ms: None,
        };
        let err = status
            .validate()
            .expect_err("missing failure reason rejected");
        assert_eq!(err, PorChallengeStatusValidationError::MissingFailureReason);
    }
    #[test]
    fn failed_challenge_status_requires_native_repair_task() {
        let mut status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 99,
            status: PorChallengeOutcome::Failed,
            sample_count: 64,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: None,
            proof_digest: None,
            repair_task_id: None,
            failure_reason: Some("provider missed deadline".to_owned()),
            verifier_latency_ms: None,
        };
        assert_eq!(
            status.validate(),
            Err(PorChallengeStatusValidationError::MissingRepairTaskId)
        );
        status.repair_task_id = Some([4; 32]);
        status.validate().expect("canonical failed status");
        status.status = PorChallengeOutcome::Repaired;
        status.responded_at = Some(1_700_000_100);
        status.proof_digest = Some([5; 32]);
        assert_eq!(
            status.validate(),
            Err(PorChallengeStatusValidationError::UnexpectedRepairTaskId)
        );
    }
    #[test]
    fn challenge_status_outcome_material_matrix_is_strict() {
        let mut status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 99,
            status: PorChallengeOutcome::AwaitingProof,
            sample_count: 64,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: None,
            proof_digest: None,
            repair_task_id: None,
            failure_reason: None,
            verifier_latency_ms: None,
        };
        status
            .validate()
            .expect("material-free awaiting-proof status");
        status.responded_at = Some(1_700_000_100);
        status.proof_digest = Some([4; 32]);
        assert_eq!(
            status.validate(),
            Err(PorChallengeStatusValidationError::UnexpectedProofMaterial)
        );
        status.status = PorChallengeOutcome::ProofSubmitted;
        status.forced = true;
        status
            .validate()
            .expect("forced provenance is orthogonal to proof submission");
        status.status = PorChallengeOutcome::Verified;
        status
            .validate()
            .expect("verified status carries proof response material");
        status.proof_digest = None;
        assert_eq!(
            status.validate(),
            Err(PorChallengeStatusValidationError::InconsistentProofMaterial)
        );
        status.proof_digest = Some([4; 32]);
        status.status = PorChallengeOutcome::Repaired;
        status.failure_reason = Some("recovered after failed audit".to_owned());
        status
            .validate()
            .expect("repaired status retains failure and proof material");
        status.status = PorChallengeOutcome::Failed;
        status.responded_at = None;
        status.proof_digest = None;
        status.verifier_latency_ms = None;
        status.repair_task_id = Some([5; 32]);
        status
            .validate()
            .expect("proofless failure keeps response material absent");
    }
    #[test]
    fn challenge_status_rejects_pre_release_task_width_and_missing_field() {
        let legacy = LegacyPorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 99,
            status: PorChallengeOutcome::Failed,
            sample_count: 64,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: Some(1_700_000_100),
            proof_digest: None,
            repair_task_id: Some([4; 16]),
            failure_reason: Some("provider missed deadline".to_owned()),
            verifier_latency_ms: Some(1_500),
        };
        let legacy_bytes = norito::to_bytes(&legacy).expect("encode legacy status");
        assert!(
            norito::decode_from_bytes::<PorChallengeStatusV1>(&legacy_bytes).is_err(),
            "the pre-release 16-byte repair task form must not decode"
        );
        let missing = MissingRepairTaskFieldStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 99,
            status: PorChallengeOutcome::Failed,
            sample_count: 64,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: Some(1_700_000_100),
            proof_digest: None,
            failure_reason: Some("provider missed deadline".to_owned()),
            verifier_latency_ms: Some(1_500),
        };
        let missing_bytes = norito::to_bytes(&missing).expect("encode missing-field status");
        assert!(
            norito::decode_from_bytes::<PorChallengeStatusV1>(&missing_bytes).is_err(),
            "a status that omits the canonical repair-task field must not decode"
        );
    }
    #[test]
    fn challenge_status_validation_succeeds() {
        let status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 42,
            status: PorChallengeOutcome::Verified,
            sample_count: 32,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: Some(1_700_000_050),
            proof_digest: Some([4; 32]),
            repair_task_id: None,
            failure_reason: None,
            verifier_latency_ms: Some(950),
        };
        assert!(status.validate().is_ok());
    }
    #[test]
    fn challenge_status_bounds_and_decoder_fail_closed() {
        let mut status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 42,
            status: PorChallengeOutcome::Failed,
            sample_count: u16::try_from(POR_CHALLENGE_MAX_SAMPLES_V1)
                .expect("challenge sample ceiling fits u16"),
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: Some(1_700_000_050),
            proof_digest: Some([4; 32]),
            repair_task_id: Some([5; 32]),
            failure_reason: Some("x".repeat(POR_CHALLENGE_STATUS_FAILURE_REASON_MAX_BYTES_V1)),
            verifier_latency_ms: Some(950),
        };
        status
            .validate()
            .expect("exact status field boundaries validate");
        let exact = status
            .encoded_len_exact()
            .expect("status exposes exact canonical length");
        assert_eq!(
            preflight_por_challenge_status_len(&status, exact),
            Ok(exact)
        );
        assert_eq!(
            preflight_por_challenge_status_len(&status, exact - 1),
            Err(PorChallengeStatusValidationError::PayloadTooLarge {
                found: exact,
                maximum: exact - 1,
            })
        );
        let encoded = norito::to_bytes(&status).expect("encode bounded status");
        assert_eq!(
            decode_por_challenge_status_v1(&encoded).expect("bounded status decoder"),
            status
        );
        assert!(
            decode_por_challenge_status_v1(&vec![
                0;
                POR_CHALLENGE_STATUS_MAX_CANONICAL_BYTES_V1 + 1
            ])
            .is_err()
        );
        status
            .failure_reason
            .as_mut()
            .expect("failure reason")
            .push('x');
        assert_eq!(
            status.validate(),
            Err(PorChallengeStatusValidationError::InvalidFailureReason)
        );
        status.failure_reason = Some("failure".to_owned());
        status.sample_count += 1;
        assert_eq!(
            status.validate(),
            Err(PorChallengeStatusValidationError::TooManySamples {
                found: u16::try_from(POR_CHALLENGE_MAX_SAMPLES_V1 + 1)
                    .expect("sample ceiling plus one fits u16"),
                maximum: POR_CHALLENGE_MAX_SAMPLES_V1,
            })
        );
    }
    #[test]
    fn challenge_status_page_record_bound_is_exact() {
        let status = PorChallengeStatusV1 {
            version: POR_CHALLENGE_STATUS_VERSION_V1,
            challenge_id: [1; 32],
            manifest_digest: [2; 32],
            provider_id: [3; 32],
            epoch_id: 10,
            drand_round: 42,
            status: PorChallengeOutcome::Verified,
            sample_count: 32,
            forced: false,
            issued_at: 1_700_000_000,
            responded_at: Some(1_700_000_050),
            proof_digest: Some([4; 32]),
            repair_task_id: None,
            failure_reason: None,
            verifier_latency_ms: Some(950),
        };
        let exact_page = vec![status.clone(); POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1];
        let exact_bytes = norito::to_bytes(&exact_page).expect("encode exact status page");
        assert_eq!(
            decode_por_challenge_status_page_v1(
                &exact_bytes,
                POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
            )
            .expect("protocol-maximum status page"),
            exact_page
        );
        let oversized_page =
            vec![status; POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1.saturating_add(1)];
        let oversized_bytes =
            norito::to_bytes(&oversized_page).expect("encode oversized status page");
        assert!(
            decode_por_challenge_status_page_v1(
                &oversized_bytes,
                POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1,
            )
            .is_err()
        );
        assert!(
            decode_por_challenge_status_page_v1(&[], POR_CHALLENGE_STATUS_PAGE_MAX_RECORDS_V1 + 1,)
                .is_err()
        );
    }
    #[test]
    fn iso_week_validation_bounds() {
        let invalid_year = PorReportIsoWeek {
            year: 1999,
            week: 1,
        };
        assert!(matches!(
            invalid_year.validate(),
            Err(PorReportIsoWeekValidationError::InvalidYear { .. })
        ));
        let invalid_week = PorReportIsoWeek {
            year: 2025,
            week: 0,
        };
        assert!(matches!(
            invalid_week.validate(),
            Err(PorReportIsoWeekValidationError::InvalidWeek { .. })
        ));
        let valid = PorReportIsoWeek {
            year: 2025,
            week: 12,
        };
        assert!(valid.validate().is_ok());
        assert_eq!(valid.to_string(), "2025-W12");
    }
    #[test]
    fn provider_summary_validation_checks_counts() {
        let mut summary = PorProviderSummaryV1 {
            provider_id: [1; 32],
            manifest_count: 5,
            challenges: 10,
            successes: 9,
            failures: 1,
            forced: 0,
            success_rate_bps: 9_000,
            first_failure_at: None,
            last_success_latency_ms_p95: Some(1_100),
            repair_dispatched: true,
            pending_repairs: 1,
            ticket_id: Some("REP-123".into()),
        };
        assert!(summary.validate().is_ok());
        summary.success_rate_bps = 10_001;
        assert!(matches!(
            summary.validate(),
            Err(PorProviderSummaryValidationError::InvalidSuccessRateBps { .. })
        ));
        summary.success_rate_bps = 9_001;
        assert!(matches!(
            summary.validate(),
            Err(PorProviderSummaryValidationError::InconsistentSuccessRateBps { .. })
        ));
    }
    #[test]
    fn slashing_event_validation_checks_fields() {
        let event = PorSlashingEventV1 {
            provider_id: [1; 32],
            manifest_digest: [2; 32],
            penalty_xor: XorQuantity::try_from_micro(1_000_000)
                .expect("legacy micro-XOR value is representable"),
            verdict_cid: "ipfs://cid".into(),
            decided_at: 1_700_000_000,
        };
        assert!(event.validate().is_ok());
    }
    fn canonical_weekly_report() -> PorWeeklyReportV1 {
        let first = PorProviderSummaryV1 {
            provider_id: [1; 32],
            manifest_count: 2,
            challenges: 10,
            successes: 6,
            failures: 3,
            forced: 1,
            success_rate_bps: 6_000,
            first_failure_at: Some(1_700_000_100),
            last_success_latency_ms_p95: Some(1_200),
            repair_dispatched: true,
            pending_repairs: 1,
            ticket_id: None,
        };
        let second = PorProviderSummaryV1 {
            provider_id: [2; 32],
            manifest_count: 1,
            challenges: 10,
            successes: 7,
            failures: 2,
            forced: 1,
            success_rate_bps: 7_000,
            first_failure_at: Some(1_700_000_200),
            last_success_latency_ms_p95: Some(1_300),
            repair_dispatched: true,
            pending_repairs: 0,
            ticket_id: None,
        };
        PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle: PorReportIsoWeek {
                year: 2025,
                week: 12,
            },
            generated_at: 1_700_000_400,
            challenges_total: 20,
            challenges_verified: 13,
            challenges_failed: 5,
            forced_challenges: 2,
            repairs_enqueued: 2,
            repairs_completed: 1,
            mean_latency_ms: Some(800),
            p95_latency_ms: Some(1_400),
            slashing_events: Vec::new(),
            providers_missing_vrf: vec![[3; 32], [4; 32]],
            top_offenders: vec![first, second],
            notes: None,
        }
    }
    #[test]
    fn weekly_report_rejects_noncanonical_and_inconsistent_aggregates() {
        let report = canonical_weekly_report();
        assert!(report.validate().is_ok());
        let mut invalid = report.clone();
        invalid.providers_missing_vrf.reverse();
        assert!(matches!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::UnsortedMissingVrfProviders { .. })
        ));
        let mut invalid = report.clone();
        invalid.providers_missing_vrf[1] = invalid.providers_missing_vrf[0];
        assert!(matches!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::UnsortedMissingVrfProviders { .. })
        ));
        let mut invalid = report.clone();
        invalid.top_offenders.reverse();
        assert!(matches!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::UnsortedTopOffenders { .. })
        ));
        let mut invalid = report.clone();
        invalid.top_offenders[1].provider_id = invalid.top_offenders[0].provider_id;
        assert!(matches!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::DuplicateTopOffender { .. })
        ));
        let slashing_event = |decided_at| PorSlashingEventV1 {
            provider_id: [1; 32],
            manifest_digest: [2; 32],
            penalty_xor: XorQuantity::try_from_micro(1).expect("positive penalty"),
            verdict_cid: format!("verdict-{decided_at}"),
            decided_at,
        };
        let mut invalid = report.clone();
        invalid.slashing_events = vec![slashing_event(2), slashing_event(1)];
        assert_eq!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::UnsortedOrDuplicateSlashingEvent { index: 1 })
        );
        let mut invalid = report.clone();
        invalid.slashing_events = vec![slashing_event(1), slashing_event(1)];
        assert_eq!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::UnsortedOrDuplicateSlashingEvent { index: 1 })
        );
        let mut invalid = report.clone();
        invalid.p95_latency_ms = Some(799);
        assert_eq!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::InvalidLatencyOrder)
        );
        let mut invalid = report.clone();
        invalid.repairs_completed = invalid.repairs_enqueued + 1;
        assert_eq!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::InvalidRepairTotals)
        );
        let mut invalid = report;
        invalid.top_offenders = (1_u8..=11)
            .map(|id| PorProviderSummaryV1 {
                provider_id: [id; 32],
                manifest_count: 1,
                challenges: 1,
                successes: 0,
                failures: 1,
                forced: 0,
                success_rate_bps: 0,
                first_failure_at: Some(1),
                last_success_latency_ms_p95: None,
                repair_dispatched: true,
                pending_repairs: 0,
                ticket_id: None,
            })
            .collect();
        assert!(matches!(
            invalid.validate(),
            Err(PorWeeklyReportValidationError::TooManyTopOffenders { count: 11 })
        ));
    }
    #[test]
    fn weekly_report_bounds_accept_boundaries_and_reject_one_over() {
        let mut report = canonical_weekly_report();
        report.challenges_total = u32::try_from(POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1)
            .expect("missing-VRF bound fits u32");
        report.forced_challenges = report.challenges_total;
        report.providers_missing_vrf = (1_u32..=report.challenges_total)
            .map(|index| {
                let mut provider = [0_u8; 32];
                provider[..4].copy_from_slice(&index.to_be_bytes());
                provider
            })
            .collect();
        report.notes = Some("x".repeat(POR_WEEKLY_REPORT_NOTES_MAX_BYTES_V1));
        assert!(report.validate().is_ok());
        report.providers_missing_vrf.push([0xFF; 32]);
        assert_eq!(
            report.validate(),
            Err(PorWeeklyReportValidationError::TooManyMissingVrfProviders {
                found: POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1 + 1,
                maximum: POR_WEEKLY_REPORT_MAX_MISSING_VRF_PROVIDERS_V1,
            })
        );
        let mut notes_too_long = canonical_weekly_report();
        notes_too_long.notes = Some("x".repeat(POR_WEEKLY_REPORT_NOTES_MAX_BYTES_V1 + 1));
        assert_eq!(
            notes_too_long.validate(),
            Err(PorWeeklyReportValidationError::InvalidNotes)
        );
    }
    #[test]
    fn weekly_slashing_inventory_accepts_boundary_and_rejects_one_over() {
        let mut report = canonical_weekly_report();
        report.challenges_total = u32::try_from(POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1)
            .expect("slashing-event bound fits u32");
        report.challenges_verified = 0;
        report.challenges_failed = report.challenges_total;
        report.forced_challenges = 0;
        report.providers_missing_vrf.clear();
        report.slashing_events = (1..=POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1)
            .map(|index| PorSlashingEventV1 {
                provider_id: [1; 32],
                manifest_digest: [2; 32],
                penalty_xor: XorQuantity::try_from_micro(1).expect("positive penalty"),
                verdict_cid: "v".to_owned(),
                decided_at: u64::try_from(index).expect("slashing index fits u64"),
            })
            .collect();
        report
            .validate()
            .expect("exact slashing-event boundary validates");
        report
            .slashing_events
            .push(report.slashing_events[0].clone());
        assert_eq!(
            report.validate(),
            Err(PorWeeklyReportValidationError::TooManySlashingEvents {
                found: POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1 + 1,
                maximum: POR_WEEKLY_REPORT_MAX_SLASHING_EVENTS_V1,
            })
        );
    }
    #[test]
    fn weekly_nested_identifiers_enforce_exact_boundaries() {
        let mut summary = canonical_weekly_report().top_offenders.remove(0);
        summary.ticket_id = Some("t".repeat(POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1));
        assert!(summary.validate().is_ok());
        summary.ticket_id = Some("t".repeat(POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1 + 1));
        assert_eq!(
            summary.validate(),
            Err(PorProviderSummaryValidationError::InvalidTicketId)
        );
        let mut event = PorSlashingEventV1 {
            provider_id: [1; 32],
            manifest_digest: [2; 32],
            penalty_xor: XorQuantity::try_from_micro(1).expect("positive penalty"),
            verdict_cid: "v".repeat(POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1),
            decided_at: 1,
        };
        assert!(event.validate().is_ok());
        event.verdict_cid = "v".repeat(POR_WEEKLY_REPORT_IDENTIFIER_MAX_BYTES_V1 + 1);
        assert_eq!(
            event.validate(),
            Err(PorSlashingEventValidationError::InvalidVerdictCid)
        );
        summary.ticket_id = Some("ticket\nid".to_owned());
        assert_eq!(
            summary.validate(),
            Err(PorProviderSummaryValidationError::InvalidTicketId)
        );
        event.verdict_cid = "verdict\ncid".to_owned();
        assert_eq!(
            event.validate(),
            Err(PorSlashingEventValidationError::InvalidVerdictCid)
        );
    }
    #[test]
    fn weekly_report_size_preflight_accepts_boundary_and_rejects_one_over() {
        let report = canonical_weekly_report();
        let exact = report
            .encoded_len_exact()
            .expect("weekly report exposes an exact canonical length");
        assert_eq!(preflight_por_weekly_report_len(&report, exact), Ok(exact));
        assert_eq!(
            preflight_por_weekly_report_len(&report, exact.saturating_sub(1)),
            Err(PorWeeklyReportValidationError::PayloadTooLarge {
                found: exact,
                maximum: exact.saturating_sub(1),
            })
        );
        let encoded = norito::to_bytes(&report).expect("encode bounded weekly report");
        assert_eq!(
            decode_por_weekly_report_v1(&encoded).expect("decode bounded canonical weekly report"),
            report
        );
    }
    #[test]
    fn provider_summary_count_validation_does_not_overflow() {
        let summary = PorProviderSummaryV1 {
            provider_id: [1; 32],
            manifest_count: 1,
            challenges: u32::MAX,
            successes: u32::MAX,
            failures: 1,
            forced: 0,
            success_rate_bps: POR_SUCCESS_RATE_BPS_MAX,
            first_failure_at: None,
            last_success_latency_ms_p95: None,
            repair_dispatched: false,
            pending_repairs: 0,
            ticket_id: None,
        };
        assert_eq!(
            summary.validate(),
            Err(PorProviderSummaryValidationError::InconsistentCounts)
        );
    }
    #[test]
    fn provider_summary_forced_count_is_orthogonal_to_outcome() {
        let failed_forced = PorProviderSummaryV1 {
            provider_id: [1; 32],
            manifest_count: 1,
            challenges: 1,
            successes: 0,
            failures: 1,
            forced: 1,
            success_rate_bps: 0,
            first_failure_at: Some(1),
            last_success_latency_ms_p95: None,
            repair_dispatched: true,
            pending_repairs: 1,
            ticket_id: None,
        };
        assert!(failed_forced.validate().is_ok());
        let mut impossible = failed_forced;
        impossible.forced = 2;
        assert_eq!(
            impossible.validate(),
            Err(PorProviderSummaryValidationError::InconsistentCounts)
        );
    }
    #[test]
    fn weekly_report_validation_succeeds() {
        let provider_summary = PorProviderSummaryV1 {
            provider_id: [5; 32],
            manifest_count: 12,
            challenges: 96,
            successes: 94,
            failures: 2,
            forced: 0,
            success_rate_bps: 9_791,
            first_failure_at: Some(1_700_000_300),
            last_success_latency_ms_p95: Some(1_800),
            repair_dispatched: true,
            pending_repairs: 1,
            ticket_id: Some("REP-342".into()),
        };
        let slashing = PorSlashingEventV1 {
            provider_id: [6; 32],
            manifest_digest: [7; 32],
            penalty_xor: XorQuantity::try_from_micro(250_000_000)
                .expect("legacy micro-XOR value is representable"),
            verdict_cid: "ipfs://verdict".into(),
            decided_at: 1_700_000_200,
        };
        let report = PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle: PorReportIsoWeek {
                year: 2025,
                week: 12,
            },
            generated_at: 1_700_000_400,
            challenges_total: 128,
            challenges_verified: 120,
            challenges_failed: 8,
            forced_challenges: 2,
            repairs_enqueued: 4,
            repairs_completed: 3,
            mean_latency_ms: Some(820),
            p95_latency_ms: Some(1_950),
            slashing_events: vec![slashing],
            providers_missing_vrf: vec![[8; 32]],
            top_offenders: vec![provider_summary],
            notes: Some("All forced challenges recovered within SLA.".into()),
        };
        assert!(report.validate().is_ok());
    }
    #[test]
    fn weekly_report_invalid_totals() {
        let report = PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle: PorReportIsoWeek {
                year: 2025,
                week: 1,
            },
            generated_at: 1_700_000_000,
            challenges_total: 10,
            challenges_verified: 9,
            challenges_failed: 3,
            forced_challenges: 0,
            repairs_enqueued: 0,
            repairs_completed: 0,
            mean_latency_ms: None,
            p95_latency_ms: None,
            slashing_events: Vec::new(),
            providers_missing_vrf: Vec::new(),
            top_offenders: Vec::new(),
            notes: None,
        };
        let err = report.validate().expect_err("invalid totals rejected");
        assert_eq!(err, PorWeeklyReportValidationError::InvalidChallengeTotals);
    }
}
