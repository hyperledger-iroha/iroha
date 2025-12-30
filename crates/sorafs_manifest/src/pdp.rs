//! Proof-of-Data Possession (PDP) Norito payloads.

use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use super::{BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1};

/// PDP commitment schema version (v1).
pub const PDP_COMMITMENT_VERSION_V1: u8 = 1;
/// PDP challenge schema version (v1).
pub const PDP_CHALLENGE_VERSION_V1: u8 = 1;
/// PDP proof schema version (v1).
pub const PDP_PROOF_VERSION_V1: u8 = 1;

/// Supported hash algorithms for PDP commitments.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
#[repr(u8)]
#[norito(tag = "algorithm", content = "value")]
pub enum HashAlgorithmV1 {
    /// BLAKE3-256 commitment.
    Blake3_256 = 1,
}

impl HashAlgorithmV1 {
    /// Canonical lowercase label for display purposes.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blake3_256 => "blake3-256",
        }
    }

    /// Returns true if the algorithm is currently supported.
    #[must_use]
    pub fn is_supported(self) -> bool {
        matches!(self, Self::Blake3_256)
    }
}

/// PDP commitment metadata embedded alongside manifests.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpCommitmentV1 {
    /// Commitment schema version.
    pub version: u8,
    /// Manifest digest (BLAKE3-256) this commitment is bound to.
    pub manifest_digest: [u8; 32],
    /// Chunking profile used when generating the commitment.
    pub chunk_profile: ChunkingProfileV1,
    /// Root of the hot-leaf (4 KiB granularity) commitment tree.
    pub commitment_root_hot: [u8; 32],
    /// Root of the segment (256 KiB granularity) commitment tree.
    pub commitment_root_segment: [u8; 32],
    /// Hash algorithm used to derive the commitments.
    pub hash_algorithm: HashAlgorithmV1,
    /// Height of the hot-leaf tree (levels including root).
    pub hot_tree_height: u16,
    /// Height of the segment tree (levels including root).
    pub segment_tree_height: u16,
    /// Target number of samples per epoch.
    pub sample_window: u16,
    /// Unix timestamp (seconds) when the commitment was sealed.
    pub sealed_at: u64,
}

impl PdpCommitmentV1 {
    /// Validates structural invariants for the commitment.
    pub fn validate(&self) -> Result<(), PdpCommitmentValidationError> {
        if self.version != PDP_COMMITMENT_VERSION_V1 {
            return Err(PdpCommitmentValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.manifest_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpCommitmentValidationError::InvalidManifestDigest);
        }
        if self.commitment_root_hot.iter().all(|byte| *byte == 0) {
            return Err(PdpCommitmentValidationError::InvalidHotRoot);
        }
        if self.commitment_root_segment.iter().all(|byte| *byte == 0) {
            return Err(PdpCommitmentValidationError::InvalidSegmentRoot);
        }
        if !self.hash_algorithm.is_supported() {
            return Err(PdpCommitmentValidationError::UnsupportedHashAlgorithm {
                algorithm: self.hash_algorithm,
            });
        }
        if self.hot_tree_height == 0 {
            return Err(PdpCommitmentValidationError::InvalidHotTreeHeight);
        }
        if self.segment_tree_height == 0 {
            return Err(PdpCommitmentValidationError::InvalidSegmentTreeHeight);
        }
        if self.sample_window == 0 {
            return Err(PdpCommitmentValidationError::InvalidSampleWindow);
        }
        if self.sealed_at == 0 {
            return Err(PdpCommitmentValidationError::InvalidSealedAt);
        }
        if self.chunk_profile.multihash_code != BLAKE3_256_MULTIHASH_CODE
            && self.chunk_profile.multihash_code != 0
        {
            return Err(PdpCommitmentValidationError::UnsupportedProfileMultihash {
                multihash: self.chunk_profile.multihash_code,
            });
        }
        Ok(())
    }
}

/// Validation failures for [`PdpCommitmentV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PdpCommitmentValidationError {
    /// Unsupported commitment version encountered.
    #[error("unsupported commitment version {found}")]
    UnsupportedVersion { found: u8 },
    /// Manifest digest must not be all zeros.
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Hot tree root must not be all zeros.
    #[error("hot commitment root must be non-zero")]
    InvalidHotRoot,
    /// Segment tree root must not be all zeros.
    #[error("segment commitment root must be non-zero")]
    InvalidSegmentRoot,
    /// Hash algorithm is not recognised.
    #[error("unsupported hash algorithm {algorithm:?}")]
    UnsupportedHashAlgorithm { algorithm: HashAlgorithmV1 },
    /// Hot tree height must be greater than zero.
    #[error("hot tree height must be greater than zero")]
    InvalidHotTreeHeight,
    /// Segment tree height must be greater than zero.
    #[error("segment tree height must be greater than zero")]
    InvalidSegmentTreeHeight,
    /// Sample window must be greater than zero.
    #[error("sample window must be greater than zero")]
    InvalidSampleWindow,
    /// Sealed-at timestamp must be non-zero.
    #[error("sealed_at timestamp must be greater than zero")]
    InvalidSealedAt,
    /// Chunk profile advertised an unsupported multihash code.
    #[error("unsupported chunk profile multihash code {multihash}")]
    UnsupportedProfileMultihash { multihash: u64 },
}

