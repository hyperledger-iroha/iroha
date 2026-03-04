//! Proof-of-Retrievability (PoR) challenge, proof, and audit verdict schemas.

use blake3::Hasher;
use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    CapacityMetadataEntry, XorAmount, chunker_registry,
    provider_advert::{AdvertSignature, SignatureAlgorithm},
};

const POR_CHALLENGE_SEED_DOMAIN: &[u8] = b"sorafs:por:seed:v1";
const POR_CHALLENGE_ID_DOMAIN: &[u8] = b"sorafs:por:id:v1";

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
/// Current manual challenge schema version.
pub const MANUAL_POR_CHALLENGE_VERSION_V1: u8 = 1;

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
    #[norito(default)]
    pub drand_signature: Vec<u8>,
    /// Provider VRF output for this manifest/epoch (optional when forced).
    #[norito(default)]
    pub vrf_output: Option<[u8; 32]>,
    /// Provider VRF proof bytes (optional when forced).
    #[norito(default)]
    pub vrf_proof: Option<Vec<u8>>,
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
        if self.drand_signature.is_empty() {
            return Err(PorChallengeValidationError::MissingDrandSignature);
        }
        match (&self.vrf_output, &self.vrf_proof, self.forced) {
            (Some(output), proof_opt, _) => {
                if output.iter().all(|&byte| byte == 0) {
                    return Err(PorChallengeValidationError::InvalidVrfOutput);
                }
                if proof_opt.as_ref().is_none_or(|proof| proof.is_empty()) {
                    return Err(PorChallengeValidationError::MissingVrfProof);
                }
            }
            (None, _, true) => {}
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
    #[error("drand signature must be present")]
    MissingDrandSignature,
    #[error("provider VRF output required unless challenge marked forced")]
    MissingVrfOutput,
    #[error("provider VRF output must be non-zero")]
    InvalidVrfOutput,
    #[error("provider VRF proof required when VRF output supplied")]
    MissingVrfProof,
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
    /// Provider signature over the canonical proof digest (`proof_digest`).
    pub signature: AdvertSignature,
    /// Unix timestamp (seconds) when the proof was submitted.
    pub submitted_at: u64,
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
        match self.signature.algorithm {
            SignatureAlgorithm::Ed25519 | SignatureAlgorithm::MultiSig => {
                if self.signature.public_key.is_empty() || self.signature.signature.is_empty() {
                    return Err(PorProofValidationError::InvalidSignature);
                }
            }
        }
        Ok(())
    }

    /// Computes the canonical digest of the proof payload (without signature).
    #[must_use]
    pub fn proof_digest(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(&self.challenge_id);
        hasher.update(&self.manifest_digest);
        for sample in &self.samples {
            hasher.update(&sample.sample_index.to_be_bytes());
            hasher.update(&sample.chunk_offset.to_be_bytes());
            hasher.update(&sample.chunk_size.to_be_bytes());
            hasher.update(&sample.chunk_digest);
            hasher.update(&sample.leaf_digest);
        }
        for node in &self.auth_path {
            hasher.update(node);
        }
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
    /// Auditor signatures attesting to the verdict.
    pub auditor_signatures: Vec<AdvertSignature>,
    /// Optional metadata entries for downstream systems.
    #[norito(default)]
    pub metadata: Vec<CapacityMetadataEntry>,
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
            if signature.public_key.is_empty() || signature.signature.is_empty() {
                return Err(AuditVerdictValidationError::InvalidSignature);
            }
        }
        for (index, entry) in self.metadata.iter().enumerate() {
            let key_trimmed = entry.key.trim();
            if key_trimmed.is_empty() {
                return Err(AuditVerdictValidationError::InvalidMetadata {
                    index,
                    reason: "metadata key must not be empty",
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
        }
        Ok(())
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

/// Manual challenge request submitted by auditors/governance.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct ManualPorChallengeV1 {
    /// Schema version (`MANUAL_POR_CHALLENGE_VERSION_V1`).
    pub version: u8,
    /// Target manifest digest.
    pub manifest_digest: [u8; 32],
    /// Target provider identifier.
    pub provider_id: [u8; 32],
    /// Optional explicit sample count override.
    #[norito(default)]
    pub requested_samples: Option<u16>,
    /// Optional deadline override (seconds since issue).
    #[norito(default)]
    pub requested_deadline_secs: Option<u32>,
    /// Human readable reason justifying the manual trigger.
    pub reason: String,
}

impl ManualPorChallengeV1 {
    /// Validates the manual challenge payload.
    pub fn validate(&self) -> Result<(), ManualPorChallengeValidationError> {
        if self.version != MANUAL_POR_CHALLENGE_VERSION_V1 {
            return Err(ManualPorChallengeValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(ManualPorChallengeValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(ManualPorChallengeValidationError::InvalidProviderId);
        }
        if let Some(samples) = self.requested_samples
            && samples == 0
        {
            return Err(ManualPorChallengeValidationError::InvalidSampleOverride);
        }
        if let Some(deadline) = self.requested_deadline_secs
            && deadline == 0
        {
            return Err(ManualPorChallengeValidationError::InvalidDeadlineOverride);
        }
        if self.reason.trim().is_empty() {
            return Err(ManualPorChallengeValidationError::MissingReason);
        }
        Ok(())
    }
}

/// Validation errors for [`ManualPorChallengeV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ManualPorChallengeValidationError {
    #[error("unsupported manual challenge version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("requested sample override must be non-zero")]
    InvalidSampleOverride,
    #[error("deadline override must be non-zero seconds")]
    InvalidDeadlineOverride,
    #[error("reason must not be empty")]
    MissingReason,
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
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq)]
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
    /// Success rate (0.0 - 1.0).
    #[norito(default)]
    pub success_rate: f64,
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
        if self.success_rate < 0.0 || self.success_rate > 1.0 {
            return Err(PorProviderSummaryValidationError::InvalidSuccessRate {
                rate: self.success_rate,
            });
        }
        if self.successes > self.challenges {
            return Err(PorProviderSummaryValidationError::InconsistentCounts);
        }
        if self.failures > self.challenges {
            return Err(PorProviderSummaryValidationError::InconsistentCounts);
        }
        if (self.successes + self.failures + self.forced) > self.challenges {
            return Err(PorProviderSummaryValidationError::InconsistentCounts);
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
#[derive(Debug, Error, Clone, Copy, PartialEq)]
pub enum PorProviderSummaryValidationError {
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    #[error("success rate must be within [0,1], got {rate}")]
    InvalidSuccessRate { rate: f64 },
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
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq)]
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
    pub mean_latency_ms: Option<f64>,
    /// Optional P95 latency (milliseconds) across verified challenges.
    #[norito(default)]
    pub p95_latency_ms: Option<f64>,
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
        for (index, provider) in self.top_offenders.iter().enumerate() {
            provider.validate().map_err(|err| {
                PorWeeklyReportValidationError::InvalidProviderSummary { index, source: err }
            })?;
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
#[derive(Debug, Error, Clone, Copy, PartialEq)]
pub enum PorWeeklyReportValidationError {
    #[error("unsupported weekly report version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("invalid ISO week specified")]
    InvalidIsoWeek(#[from] PorReportIsoWeekValidationError),
    #[error("generated_at timestamp must be non-zero")]
    InvalidGeneratedAt,
    #[error("challenge totals are inconsistent")]
    InvalidChallengeTotals,
    #[error("provider summary #{index} invalid: {source}")]
    InvalidProviderSummary {
        index: usize,
        source: PorProviderSummaryValidationError,
    },
    #[error("slashing event #{index} invalid: {source}")]
    InvalidSlashingEvent {
        index: usize,
        source: PorSlashingEventValidationError,
    },
    #[error("providers_missing_vrf entry #{index} must be non-zero")]
    InvalidMissingVrfProvider { index: usize },
    #[error("notes field must not be empty when present")]
    InvalidNotes,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            drand_signature: vec![0x66; 96],
            vrf_output: Some(vrf_output),
            vrf_proof: Some(vec![0x77; 80]),
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
            drand_signature: vec![0xCD; 96],
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
            drand_signature: Vec::new(),
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
            Err(PorChallengeValidationError::MissingDrandSignature)
        );
    }

    #[test]
    fn proof_validation_succeeds() {
        let proof = PorProofV1 {
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
        };
        assert!(proof.validate().is_ok());
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
    fn manual_challenge_requires_reason() {
        let manual = ManualPorChallengeV1 {
            version: MANUAL_POR_CHALLENGE_VERSION_V1,
            manifest_digest: [1; 32],
            provider_id: [2; 32],
            requested_samples: Some(16),
            requested_deadline_secs: Some(600),
            reason: String::new(),
        };
        let err = manual.validate().expect_err("empty reason rejected");
        assert_eq!(err, ManualPorChallengeValidationError::MissingReason);
    }

    #[test]
    fn manual_challenge_validation_succeeds() {
        let manual = ManualPorChallengeV1 {
            version: MANUAL_POR_CHALLENGE_VERSION_V1,
            manifest_digest: [1; 32],
            provider_id: [2; 32],
            requested_samples: Some(32),
            requested_deadline_secs: None,
            reason: "trigger due to latency regression".into(),
        };
        assert!(manual.validate().is_ok());
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
            success_rate: 0.9,
            first_failure_at: None,
            last_success_latency_ms_p95: Some(1_100),
            repair_dispatched: true,
            pending_repairs: 1,
            ticket_id: Some("REP-123".into()),
        };
        assert!(summary.validate().is_ok());

        summary.success_rate = 1.5;
        assert!(matches!(
            summary.validate(),
            Err(PorProviderSummaryValidationError::InvalidSuccessRate { .. })
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

    #[test]
    fn weekly_report_validation_succeeds() {
        let provider_summary = PorProviderSummaryV1 {
            provider_id: [5; 32],
            manifest_count: 12,
            challenges: 96,
            successes: 94,
            failures: 2,
            forced: 0,
            success_rate: 0.979,
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
            mean_latency_ms: Some(820.0),
            p95_latency_ms: Some(1_950.0),
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
