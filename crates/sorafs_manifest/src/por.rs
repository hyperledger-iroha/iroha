//! Proof-of-Retrievability (PoR) challenge, proof, and audit verdict schemas.

use std::collections::BTreeSet;

use blake3::Hasher;
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    CapacityMetadataEntry, XorAmount, chunker_registry,
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
        if self.version != POR_VRF_SUBMISSION_VERSION_V1 {
            return Err(ProviderVrfSubmissionValidationError::UnsupportedVersion {
                found: self.version,
            });
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
        if self.signature.algorithm != SignatureAlgorithm::Ed25519
            || self.signature.public_key.is_empty()
            || self.signature.signature.is_empty()
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
    /// The submission version is unsupported.
    #[error("unsupported provider VRF submission version {found}")]
    UnsupportedVersion { found: u8 },
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
    #[error("provider VRF submission signature must be Ed25519 and non-empty")]
    InvalidSignature,
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
        if self.sample_indices.len() != usize::from(self.sample_count) {
            return Err(PorChallengeValidationError::SampleCountMismatch {
                expected: self.sample_count,
                actual: self.sample_indices.len() as u16,
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
    #[error("unknown chunker profile handle: {handle}")]
    UnknownChunkerHandle { handle: String },
    #[error("sample tier must be non-zero")]
    InvalidSampleTier,
    #[error("challenge must contain at least one sample")]
    ZeroSampleCount,
    #[error("sample count mismatch (expected {expected}, actual {actual})")]
    SampleCountMismatch { expected: u16, actual: u16 },
    #[error("deadline {deadline_at} must be greater than issued_at {issued_at}")]
    InvalidDeadline { issued_at: u64, deadline_at: u64 },
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
        for sample in &self.samples {
            sample.validate()?;
        }
        if self.auth_path.is_empty() {
            return Err(PorProofValidationError::MissingAuthPath);
        }
        if self.submitted_at == 0 {
            return Err(PorProofValidationError::InvalidSubmittedAt);
        }
        if self.signature.algorithm != SignatureAlgorithm::Ed25519
            || self.signature.public_key.is_empty()
            || self.signature.public_key.iter().all(|byte| *byte == 0)
            || self.signature.signature.is_empty()
            || self.signature.signature.iter().all(|byte| *byte == 0)
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
        norito::to_bytes(&PorProofSigningPayloadV1::from(self))
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
    #[error("authentication path must not be empty")]
    MissingAuthPath,
    #[error("sample {sample_index} has invalid chunk size")]
    InvalidChunkSize { sample_index: u64 },
    #[error("sample {sample_index} has invalid chunk digest")]
    InvalidChunkDigest { sample_index: u64 },
    #[error("sample {sample_index} has invalid leaf digest")]
    InvalidLeafDigest { sample_index: u64 },
    #[error("signature must include algorithm-specific public key and signature bytes")]
    InvalidSignature,
    #[error("proof submitted_at must be non-zero")]
    InvalidSubmittedAt,
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
        for signature in &self.auditor_signatures {
            if signature.algorithm != SignatureAlgorithm::Ed25519
                || signature.public_key.is_empty()
                || signature.public_key.iter().all(|byte| *byte == 0)
                || signature.signature.is_empty()
                || signature.signature.iter().all(|byte| *byte == 0)
            {
                return Err(AuditVerdictValidationError::InvalidSignature);
            }
        }
        let mut metadata_keys = BTreeSet::new();
        for (index, entry) in self.metadata.iter().enumerate() {
            let key_trimmed = entry.key.trim();
            if key_trimmed.is_empty() {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata key must not be empty",
                });
            }
            if key_trimmed != entry.key {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata key must not contain surrounding whitespace",
                });
            }
            if !key_trimmed.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            }) {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata key must use [a-z0-9_.-]",
                });
            }
            if entry.value.trim().is_empty() {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata value must not be empty",
                });
            }
            if entry.value.trim() != entry.value {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata value must not contain surrounding whitespace",
                });
            }
            if !metadata_keys.insert(entry.key.as_str()) {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata keys must be unique",
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
        norito::to_bytes(&AuditVerdictSigningPayloadV1::from(self))
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
            if !signers.insert(signature.public_key.clone()) {
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
    #[error("failure reason must be absent for success outcomes")]
    UnexpectedFailureReason,
    #[error("at least one auditor signature is required")]
    MissingSignatures,
    #[error("auditor signature is missing key or signature bytes")]
    InvalidSignature,
    #[error("metadata entry {index} invalid: {reason}")]
    InvalidMetadata { index: usize, reason: &'static str },
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

/// Lifecycle states emitted by the PoR coordinator.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[norito(tag = "outcome")]
#[repr(u8)]
pub enum PorChallengeOutcome {
    /// Proof has not yet been submitted or verified.
    #[norito(rename = "pending")]
    Pending = 1,
    /// Proof verified successfully.
    #[norito(rename = "verified")]
    Verified = 2,
    /// Proof failed and awaits remediation.
    #[norito(rename = "failed")]
    Failed = 3,
    /// Proof recovered after repair.
    #[norito(rename = "repaired")]
    Repaired = 4,
    /// Challenge was forced due to missing VRF.
    #[norito(rename = "forced")]
    Forced = 5,
}

impl PorChallengeOutcome {
    /// Human-readable label for reporting.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::Repaired => "repaired",
            Self::Forced => "forced",
        }
    }

    /// Parses a label into an outcome.
    pub fn parse(label: &str) -> Result<Self, PorChallengeOutcomeParseError> {
        match label.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "verified" => Ok(Self::Verified),
            "failed" => Ok(Self::Failed),
            "repaired" => Ok(Self::Repaired),
            "forced" => Ok(Self::Forced),
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
            1 => Ok(Self::Pending),
            2 => Ok(Self::Verified),
            3 => Ok(Self::Failed),
            4 => Ok(Self::Repaired),
            5 => Ok(Self::Forced),
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
    /// Optional repair task identifier linked to the challenge.
    #[norito(default)]
    pub repair_task_id: Option<[u8; 16]>,
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
        if self.sample_count == 0 {
            return Err(PorChallengeStatusValidationError::InvalidSampleCount);
        }
        if let Some(responded_at) = self.responded_at
            && responded_at < self.issued_at
        {
            return Err(PorChallengeStatusValidationError::InvalidResponseTimestamp);
        }
        if self
            .failure_reason
            .as_ref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(PorChallengeStatusValidationError::InvalidFailureReason);
        } else if matches!(
            self.status,
            PorChallengeOutcome::Failed | PorChallengeOutcome::Repaired
        ) {
            return Err(PorChallengeStatusValidationError::MissingFailureReason);
        }
        Ok(())
    }
}