/// PDP sample referencing a segment and associated hot leaves.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpSampleV1 {
    /// Segment index being sampled.
    pub segment_index: u32,
    /// Hot leaf indices (within the segment) requested for inclusion proofs.
    pub hot_leaf_indices: Vec<u32>,
    /// Expected segment hash derived from the commitment.
    pub segment_leaf_hash: [u8; 32],
}

impl PdpSampleV1 {
    fn validate(&self) -> Result<(), PdpChallengeValidationError> {
        if self.hot_leaf_indices.is_empty() {
            return Err(PdpChallengeValidationError::EmptyHotLeafSet {
                segment_index: self.segment_index,
            });
        }
        let mut dedupe = self.hot_leaf_indices.clone();
        dedupe.sort_unstable();
        dedupe.dedup();
        if dedupe.len() != self.hot_leaf_indices.len() {
            return Err(PdpChallengeValidationError::DuplicateHotLeafIndex {
                segment_index: self.segment_index,
            });
        }
        if self.segment_leaf_hash.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidSegmentDigest {
                segment_index: self.segment_index,
            });
        }
        Ok(())
    }
}

/// PDP challenge describing the sample set for an epoch.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpChallengeV1 {
    /// Challenge schema version.
    pub version: u8,
    /// Unique identifier for the challenge.
    pub challenge_id: [u8; 32],
    /// Manifest digest tied to the challenge.
    pub manifest_digest: [u8; 32],
    /// Provider being challenged.
    pub provider_id: [u8; 32],
    /// Chunking profile used for the manifest.
    pub chunk_profile: ChunkingProfileV1,
    /// Seed derived from randomness beacons.
    pub seed: [u8; 32],
    /// Epoch identifier for scheduling.
    pub epoch_id: u64,
    /// drand round tied to the challenge.
    pub drand_round: u64,
    /// Unix timestamp (milliseconds) when responses must be submitted.
    pub response_deadline_unix: u64,
    /// Samples requested for this challenge.
    pub samples: Vec<PdpSampleV1>,
}

impl PdpChallengeV1 {
    /// Validates the challenge payload.
    pub fn validate(&self) -> Result<(), PdpChallengeValidationError> {
        if self.version != PDP_CHALLENGE_VERSION_V1 {
            return Err(PdpChallengeValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.challenge_id.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidChallengeId);
        }
        if self.manifest_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidProviderId);
        }
        if self.seed.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidSeed);
        }
        if self.response_deadline_unix == 0 {
            return Err(PdpChallengeValidationError::InvalidDeadline);
        }
        if self.samples.is_empty() {
            return Err(PdpChallengeValidationError::EmptySampleSet);
        }
        for sample in &self.samples {
            sample.validate()?;
        }
        Ok(())
    }
}

/// Validation failures for [`PdpChallengeV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PdpChallengeValidationError {
    /// Unsupported challenge version.
    #[error("unsupported challenge version {found}")]
    UnsupportedVersion { found: u8 },
    /// Challenge identifier must be non-zero.
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    /// Manifest digest must be non-zero.
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Provider identifier must be non-zero.
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    /// Seed must be non-zero.
    #[error("seed must be non-zero")]
    InvalidSeed,
    /// Deadline must be non-zero.
    #[error("response deadline must be greater than zero")]
    InvalidDeadline,
    /// Challenge must include at least one sample.
    #[error("challenge must include at least one sample")]
    EmptySampleSet,
    /// A sample referenced an empty hot-leaf set.
    #[error("segment {segment_index} contains an empty hot-leaf set")]
    EmptyHotLeafSet { segment_index: u32 },
    /// A sample contained duplicate hot-leaf indices.
    #[error("segment {segment_index} contains duplicate hot leaf indices")]
    DuplicateHotLeafIndex { segment_index: u32 },
    /// Sample declared a zero-valued segment digest.
    #[error("segment {segment_index} digest must be non-zero")]
    InvalidSegmentDigest { segment_index: u32 },
}

/// Inclusion proof for a hot leaf inside a PDP segment.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpHotLeafProofV1 {
    /// Leaf index (within the segment) covered by this proof.
    pub leaf_index: u32,
    /// Leaf hash (BLAKE3-256) being proven.
    pub leaf_hash: [u8; 32],
    /// Merkle path (sibling hashes) from leaf to segment root.
    pub leaf_merkle_path: Vec<[u8; 32]>,
}

impl PdpHotLeafProofV1 {
    fn validate(&self) -> Result<(), PdpProofValidationError> {
        if self.leaf_hash.iter().all(|byte| *byte == 0) {
            return Err(PdpProofValidationError::InvalidLeafDigest {
                leaf_index: self.leaf_index,
            });
        }
        Ok(())
    }
}

/// Inclusion proof for a PDP segment.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpProofLeafV1 {
    /// Segment index being proven.
    pub segment_index: u32,
    /// Segment hash advertised in the challenge.
    pub segment_hash: [u8; 32],
    /// Merkle path (sibling hashes) from segment to segment-root commitment.
    pub segment_merkle_path: Vec<[u8; 32]>,
    /// Hot leaf proofs associated with this segment.
    pub hot_leaves: Vec<PdpHotLeafProofV1>,
}

impl PdpProofLeafV1 {
    fn validate(&self) -> Result<(), PdpProofValidationError> {
        if self.segment_hash.iter().all(|byte| *byte == 0) {
            return Err(PdpProofValidationError::InvalidSegmentDigest {
                segment_index: self.segment_index,
            });
        }
        if self.hot_leaves.is_empty() {
            return Err(PdpProofValidationError::MissingHotLeafProofs {
                segment_index: self.segment_index,
            });
        }
        for hot in &self.hot_leaves {
            hot.validate()?;
        }
        Ok(())
    }
}

/// Provider response to a PDP challenge.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpProofV1 {
    /// Proof schema version.
    pub version: u8,
    /// Challenge identifier being answered.
    pub challenge_id: [u8; 32],
    /// Manifest digest associated with the challenge.
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Epoch identifier.
    pub epoch_id: u64,
    /// Inclusion proofs for the requested segments.
    pub proof_leaves: Vec<PdpProofLeafV1>,
    /// Provider signature over the canonical proof bytes.
    pub signature: Vec<u8>,
    /// Unix timestamp (milliseconds) when the proof was issued.
    pub issued_at_unix: u64,
}

impl PdpProofV1 {
    /// Validates structural invariants for the proof.
    pub fn validate(&self) -> Result<(), PdpProofValidationError> {
        if self.version != PDP_PROOF_VERSION_V1 {
            return Err(PdpProofValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.challenge_id.iter().all(|byte| *byte == 0) {
            return Err(PdpProofValidationError::InvalidChallengeId);
        }
        if self.manifest_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpProofValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|byte| *byte == 0) {
            return Err(PdpProofValidationError::InvalidProviderId);
        }
        if self.issued_at_unix == 0 {
            return Err(PdpProofValidationError::InvalidIssuedAt);
        }
        if self.signature.is_empty() {
            return Err(PdpProofValidationError::MissingSignature);
        }
        if self.proof_leaves.is_empty() {
            return Err(PdpProofValidationError::EmptyProofSet);
        }
        for leaf in &self.proof_leaves {
            leaf.validate()?;
        }
        Ok(())
    }
}

/// Validation failures for [`PdpProofV1`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PdpProofValidationError {
    /// Unsupported proof version.
    #[error("unsupported proof version {found}")]
    UnsupportedVersion { found: u8 },
    /// Challenge id must be non-zero.
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    /// Manifest digest must be non-zero.
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Provider id must be non-zero.
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    /// Proof issued-at timestamp must be non-zero.
    #[error("issued_at timestamp must be greater than zero")]
    InvalidIssuedAt,
    /// Proof must include at least one segment witness.
    #[error("proof must include at least one segment witness")]
    EmptyProofSet,
    /// Segment digest must be non-zero.
    #[error("segment {segment_index} digest must be non-zero")]
    InvalidSegmentDigest { segment_index: u32 },
    /// Segment witness missing hot leaf proofs.
    #[error("segment {segment_index} is missing hot-leaf proofs")]
    MissingHotLeafProofs { segment_index: u32 },
    /// Hot leaf digest must be non-zero.
    #[error("leaf {leaf_index} digest must be non-zero")]
    InvalidLeafDigest { leaf_index: u32 },
    /// Proof signature missing.
    #[error("proof signature must be present")]
    MissingSignature,
}