/// Validation errors for [`PorChallengeStatusV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorChallengeStatusValidationError {
    #[error("unsupported status version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("sample count must be greater than zero")]
    InvalidSampleCount,
    #[error("failure reason must be provided for failed/repaired outcomes")]
    MissingFailureReason,
    #[error("failure reason must not be empty")]
    InvalidFailureReason,
    #[error("responded_at must not precede issued_at")]
    InvalidResponseTimestamp,
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
    /// Number of forced challenges issued.
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
        if u64::from(self.successes) + u64::from(self.failures) + u64::from(self.forced)
            > u64::from(self.challenges)
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
        if self
            .ticket_id
            .as_ref()
            .is_some_and(|ticket| ticket.trim().is_empty())
        {
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
    #[error("ticket identifier must not be empty")]
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
    pub penalty_xor: XorAmount,
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
        if self.verdict_cid.trim().is_empty() {
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
    #[error("verdict CID must not be empty")]
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
    /// Slashing events recorded in the cycle.
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
        }
        for (index, provider) in self.providers_missing_vrf.iter().enumerate() {
            if provider.iter().all(|&byte| byte == 0) {
                return Err(PorWeeklyReportValidationError::InvalidMissingVrfProvider { index });
            }
            if index > 0 && self.providers_missing_vrf[index - 1] >= *provider {
                return Err(PorWeeklyReportValidationError::UnsortedMissingVrfProviders { index });
            }
        }
        if self
            .notes
            .as_ref()
            .is_some_and(|notes| notes.trim().is_empty())
        {
            return Err(PorWeeklyReportValidationError::InvalidNotes);
        }
        Ok(())
    }
}

/// Validation errors for [`PorWeeklyReportV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PorWeeklyReportValidationError {
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
    #[error("providers_missing_vrf entry #{index} must be non-zero")]
    InvalidMissingVrfProvider { index: usize },
    #[error("providers_missing_vrf entry #{index} is duplicate or not in canonical order")]
    UnsortedMissingVrfProviders { index: usize },
    #[error("notes field must not be empty when present")]
    InvalidNotes,
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::*;

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
            PorChallengeOutcome::Pending,
            PorChallengeOutcome::Verified,
            PorChallengeOutcome::Failed,
            PorChallengeOutcome::Repaired,
            PorChallengeOutcome::Forced,
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
            responded_at: Some(1_700_000_100),
            proof_digest: None,
            repair_task_id: None,
            failure_reason: None,
            verifier_latency_ms: Some(1_500),
        };
        let err = status
            .validate()
            .expect_err("missing failure reason rejected");
        assert_eq!(err, PorChallengeStatusValidationError::MissingFailureReason);
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
            penalty_xor: XorAmount::from_micro(1_000_000),
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
            penalty_xor: XorAmount::from_micro(250_000_000),
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
