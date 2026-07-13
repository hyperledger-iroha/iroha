//! Proof-of-Data Possession (PDP) commitment, challenge, and proof primitives.
//!
//! V1 commits to a payload twice: once as a global tree of 4 KiB hot leaves,
//! and once as a global tree of 256 KiB segment commitments. Each segment
//! commitment contains its own hot-leaf Merkle root. A valid proof therefore
//! demonstrates possession of the challenged bytes against both advertised
//! roots rather than merely replaying precomputed leaf hashes.

use std::collections::BTreeSet;

use ed25519_dalek::{Signer as _, SigningKey};
use norito::derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use super::{BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, ProfileId};
use crate::AdmissionRecord;

mod merkle;

pub use merkle::{
    PdpMerklePathError, PdpMerkleReadError, PdpMerkleTreeBuilderV1, PdpMerkleTreeError,
    PdpMerkleTreeV1, estimated_heap_bytes,
};

use merkle::{
    fold_global_hot_path_v1, fold_segment_hot_path_v1, fold_segment_path_v1, hash_hot_leaf_v1,
    hash_segment_leaf_v1, merkle_path_depth, wrap_hot_root_v1, wrap_segment_root_v1,
};

/// PDP commitment schema version (v1).
pub const PDP_COMMITMENT_VERSION_V1: u8 = 1;
/// PDP challenge schema version (v1).
pub const PDP_CHALLENGE_VERSION_V1: u8 = 1;
/// PDP proof schema version (v1).
pub const PDP_PROOF_VERSION_V1: u8 = 1;
/// PDP Governance DAG archive schema version (v1).
pub const PDP_GOVERNANCE_ARCHIVE_VERSION_V1: u8 = 1;
/// Fixed hot-leaf granularity for PDP v1.
pub const PDP_HOT_LEAF_SIZE_V1: u32 = 4 * 1024;
/// Fixed segment granularity for PDP v1.
pub const PDP_SEGMENT_SIZE_V1: u32 = 256 * 1024;
/// Number of full hot leaves in one full segment.
pub const PDP_HOT_LEAVES_PER_SEGMENT_V1: u16 = (PDP_SEGMENT_SIZE_V1 / PDP_HOT_LEAF_SIZE_V1) as u16;
/// Maximum challenged segments in one v1 challenge/proof.
pub const PDP_MAX_SEGMENT_SAMPLES_V1: usize = 500;
/// Maximum hot leaves requested from one segment.
pub const PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1: usize = PDP_HOT_LEAVES_PER_SEGMENT_V1 as usize;
/// Maximum total hot-leaf witnesses in one v1 challenge/proof.
pub const PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1: usize = 1_024;
/// Maximum authentication-path depth accepted by v1.
pub const PDP_MAX_MERKLE_PATH_DEPTH_V1: usize = 64;
/// Maximum canonical commitment payload size used by decoders/reference tooling.
pub const PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1: usize = 16 * 1024;
/// Maximum canonical challenge payload size used by decoders/reference tooling.
pub const PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1: usize = 512 * 1024;
/// Maximum canonical proof payload size used by decoders/reference tooling.
pub const PDP_PROOF_MAX_CANONICAL_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum canonical PDP Governance DAG archive payload size.
pub const PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1: usize =
    PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1 + PDP_PROOF_MAX_CANONICAL_BYTES_V1 + 64 * 1024;
/// Maximum aliases in an embedded chunking profile.
pub const PDP_CHUNK_PROFILE_MAX_ALIASES_V1: usize = 16;
/// Maximum bytes in a chunk-profile namespace or name.
pub const PDP_CHUNK_PROFILE_COMPONENT_MAX_BYTES_V1: usize = 64;
/// Maximum bytes in a chunk-profile semantic version.
pub const PDP_CHUNK_PROFILE_SEMVER_MAX_BYTES_V1: usize = 32;
/// Maximum bytes in one chunk-profile alias.
pub const PDP_CHUNK_PROFILE_ALIAS_MAX_BYTES_V1: usize = 128;

const PDP_COMMITMENT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.pdp.commitment.digest.v1\0";
const PDP_CHALLENGE_ID_DOMAIN_V1: &[u8] = b"sorafs.pdp.challenge.id.v1\0";
const PDP_PROOF_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.pdp.proof.digest.v1\0";
const PDP_GOVERNANCE_ARCHIVE_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.pdp.governance-archive.v1\0";
/// Domain under which providers sign canonical PDP proof digests.
pub const PDP_PROOF_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.pdp.proof.signature.v1\0";

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
    /// Chunking profile used when ingesting the manifest.
    pub chunk_profile: ChunkingProfileV1,
    /// Total committed payload length.
    pub payload_len: u64,
    /// Fixed v1 hot-leaf size; must equal [`PDP_HOT_LEAF_SIZE_V1`].
    pub hot_leaf_size: u32,
    /// Fixed v1 segment size; must equal [`PDP_SEGMENT_SIZE_V1`].
    pub segment_size: u32,
    /// Exact number of global hot leaves.
    pub hot_leaf_count: u64,
    /// Exact number of global segments.
    pub segment_count: u64,
    /// Root of the global hot-leaf commitment tree.
    pub commitment_root_hot: [u8; 32],
    /// Root of the global segment commitment tree.
    pub commitment_root_segment: [u8; 32],
    /// Hash algorithm used to derive the commitments.
    pub hash_algorithm: HashAlgorithmV1,
    /// Height of the hot-leaf tree (levels including root).
    pub hot_tree_height: u16,
    /// Height of the segment tree (levels including root).
    pub segment_tree_height: u16,
    /// Maximum number of challenged segments in one proof.
    pub sample_window: u16,
    /// Unix timestamp (seconds) when the commitment was sealed.
    pub sealed_at: u64,
}

impl PdpCommitmentV1 {
    /// Construct a commitment from a canonical in-memory PDP tree.
    pub fn from_tree(
        tree: &PdpMerkleTreeV1,
        manifest_digest: [u8; 32],
        chunk_profile: ChunkingProfileV1,
        sample_window: u16,
        sealed_at: u64,
    ) -> Result<Self, PdpCommitmentValidationError> {
        let commitment = Self {
            version: PDP_COMMITMENT_VERSION_V1,
            manifest_digest,
            chunk_profile,
            payload_len: tree.payload_len(),
            hot_leaf_size: PDP_HOT_LEAF_SIZE_V1,
            segment_size: PDP_SEGMENT_SIZE_V1,
            hot_leaf_count: tree.hot_leaf_count(),
            segment_count: tree.segment_count(),
            commitment_root_hot: tree.hot_root(),
            commitment_root_segment: tree.segment_root(),
            hash_algorithm: HashAlgorithmV1::Blake3_256,
            hot_tree_height: tree.hot_tree_height(),
            segment_tree_height: tree.segment_tree_height(),
            sample_window,
            sealed_at,
        };
        commitment.validate()?;
        Ok(commitment)
    }

    /// Validates structural and exact-geometry invariants for the commitment.
    pub fn validate(&self) -> Result<(), PdpCommitmentValidationError> {
        if self.version != PDP_COMMITMENT_VERSION_V1 {
            return Err(PdpCommitmentValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.manifest_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpCommitmentValidationError::InvalidManifestDigest);
        }
        validate_chunk_profile_v1(&self.chunk_profile)
            .map_err(PdpCommitmentValidationError::InvalidChunkProfile)?;
        if self.payload_len == 0 {
            return Err(PdpCommitmentValidationError::InvalidPayloadLength);
        }
        if self.hot_leaf_size != PDP_HOT_LEAF_SIZE_V1 {
            return Err(PdpCommitmentValidationError::InvalidHotLeafSize {
                found: self.hot_leaf_size,
            });
        }
        if self.segment_size != PDP_SEGMENT_SIZE_V1 {
            return Err(PdpCommitmentValidationError::InvalidSegmentSize {
                found: self.segment_size,
            });
        }
        let expected_hot_leaf_count = div_ceil_u64(self.payload_len, self.hot_leaf_size.into())
            .ok_or(PdpCommitmentValidationError::GeometryOverflow)?;
        if self.hot_leaf_count != expected_hot_leaf_count {
            return Err(PdpCommitmentValidationError::HotLeafCountMismatch {
                expected: expected_hot_leaf_count,
                found: self.hot_leaf_count,
            });
        }
        let expected_segment_count = div_ceil_u64(self.payload_len, self.segment_size.into())
            .ok_or(PdpCommitmentValidationError::GeometryOverflow)?;
        if self.segment_count != expected_segment_count {
            return Err(PdpCommitmentValidationError::SegmentCountMismatch {
                expected: expected_segment_count,
                found: self.segment_count,
            });
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
        let expected_hot_height = tree_height(self.hot_leaf_count)?;
        if self.hot_tree_height != expected_hot_height {
            return Err(PdpCommitmentValidationError::HotTreeHeightMismatch {
                expected: expected_hot_height,
                found: self.hot_tree_height,
            });
        }
        let expected_segment_height = tree_height(self.segment_count)?;
        if self.segment_tree_height != expected_segment_height {
            return Err(PdpCommitmentValidationError::SegmentTreeHeightMismatch {
                expected: expected_segment_height,
                found: self.segment_tree_height,
            });
        }
        if self.sample_window == 0 || usize::from(self.sample_window) > PDP_MAX_SEGMENT_SAMPLES_V1 {
            return Err(PdpCommitmentValidationError::InvalidSampleWindow {
                found: self.sample_window,
            });
        }
        if self.sealed_at == 0 {
            return Err(PdpCommitmentValidationError::InvalidSealedAt);
        }
        ensure_canonical_size(self, PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1)
            .map_err(|_| PdpCommitmentValidationError::CanonicalEncoding)?;
        Ok(())
    }

    /// Compute the domain-separated digest that challenges bind to.
    pub fn commitment_digest(&self) -> Result<[u8; 32], norito::core::Error> {
        domain_separated_norito_digest(PDP_COMMITMENT_DIGEST_DOMAIN_V1, self)
    }
}

/// Validation failures for [`PdpCommitmentV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpCommitmentValidationError {
    /// Unsupported commitment version encountered.
    #[error("unsupported commitment version {found}")]
    UnsupportedVersion { found: u8 },
    /// Manifest digest must not be all zeros.
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Embedded chunk profile is not canonical and bounded.
    #[error("invalid PDP chunk profile: {0}")]
    InvalidChunkProfile(PdpChunkProfileValidationError),
    /// Payload length must be positive.
    #[error("PDP payload length must be greater than zero")]
    InvalidPayloadLength,
    /// Hot-leaf size differs from the fixed v1 granularity.
    #[error("PDP hot-leaf size must be {PDP_HOT_LEAF_SIZE_V1}, got {found}")]
    InvalidHotLeafSize { found: u32 },
    /// Segment size differs from the fixed v1 granularity.
    #[error("PDP segment size must be {PDP_SEGMENT_SIZE_V1}, got {found}")]
    InvalidSegmentSize { found: u32 },
    /// Hot-leaf count is not the exact count implied by payload geometry.
    #[error("PDP hot-leaf count must be {expected}, got {found}")]
    HotLeafCountMismatch { expected: u64, found: u64 },
    /// Segment count is not the exact count implied by payload geometry.
    #[error("PDP segment count must be {expected}, got {found}")]
    SegmentCountMismatch { expected: u64, found: u64 },
    /// Geometry arithmetic overflowed.
    #[error("PDP commitment geometry overflow")]
    GeometryOverflow,
    /// Hot tree root must not be all zeros.
    #[error("hot commitment root must be non-zero")]
    InvalidHotRoot,
    /// Segment tree root must not be all zeros.
    #[error("segment commitment root must be non-zero")]
    InvalidSegmentRoot,
    /// Hash algorithm is not recognised.
    #[error("unsupported hash algorithm {algorithm:?}")]
    UnsupportedHashAlgorithm { algorithm: HashAlgorithmV1 },
    /// Hot-tree height differs from the exact count-derived height.
    #[error("PDP hot-tree height must be {expected}, got {found}")]
    HotTreeHeightMismatch { expected: u16, found: u16 },
    /// Segment-tree height differs from the exact count-derived height.
    #[error("PDP segment-tree height must be {expected}, got {found}")]
    SegmentTreeHeightMismatch { expected: u16, found: u16 },
    /// Sample window must be within the protocol bound.
    #[error("PDP sample window {found} is outside 1..={PDP_MAX_SEGMENT_SAMPLES_V1}")]
    InvalidSampleWindow { found: u16 },
    /// Sealed-at timestamp must be non-zero.
    #[error("sealed_at timestamp must be greater than zero")]
    InvalidSealedAt,
    /// Canonical encoding failed or exceeded the commitment byte cap.
    #[error("PDP commitment canonical encoding is unavailable or over limit")]
    CanonicalEncoding,
}

/// PDP sample selecting one segment and segment-local hot leaves.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpSampleV1 {
    /// Global segment index being sampled.
    pub segment_index: u64,
    /// Strictly increasing hot-leaf indices within the segment.
    pub hot_leaf_indices: Vec<u16>,
}

impl PdpSampleV1 {
    fn validate(&self) -> Result<(), PdpChallengeValidationError> {
        if self.hot_leaf_indices.is_empty() {
            return Err(PdpChallengeValidationError::EmptyHotLeafSet {
                segment_index: self.segment_index,
            });
        }
        if self.hot_leaf_indices.len() > PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1 {
            return Err(PdpChallengeValidationError::TooManyHotLeaves {
                segment_index: self.segment_index,
                found: self.hot_leaf_indices.len(),
            });
        }
        let mut previous = None;
        for &leaf_index in &self.hot_leaf_indices {
            if leaf_index >= PDP_HOT_LEAVES_PER_SEGMENT_V1 {
                return Err(PdpChallengeValidationError::HotLeafIndexOutOfRange {
                    segment_index: self.segment_index,
                    leaf_index,
                });
            }
            if previous.is_some_and(|value| value >= leaf_index) {
                return Err(PdpChallengeValidationError::NonCanonicalHotLeafOrder {
                    segment_index: self.segment_index,
                });
            }
            previous = Some(leaf_index);
        }
        Ok(())
    }
}

/// PDP challenge describing the exact sample set for an epoch.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpChallengeV1 {
    /// Challenge schema version.
    pub version: u8,
    /// Identifier derived from every other canonical challenge field.
    pub challenge_id: [u8; 32],
    /// Domain-separated digest of the exact commitment being challenged.
    pub commitment_digest: [u8; 32],
    /// Manifest digest tied to the challenge.
    pub manifest_digest: [u8; 32],
    /// Provider being challenged.
    pub provider_id: [u8; 32],
    /// Chunking profile copied from the commitment.
    pub chunk_profile: ChunkingProfileV1,
    /// Seed supplied by the verified randomness corridor.
    pub seed: [u8; 32],
    /// Epoch identifier for scheduling.
    pub epoch_id: u64,
    /// Verified drand round tied to the challenge.
    pub drand_round: u64,
    /// Unix timestamp (seconds) when the challenge was issued.
    pub issued_at_unix: u64,
    /// Unix timestamp (seconds) when responses must be submitted.
    pub response_deadline_unix: u64,
    /// Strictly ordered samples requested for this challenge.
    pub samples: Vec<PdpSampleV1>,
}

#[derive(Debug, Clone, NoritoSerialize)]
struct PdpChallengeIdPayloadV1 {
    version: u8,
    commitment_digest: [u8; 32],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    chunk_profile: ChunkingProfileV1,
    seed: [u8; 32],
    epoch_id: u64,
    drand_round: u64,
    issued_at_unix: u64,
    response_deadline_unix: u64,
    samples: Vec<PdpSampleV1>,
}

impl From<&PdpChallengeV1> for PdpChallengeIdPayloadV1 {
    fn from(challenge: &PdpChallengeV1) -> Self {
        Self {
            version: challenge.version,
            commitment_digest: challenge.commitment_digest,
            manifest_digest: challenge.manifest_digest,
            provider_id: challenge.provider_id,
            chunk_profile: challenge.chunk_profile.clone(),
            seed: challenge.seed,
            epoch_id: challenge.epoch_id,
            drand_round: challenge.drand_round,
            issued_at_unix: challenge.issued_at_unix,
            response_deadline_unix: challenge.response_deadline_unix,
            samples: challenge.samples.clone(),
        }
    }
}

impl PdpChallengeV1 {
    /// Build a challenge and derive its identifier from the complete body.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        commitment_digest: [u8; 32],
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
        chunk_profile: ChunkingProfileV1,
        seed: [u8; 32],
        epoch_id: u64,
        drand_round: u64,
        issued_at_unix: u64,
        response_deadline_unix: u64,
        samples: Vec<PdpSampleV1>,
    ) -> Result<Self, PdpChallengeValidationError> {
        let mut challenge = Self {
            version: PDP_CHALLENGE_VERSION_V1,
            challenge_id: [0; 32],
            commitment_digest,
            manifest_digest,
            provider_id,
            chunk_profile,
            seed,
            epoch_id,
            drand_round,
            issued_at_unix,
            response_deadline_unix,
            samples,
        };
        challenge.challenge_id = challenge
            .derived_challenge_id()
            .map_err(|_| PdpChallengeValidationError::CanonicalEncoding)?;
        challenge.validate()?;
        Ok(challenge)
    }

    /// Derive the identifier bound to every canonical challenge field.
    pub fn derived_challenge_id(&self) -> Result<[u8; 32], norito::core::Error> {
        domain_separated_norito_digest(
            PDP_CHALLENGE_ID_DOMAIN_V1,
            &PdpChallengeIdPayloadV1::from(self),
        )
    }

    /// Validates the bounded canonical challenge payload.
    pub fn validate(&self) -> Result<(), PdpChallengeValidationError> {
        if self.version != PDP_CHALLENGE_VERSION_V1 {
            return Err(PdpChallengeValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.challenge_id.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidChallengeId);
        }
        if self.commitment_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidCommitmentDigest);
        }
        if self.manifest_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidManifestDigest);
        }
        if self.provider_id.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidProviderId);
        }
        validate_chunk_profile_v1(&self.chunk_profile)
            .map_err(PdpChallengeValidationError::InvalidChunkProfile)?;
        if self.seed.iter().all(|byte| *byte == 0) {
            return Err(PdpChallengeValidationError::InvalidSeed);
        }
        if self.epoch_id == 0 {
            return Err(PdpChallengeValidationError::InvalidEpoch);
        }
        if self.drand_round == 0 {
            return Err(PdpChallengeValidationError::InvalidDrandRound);
        }
        if self.issued_at_unix == 0 || self.response_deadline_unix <= self.issued_at_unix {
            return Err(PdpChallengeValidationError::InvalidDeadline {
                issued_at: self.issued_at_unix,
                deadline_at: self.response_deadline_unix,
            });
        }
        if self.samples.is_empty() {
            return Err(PdpChallengeValidationError::EmptySampleSet);
        }
        if self.samples.len() > PDP_MAX_SEGMENT_SAMPLES_V1 {
            return Err(PdpChallengeValidationError::TooManySegments {
                found: self.samples.len(),
            });
        }
        let mut previous_segment = None;
        let mut hot_leaf_total = 0usize;
        for sample in &self.samples {
            if previous_segment.is_some_and(|value| value >= sample.segment_index) {
                return Err(PdpChallengeValidationError::NonCanonicalSegmentOrder);
            }
            sample.validate()?;
            hot_leaf_total = hot_leaf_total
                .checked_add(sample.hot_leaf_indices.len())
                .ok_or(PdpChallengeValidationError::TooManyHotLeavesTotal { found: usize::MAX })?;
            previous_segment = Some(sample.segment_index);
        }
        if hot_leaf_total > PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1 {
            return Err(PdpChallengeValidationError::TooManyHotLeavesTotal {
                found: hot_leaf_total,
            });
        }
        let expected_id = self
            .derived_challenge_id()
            .map_err(|_| PdpChallengeValidationError::CanonicalEncoding)?;
        if expected_id != self.challenge_id {
            return Err(PdpChallengeValidationError::ChallengeIdMismatch);
        }
        ensure_canonical_size(self, PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1)
            .map_err(|_| PdpChallengeValidationError::CanonicalEncoding)?;
        Ok(())
    }
}

/// Validation failures for [`PdpChallengeV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpChallengeValidationError {
    /// Unsupported challenge version.
    #[error("unsupported challenge version {found}")]
    UnsupportedVersion { found: u8 },
    /// Challenge identifier must be non-zero.
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    /// Commitment digest must be non-zero.
    #[error("commitment digest must be non-zero")]
    InvalidCommitmentDigest,
    /// Manifest digest must be non-zero.
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Provider identifier must be non-zero.
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    /// Embedded chunk profile is invalid.
    #[error("invalid PDP challenge chunk profile: {0}")]
    InvalidChunkProfile(PdpChunkProfileValidationError),
    /// Seed must be non-zero.
    #[error("seed must be non-zero")]
    InvalidSeed,
    /// Epoch zero is reserved.
    #[error("epoch id must be non-zero")]
    InvalidEpoch,
    /// Drand round zero is invalid.
    #[error("drand round must be non-zero")]
    InvalidDrandRound,
    /// Deadline must follow a non-zero issued timestamp.
    #[error("response deadline {deadline_at} must be greater than issued_at {issued_at}")]
    InvalidDeadline { issued_at: u64, deadline_at: u64 },
    /// Challenge must include at least one sample.
    #[error("challenge must include at least one sample")]
    EmptySampleSet,
    /// Challenge exceeded the segment-sample cap.
    #[error("challenge has {found} segments; maximum is {PDP_MAX_SEGMENT_SAMPLES_V1}")]
    TooManySegments { found: usize },
    /// Challenge segment entries are not strictly increasing.
    #[error("challenge segment indices must be strictly increasing")]
    NonCanonicalSegmentOrder,
    /// A sample referenced an empty hot-leaf set.
    #[error("segment {segment_index} contains an empty hot-leaf set")]
    EmptyHotLeafSet { segment_index: u64 },
    /// A sample exceeded the per-segment hot-leaf cap.
    #[error(
        "segment {segment_index} has {found} hot leaves; maximum is {PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1}"
    )]
    TooManyHotLeaves { segment_index: u64, found: usize },
    /// A segment-local hot index exceeded fixed v1 geometry.
    #[error("segment {segment_index} hot leaf {leaf_index} is outside fixed v1 geometry")]
    HotLeafIndexOutOfRange { segment_index: u64, leaf_index: u16 },
    /// Segment-local hot indices are duplicated or unordered.
    #[error("segment {segment_index} hot leaf indices must be strictly increasing")]
    NonCanonicalHotLeafOrder { segment_index: u64 },
    /// Challenge exceeded the global hot-witness cap.
    #[error("challenge has {found} hot leaves; maximum is {PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1}")]
    TooManyHotLeavesTotal { found: usize },
    /// Challenge identifier does not bind its canonical body.
    #[error("challenge id does not match the canonical challenge body")]
    ChallengeIdMismatch,
    /// Canonical encoding failed or exceeded the challenge byte cap.
    #[error("PDP challenge canonical encoding is unavailable or over limit")]
    CanonicalEncoding,
}

/// Inclusion proof for one sampled hot leaf.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpHotLeafProofV1 {
    /// Segment-local hot-leaf index.
    pub leaf_index: u16,
    /// Absolute byte offset in the committed payload.
    pub leaf_offset: u64,
    /// Exact sampled byte length.
    pub leaf_length: u32,
    /// Sampled payload bytes; these are hashed by the verifier.
    pub leaf_bytes: Vec<u8>,
    /// Path from this leaf to its segment-local hot root.
    pub segment_hot_merkle_path: Vec<[u8; 32]>,
    /// Path from this leaf to the global hot root.
    pub global_hot_merkle_path: Vec<[u8; 32]>,
}

impl PdpHotLeafProofV1 {
    fn validate(&self, segment_index: u64) -> Result<(), PdpProofValidationError> {
        if self.leaf_index >= PDP_HOT_LEAVES_PER_SEGMENT_V1 {
            return Err(PdpProofValidationError::HotLeafIndexOutOfRange {
                segment_index,
                leaf_index: self.leaf_index,
            });
        }
        if self.leaf_length == 0 || self.leaf_length > PDP_HOT_LEAF_SIZE_V1 {
            return Err(PdpProofValidationError::InvalidLeafLength {
                segment_index,
                leaf_index: self.leaf_index,
                found: self.leaf_length,
            });
        }
        if self.leaf_bytes.len() != self.leaf_length as usize {
            return Err(PdpProofValidationError::LeafByteLengthMismatch {
                segment_index,
                leaf_index: self.leaf_index,
                expected: self.leaf_length,
                found: self.leaf_bytes.len(),
            });
        }
        for (kind, path) in [
            ("segment-hot", &self.segment_hot_merkle_path),
            ("global-hot", &self.global_hot_merkle_path),
        ] {
            if path.len() > PDP_MAX_MERKLE_PATH_DEPTH_V1 {
                return Err(PdpProofValidationError::MerklePathTooDeep {
                    kind,
                    segment_index,
                    leaf_index: Some(self.leaf_index),
                    found: path.len(),
                });
            }
        }
        Ok(())
    }
}

/// Inclusion proof for one challenged PDP segment.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpProofLeafV1 {
    /// Global segment index being proven.
    pub segment_index: u64,
    /// Absolute segment byte offset.
    pub segment_offset: u64,
    /// Exact segment byte length.
    pub segment_length: u32,
    /// Path from the segment commitment to the global segment root.
    pub segment_merkle_path: Vec<[u8; 32]>,
    /// Strictly ordered sampled hot leaves associated with this segment.
    pub hot_leaves: Vec<PdpHotLeafProofV1>,
}

impl PdpProofLeafV1 {
    fn validate(&self) -> Result<(), PdpProofValidationError> {
        if self.segment_length == 0 || self.segment_length > PDP_SEGMENT_SIZE_V1 {
            return Err(PdpProofValidationError::InvalidSegmentLength {
                segment_index: self.segment_index,
                found: self.segment_length,
            });
        }
        if self.segment_merkle_path.len() > PDP_MAX_MERKLE_PATH_DEPTH_V1 {
            return Err(PdpProofValidationError::MerklePathTooDeep {
                kind: "segment",
                segment_index: self.segment_index,
                leaf_index: None,
                found: self.segment_merkle_path.len(),
            });
        }
        if self.hot_leaves.is_empty() {
            return Err(PdpProofValidationError::MissingHotLeafProofs {
                segment_index: self.segment_index,
            });
        }
        if self.hot_leaves.len() > PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1 {
            return Err(PdpProofValidationError::TooManyHotLeafProofs {
                segment_index: self.segment_index,
                found: self.hot_leaves.len(),
            });
        }
        let expected_segment_offset = self
            .segment_index
            .checked_mul(u64::from(PDP_SEGMENT_SIZE_V1))
            .ok_or(PdpProofValidationError::GeometryOverflow)?;
        if self.segment_offset != expected_segment_offset {
            return Err(PdpProofValidationError::InvalidSegmentOffset {
                segment_index: self.segment_index,
                expected: expected_segment_offset,
                found: self.segment_offset,
            });
        }
        let mut previous_leaf = None;
        for hot in &self.hot_leaves {
            if previous_leaf.is_some_and(|value| value >= hot.leaf_index) {
                return Err(PdpProofValidationError::NonCanonicalHotLeafOrder {
                    segment_index: self.segment_index,
                });
            }
            hot.validate(self.segment_index)?;
            let expected_offset = self
                .segment_offset
                .checked_add(u64::from(hot.leaf_index) * u64::from(PDP_HOT_LEAF_SIZE_V1))
                .ok_or(PdpProofValidationError::GeometryOverflow)?;
            if hot.leaf_offset != expected_offset {
                return Err(PdpProofValidationError::InvalidLeafOffset {
                    segment_index: self.segment_index,
                    leaf_index: hot.leaf_index,
                    expected: expected_offset,
                    found: hot.leaf_offset,
                });
            }
            previous_leaf = Some(hot.leaf_index);
        }
        Ok(())
    }
}

/// Fixed-size Ed25519 signature attached to a PDP proof.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpEd25519SignatureV1 {
    /// Canonical strong Ed25519 public key.
    pub public_key: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
    /// Canonical Ed25519 signature bytes.
    pub signature: [u8; ed25519_dalek::SIGNATURE_LENGTH],
}

impl PdpEd25519SignatureV1 {
    fn validate(&self) -> Result<(), PdpSignatureVerificationError> {
        crate::checked_ed25519_verifying_key_from_bytes(&self.public_key)
            .map_err(|reason| PdpSignatureVerificationError::InvalidPublicKey { reason })?;
        crate::checked_ed25519_signature_from_bytes(&self.signature)
            .map_err(|reason| PdpSignatureVerificationError::InvalidSignature { reason })?;
        Ok(())
    }
}

/// Provider response to a PDP challenge.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpProofV1 {
    /// Proof schema version.
    pub version: u8,
    /// Commitment digest named by the challenge.
    pub commitment_digest: [u8; 32],
    /// Challenge identifier being answered.
    pub challenge_id: [u8; 32],
    /// Manifest digest associated with the challenge.
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Epoch identifier.
    pub epoch_id: u64,
    /// Inclusion proofs for the requested segments and sampled bytes.
    pub proof_leaves: Vec<PdpProofLeafV1>,
    /// Unix timestamp (seconds) when the proof was issued.
    pub issued_at_unix: u64,
    /// Provider Ed25519 signature over the canonical proof digest.
    pub signature: PdpEd25519SignatureV1,
}

#[derive(Debug, Clone, NoritoSerialize)]
struct PdpProofSigningPayloadV1 {
    version: u8,
    commitment_digest: [u8; 32],
    challenge_id: [u8; 32],
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    epoch_id: u64,
    proof_leaves: Vec<PdpProofLeafV1>,
    issued_at_unix: u64,
    signer_public_key: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
}

impl From<&PdpProofV1> for PdpProofSigningPayloadV1 {
    fn from(proof: &PdpProofV1) -> Self {
        Self {
            version: proof.version,
            commitment_digest: proof.commitment_digest,
            challenge_id: proof.challenge_id,
            manifest_digest: proof.manifest_digest,
            provider_id: proof.provider_id,
            epoch_id: proof.epoch_id,
            proof_leaves: proof.proof_leaves.clone(),
            issued_at_unix: proof.issued_at_unix,
            signer_public_key: proof.signature.public_key,
        }
    }
}

impl PdpProofV1 {
    /// Validates bounded proof structure and canonical signature material.
    pub fn validate(&self) -> Result<(), PdpProofValidationError> {
        self.validate_unsigned_fields()?;
        self.signature
            .validate()
            .map_err(PdpProofValidationError::InvalidSignature)?;
        ensure_canonical_size(self, PDP_PROOF_MAX_CANONICAL_BYTES_V1)
            .map_err(|_| PdpProofValidationError::CanonicalEncoding)?;
        Ok(())
    }

    fn validate_unsigned_fields(&self) -> Result<(), PdpProofValidationError> {
        if self.version != PDP_PROOF_VERSION_V1 {
            return Err(PdpProofValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.commitment_digest.iter().all(|byte| *byte == 0) {
            return Err(PdpProofValidationError::InvalidCommitmentDigest);
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
        if self.epoch_id == 0 {
            return Err(PdpProofValidationError::InvalidEpoch);
        }
        if self.issued_at_unix == 0 {
            return Err(PdpProofValidationError::InvalidIssuedAt);
        }
        if self.proof_leaves.is_empty() {
            return Err(PdpProofValidationError::EmptyProofSet);
        }
        if self.proof_leaves.len() > PDP_MAX_SEGMENT_SAMPLES_V1 {
            return Err(PdpProofValidationError::TooManySegments {
                found: self.proof_leaves.len(),
            });
        }
        let mut previous_segment = None;
        let mut hot_leaf_total = 0usize;
        for leaf in &self.proof_leaves {
            if previous_segment.is_some_and(|value| value >= leaf.segment_index) {
                return Err(PdpProofValidationError::NonCanonicalSegmentOrder);
            }
            leaf.validate()?;
            hot_leaf_total = hot_leaf_total
                .checked_add(leaf.hot_leaves.len())
                .ok_or(PdpProofValidationError::TooManyHotLeavesTotal { found: usize::MAX })?;
            previous_segment = Some(leaf.segment_index);
        }
        if hot_leaf_total > PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1 {
            return Err(PdpProofValidationError::TooManyHotLeavesTotal {
                found: hot_leaf_total,
            });
        }
        Ok(())
    }

    /// Compute the canonical domain-separated digest signed by the provider.
    pub fn proof_digest(&self) -> Result<[u8; 32], norito::core::Error> {
        domain_separated_norito_digest(
            PDP_PROOF_DIGEST_DOMAIN_V1,
            &PdpProofSigningPayloadV1::from(self),
        )
    }

    /// Verify the strict Ed25519 signature without establishing signer admission.
    pub fn verify_signature(&self) -> Result<(), PdpSignatureVerificationError> {
        self.signature.validate()?;
        let digest = self.proof_digest().map_err(|error| {
            PdpSignatureVerificationError::PayloadEncoding {
                reason: error.to_string(),
            }
        })?;
        let mut message = Vec::with_capacity(PDP_PROOF_SIGNATURE_DOMAIN_V1.len() + digest.len());
        message.extend_from_slice(PDP_PROOF_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&digest);
        let verifying_key =
            crate::checked_ed25519_verifying_key_from_bytes(&self.signature.public_key)
                .map_err(|reason| PdpSignatureVerificationError::InvalidPublicKey { reason })?;
        let signature = crate::checked_ed25519_signature_from_bytes(&self.signature.signature)
            .map_err(|reason| PdpSignatureVerificationError::InvalidSignature { reason })?;
        verifying_key
            .verify_strict(&message, &signature)
            .map_err(|error| PdpSignatureVerificationError::Verification {
                reason: error.to_string(),
            })
    }
}

/// Sign a complete unsigned PDP proof with a deterministic Ed25519 key.
pub fn sign_pdp_proof_ed25519_v1(
    mut proof: PdpProofV1,
    signing_key: &SigningKey,
) -> Result<PdpProofV1, PdpProofSigningError> {
    proof.validate_unsigned_fields()?;
    proof.signature.public_key = signing_key.verifying_key().to_bytes();
    proof.signature.signature = [0; ed25519_dalek::SIGNATURE_LENGTH];
    let digest = proof
        .proof_digest()
        .map_err(|error| PdpProofSigningError::PayloadEncoding {
            reason: error.to_string(),
        })?;
    let mut message = Vec::with_capacity(PDP_PROOF_SIGNATURE_DOMAIN_V1.len() + digest.len());
    message.extend_from_slice(PDP_PROOF_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&digest);
    proof.signature.signature = signing_key.sign(&message).to_bytes();
    proof.validate()?;
    Ok(proof)
}

/// Errors while producing a provider PDP signature.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpProofSigningError {
    /// Unsigned proof fields are invalid.
    #[error("invalid unsigned PDP proof: {0}")]
    Validation(#[from] PdpProofValidationError),
    /// Canonical signing payload encoding failed.
    #[error("failed to encode PDP signing payload: {reason}")]
    PayloadEncoding { reason: String },
}

/// Strict PDP signature verification errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpSignatureVerificationError {
    /// Public key is inert, noncanonical, or weak.
    #[error("invalid PDP Ed25519 public key: {reason}")]
    InvalidPublicKey { reason: String },
    /// Signature is inert, noncanonical, or weak.
    #[error("invalid PDP Ed25519 signature: {reason}")]
    InvalidSignature { reason: String },
    /// Canonical proof digest encoding failed.
    #[error("failed to encode PDP proof digest: {reason}")]
    PayloadEncoding { reason: String },
    /// Ed25519 verification failed.
    #[error("PDP Ed25519 signature verification failed: {reason}")]
    Verification { reason: String },
}

/// Validation errors for [`PdpProofV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpProofValidationError {
    /// Unsupported proof version.
    #[error("unsupported proof version {found}")]
    UnsupportedVersion { found: u8 },
    /// Commitment digest must be non-zero.
    #[error("commitment digest must be non-zero")]
    InvalidCommitmentDigest,
    /// Challenge id must be non-zero.
    #[error("challenge id must be non-zero")]
    InvalidChallengeId,
    /// Manifest digest must be non-zero.
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    /// Provider id must be non-zero.
    #[error("provider id must be non-zero")]
    InvalidProviderId,
    /// Epoch zero is reserved.
    #[error("epoch id must be non-zero")]
    InvalidEpoch,
    /// Proof issued-at timestamp must be non-zero.
    #[error("issued_at timestamp must be greater than zero")]
    InvalidIssuedAt,
    /// Proof must include at least one segment witness.
    #[error("proof must include at least one segment witness")]
    EmptyProofSet,
    /// Proof exceeded the segment cap.
    #[error("proof has {found} segments; maximum is {PDP_MAX_SEGMENT_SAMPLES_V1}")]
    TooManySegments { found: usize },
    /// Segment proofs are duplicated or unordered.
    #[error("proof segment indices must be strictly increasing")]
    NonCanonicalSegmentOrder,
    /// Segment length is invalid.
    #[error("segment {segment_index} has invalid length {found}")]
    InvalidSegmentLength { segment_index: u64, found: u32 },
    /// Segment offset is not canonical.
    #[error("segment {segment_index} offset must be {expected}, got {found}")]
    InvalidSegmentOffset {
        segment_index: u64,
        expected: u64,
        found: u64,
    },
    /// Segment witness missing hot-leaf proofs.
    #[error("segment {segment_index} is missing hot-leaf proofs")]
    MissingHotLeafProofs { segment_index: u64 },
    /// Segment exceeded the per-segment hot proof cap.
    #[error(
        "segment {segment_index} has {found} hot proofs; maximum is {PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1}"
    )]
    TooManyHotLeafProofs { segment_index: u64, found: usize },
    /// Segment-local hot proof index is invalid.
    #[error("segment {segment_index} hot leaf {leaf_index} is outside fixed v1 geometry")]
    HotLeafIndexOutOfRange { segment_index: u64, leaf_index: u16 },
    /// Segment-local hot proofs are duplicated or unordered.
    #[error("segment {segment_index} hot proofs must be strictly increasing")]
    NonCanonicalHotLeafOrder { segment_index: u64 },
    /// Hot-leaf length is invalid.
    #[error("segment {segment_index} hot leaf {leaf_index} has invalid length {found}")]
    InvalidLeafLength {
        segment_index: u64,
        leaf_index: u16,
        found: u32,
    },
    /// Sampled byte vector does not match the declared leaf length.
    #[error(
        "segment {segment_index} hot leaf {leaf_index} declares {expected} bytes but carries {found}"
    )]
    LeafByteLengthMismatch {
        segment_index: u64,
        leaf_index: u16,
        expected: u32,
        found: usize,
    },
    /// Hot-leaf offset is not canonical.
    #[error("segment {segment_index} hot leaf {leaf_index} offset must be {expected}, got {found}")]
    InvalidLeafOffset {
        segment_index: u64,
        leaf_index: u16,
        expected: u64,
        found: u64,
    },
    /// An authentication path exceeded the hard depth cap.
    #[error(
        "{kind} path for segment {segment_index} leaf {leaf_index:?} has depth {found}; maximum is {PDP_MAX_MERKLE_PATH_DEPTH_V1}"
    )]
    MerklePathTooDeep {
        kind: &'static str,
        segment_index: u64,
        leaf_index: Option<u16>,
        found: usize,
    },
    /// Proof exceeded the global hot witness cap.
    #[error("proof has {found} hot leaves; maximum is {PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1}")]
    TooManyHotLeavesTotal { found: usize },
    /// Geometry arithmetic overflowed.
    #[error("PDP proof geometry overflow")]
    GeometryOverflow,
    /// Fixed-size signature material is malformed.
    #[error("invalid PDP proof signature: {0}")]
    InvalidSignature(PdpSignatureVerificationError),
    /// Canonical encoding failed or exceeded the proof byte cap.
    #[error("PDP proof canonical encoding is unavailable or over limit")]
    CanonicalEncoding,
}

/// Stable rejection reason recorded for a terminal PDP challenge.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
#[norito(tag = "reason", content = "details")]
pub enum PdpRejectionReasonV1 {
    /// No proof arrived before the transport deadline.
    DeadlineExpired,
    /// A proof reached the service after the transport deadline.
    SubmissionLate,
    /// A signed proof claimed an issued-at time beyond governed clock skew.
    FutureTimestamp,
    /// An authenticated proof failed binding, coverage, or Merkle verification.
    InvalidProof,
    /// The provider admission disappeared while the challenge was pending.
    AdmissionRevoked,
    /// The active admission no longer authorised the challenge or provider key.
    AdmissionInactive,
    /// Safe local proof generation failed because retained storage was unavailable.
    StorageUnavailable,
}

/// Accepted or rejected terminal result for one PDP challenge.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
#[norito(tag = "decision", content = "details")]
pub enum PdpTerminalDecisionV1 {
    /// Exhaustive admission-bound verification succeeded.
    Accepted,
    /// The challenge failed with a stable repair category.
    Rejected(PdpRejectionReasonV1),
}

/// Canonical Governance DAG archive payload for one terminal PDP decision.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct PdpGovernanceArchiveV1 {
    /// Schema version.
    pub version: u8,
    /// Monotonic local provider-queue sequence.
    pub sequence: u64,
    /// Challenge identifier.
    pub challenge_id: [u8; 32],
    /// Commitment digest fixed by the challenge.
    pub commitment_digest: [u8; 32],
    /// Manifest digest fixed by the challenge.
    pub manifest_digest: [u8; 32],
    /// Governance provider identifier.
    pub provider_id: [u8; 32],
    /// Challenge epoch.
    pub epoch_id: u64,
    /// Terminal decision.
    pub decision: PdpTerminalDecisionV1,
    /// Canonical domain-separated proof digest when a proof was supplied.
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
    /// Number of challenged segments verified or failed.
    pub sampled_segments: u16,
    /// Number of challenged hot leaves verified or failed.
    pub sampled_hot_leaves: u16,
    /// Number of sampled payload bytes established by a successful proof.
    pub sampled_bytes: u64,
    /// Challenge issue timestamp.
    pub issued_at_unix: u64,
    /// Challenge response deadline.
    pub response_deadline_unix: u64,
    /// Server timestamp at terminal decision.
    pub decided_at_unix: u64,
    /// Exact council-verified admission envelope digest captured at enqueue.
    pub admission_envelope_digest: [u8; 32],
    /// Exact canonical challenge bytes.
    pub canonical_challenge: Vec<u8>,
    /// Exact canonical authenticated proof bytes, absent for no-submission failures.
    #[norito(default)]
    pub canonical_proof: Option<Vec<u8>>,
}

impl PdpGovernanceArchiveV1 {
    /// Validate the archive's bounded canonical payloads and terminal invariants.
    pub fn validate(&self) -> Result<(), PdpGovernanceArchiveValidationError> {
        if self.version != PDP_GOVERNANCE_ARCHIVE_VERSION_V1 {
            return Err(PdpGovernanceArchiveValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.sequence == 0
            || self.challenge_id == [0; 32]
            || self.commitment_digest == [0; 32]
            || self.manifest_digest == [0; 32]
            || self.provider_id == [0; 32]
            || self.epoch_id == 0
            || self.admission_envelope_digest == [0; 32]
        {
            return Err(PdpGovernanceArchiveValidationError::InvalidIdentity);
        }
        if self.issued_at_unix == 0
            || self.response_deadline_unix <= self.issued_at_unix
            || self.decided_at_unix < self.issued_at_unix
        {
            return Err(PdpGovernanceArchiveValidationError::InvalidTimeline);
        }
        if self.proof_digest.is_some() != self.canonical_proof.is_some() {
            return Err(PdpGovernanceArchiveValidationError::ProofPresenceMismatch);
        }
        match self.decision {
            PdpTerminalDecisionV1::Accepted => {
                if self.canonical_proof.is_none()
                    || self.sampled_bytes == 0
                    || self.decided_at_unix > self.response_deadline_unix
                {
                    return Err(PdpGovernanceArchiveValidationError::DecisionMismatch);
                }
            }
            PdpTerminalDecisionV1::Rejected(reason) => {
                let proof_required = matches!(
                    reason,
                    PdpRejectionReasonV1::SubmissionLate
                        | PdpRejectionReasonV1::FutureTimestamp
                        | PdpRejectionReasonV1::InvalidProof
                );
                if proof_required != self.canonical_proof.is_some() || self.sampled_bytes != 0 {
                    return Err(PdpGovernanceArchiveValidationError::DecisionMismatch);
                }
                if matches!(
                    reason,
                    PdpRejectionReasonV1::DeadlineExpired | PdpRejectionReasonV1::SubmissionLate
                ) && self.decided_at_unix <= self.response_deadline_unix
                {
                    return Err(PdpGovernanceArchiveValidationError::DecisionMismatch);
                }
            }
        }

        let challenge = decode_canonical_archive_challenge(&self.canonical_challenge)?;
        let sampled_segments = u16::try_from(challenge.samples.len())
            .map_err(|_| PdpGovernanceArchiveValidationError::SampleCountMismatch)?;
        let sampled_hot_leaves = challenge
            .samples
            .iter()
            .try_fold(0usize, |total, sample| {
                total.checked_add(sample.hot_leaf_indices.len())
            })
            .and_then(|total| u16::try_from(total).ok())
            .ok_or(PdpGovernanceArchiveValidationError::SampleCountMismatch)?;
        if challenge.challenge_id != self.challenge_id
            || challenge.commitment_digest != self.commitment_digest
            || challenge.manifest_digest != self.manifest_digest
            || challenge.provider_id != self.provider_id
            || challenge.epoch_id != self.epoch_id
            || challenge.issued_at_unix != self.issued_at_unix
            || challenge.response_deadline_unix != self.response_deadline_unix
        {
            return Err(PdpGovernanceArchiveValidationError::ChallengeBindingMismatch);
        }
        if sampled_segments == 0
            || sampled_hot_leaves == 0
            || sampled_segments != self.sampled_segments
            || sampled_hot_leaves != self.sampled_hot_leaves
        {
            return Err(PdpGovernanceArchiveValidationError::SampleCountMismatch);
        }

        if let Some(bytes) = self.canonical_proof.as_ref() {
            let proof = decode_canonical_archive_proof(bytes)?;
            proof.verify_signature().map_err(|error| {
                PdpGovernanceArchiveValidationError::InvalidProofSignature {
                    reason: error.to_string(),
                }
            })?;
            let digest = proof.proof_digest().map_err(|error| {
                PdpGovernanceArchiveValidationError::CanonicalEncoding {
                    reason: error.to_string(),
                }
            })?;
            if Some(digest) != self.proof_digest || proof.challenge_id != self.challenge_id {
                return Err(PdpGovernanceArchiveValidationError::ProofBindingMismatch);
            }
            if matches!(self.decision, PdpTerminalDecisionV1::Accepted)
                && (proof.commitment_digest != self.commitment_digest
                    || proof.manifest_digest != self.manifest_digest
                    || proof.provider_id != self.provider_id
                    || proof.epoch_id != self.epoch_id)
            {
                return Err(PdpGovernanceArchiveValidationError::ProofBindingMismatch);
            }
        }

        ensure_canonical_size(self, PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1).map_err(
            |error| PdpGovernanceArchiveValidationError::CanonicalEncoding {
                reason: error.to_string(),
            },
        )?;
        Ok(())
    }

    /// Return the canonical domain-separated archive digest.
    pub fn digest(&self) -> Result<[u8; 32], norito::core::Error> {
        domain_separated_norito_digest(PDP_GOVERNANCE_ARCHIVE_DIGEST_DOMAIN_V1, self)
    }
}

fn decode_canonical_archive_challenge(
    bytes: &[u8],
) -> Result<PdpChallengeV1, PdpGovernanceArchiveValidationError> {
    if bytes.is_empty() || bytes.len() > PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1 {
        return Err(PdpGovernanceArchiveValidationError::InvalidCanonicalChallenge);
    }
    let challenge = norito::decode_from_bytes::<PdpChallengeV1>(bytes)
        .map_err(|_| PdpGovernanceArchiveValidationError::InvalidCanonicalChallenge)?;
    challenge
        .validate()
        .map_err(|_| PdpGovernanceArchiveValidationError::InvalidCanonicalChallenge)?;
    let canonical = norito::to_bytes(&challenge)
        .map_err(|_| PdpGovernanceArchiveValidationError::InvalidCanonicalChallenge)?;
    if canonical != bytes {
        return Err(PdpGovernanceArchiveValidationError::InvalidCanonicalChallenge);
    }
    Ok(challenge)
}

fn decode_canonical_archive_proof(
    bytes: &[u8],
) -> Result<PdpProofV1, PdpGovernanceArchiveValidationError> {
    if bytes.is_empty() || bytes.len() > PDP_PROOF_MAX_CANONICAL_BYTES_V1 {
        return Err(PdpGovernanceArchiveValidationError::InvalidCanonicalProof);
    }
    let proof = norito::decode_from_bytes::<PdpProofV1>(bytes)
        .map_err(|_| PdpGovernanceArchiveValidationError::InvalidCanonicalProof)?;
    proof
        .validate()
        .map_err(|_| PdpGovernanceArchiveValidationError::InvalidCanonicalProof)?;
    let canonical = norito::to_bytes(&proof)
        .map_err(|_| PdpGovernanceArchiveValidationError::InvalidCanonicalProof)?;
    if canonical != bytes {
        return Err(PdpGovernanceArchiveValidationError::InvalidCanonicalProof);
    }
    Ok(proof)
}

/// Validation errors for [`PdpGovernanceArchiveV1`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpGovernanceArchiveValidationError {
    /// Archive schema version is unsupported.
    #[error("unsupported PDP governance archive version {found}")]
    UnsupportedVersion { found: u8 },
    /// Sequence or identity material is zero/inert.
    #[error("PDP governance archive identity fields must be non-zero")]
    InvalidIdentity,
    /// Challenge and terminal timestamps are inconsistent.
    #[error("PDP governance archive timeline is inconsistent")]
    InvalidTimeline,
    /// Proof digest and proof payload presence disagree.
    #[error("PDP governance archive proof digest/payload presence mismatch")]
    ProofPresenceMismatch,
    /// Terminal decision does not match proof presence, bytes, or deadline state.
    #[error("PDP governance archive terminal decision is inconsistent")]
    DecisionMismatch,
    /// Embedded challenge is malformed, noncanonical, or oversized.
    #[error("PDP governance archive challenge is not canonical")]
    InvalidCanonicalChallenge,
    /// Embedded proof is malformed, noncanonical, or oversized.
    #[error("PDP governance archive proof is not canonical")]
    InvalidCanonicalProof,
    /// Top-level archive fields disagree with the embedded challenge.
    #[error("PDP governance archive challenge binding mismatch")]
    ChallengeBindingMismatch,
    /// Declared sample counts disagree with the embedded challenge.
    #[error("PDP governance archive sample count mismatch")]
    SampleCountMismatch,
    /// Proof digest, challenge, or accepted identity binding disagrees.
    #[error("PDP governance archive proof binding mismatch")]
    ProofBindingMismatch,
    /// Embedded provider signature is invalid.
    #[error("invalid PDP governance archive proof signature: {reason}")]
    InvalidProofSignature { reason: String },
    /// Canonical archive encoding failed or exceeded its hard cap.
    #[error("PDP governance archive canonical encoding failed: {reason}")]
    CanonicalEncoding { reason: String },
}

/// Opaque result returned only after exhaustive PDP verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedPdpProofV1 {
    commitment_digest: [u8; 32],
    challenge_id: [u8; 32],
    proof_digest: [u8; 32],
    provider_id: [u8; 32],
    sampled_segments: u16,
    sampled_hot_leaves: u16,
    sampled_bytes: u64,
}

impl VerifiedPdpProofV1 {
    /// Commitment digest established by verification.
    #[must_use]
    pub fn commitment_digest(&self) -> &[u8; 32] {
        &self.commitment_digest
    }

    /// Challenge identifier established by verification.
    #[must_use]
    pub fn challenge_id(&self) -> &[u8; 32] {
        &self.challenge_id
    }

    /// Canonical signed proof digest.
    #[must_use]
    pub fn proof_digest(&self) -> &[u8; 32] {
        &self.proof_digest
    }

    /// Admitted provider identifier.
    #[must_use]
    pub fn provider_id(&self) -> &[u8; 32] {
        &self.provider_id
    }

    /// Number of segments proven.
    #[must_use]
    pub fn sampled_segments(&self) -> u16 {
        self.sampled_segments
    }

    /// Number of hot leaves proven.
    #[must_use]
    pub fn sampled_hot_leaves(&self) -> u16 {
        self.sampled_hot_leaves
    }

    /// Number of raw payload bytes proven.
    #[must_use]
    pub fn sampled_bytes(&self) -> u64 {
        self.sampled_bytes
    }
}

/// Failures emitted by the single production PDP verifier.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpVerificationError {
    /// Commitment structure or exact geometry is invalid.
    #[error("invalid PDP commitment: {0}")]
    Commitment(#[from] PdpCommitmentValidationError),
    /// Challenge structure, ordering, or identifier is invalid.
    #[error("invalid PDP challenge: {0}")]
    Challenge(#[from] PdpChallengeValidationError),
    /// Proof structure, ordering, geometry, or signature material is invalid.
    #[error("invalid PDP proof: {0}")]
    Proof(#[from] PdpProofValidationError),
    /// Commitment digest could not be encoded.
    #[error("failed to derive canonical PDP commitment digest: {reason}")]
    CommitmentDigestEncoding { reason: String },
    /// Proof digest could not be encoded.
    #[error("failed to derive canonical PDP proof digest: {reason}")]
    ProofDigestEncoding { reason: String },
    /// Challenge names a different commitment.
    #[error("PDP challenge commitment digest does not match commitment")]
    ChallengeCommitmentMismatch,
    /// Challenge manifest does not match the commitment.
    #[error("PDP challenge manifest digest does not match commitment")]
    ChallengeManifestMismatch,
    /// Challenge profile does not match the commitment.
    #[error("PDP challenge chunk profile does not match commitment")]
    ChallengeProfileMismatch,
    /// Challenge asks for more segments than the commitment permits.
    #[error("PDP challenge has {actual} segments but commitment permits {maximum}")]
    SampleWindowExceeded { maximum: u16, actual: usize },
    /// Commitment was sealed after challenge issuance.
    #[error("PDP commitment sealed at {sealed_at} after challenge issuance {issued_at}")]
    CommitmentSealedAfterChallenge { sealed_at: u64, issued_at: u64 },
    /// Proof references a different commitment.
    #[error("PDP proof commitment digest does not match challenge")]
    ProofCommitmentMismatch,
    /// Proof challenge id differs from the challenge.
    #[error("PDP proof challenge id does not match challenge")]
    ProofChallengeMismatch,
    /// Proof manifest differs from the challenge.
    #[error("PDP proof manifest digest does not match challenge")]
    ProofManifestMismatch,
    /// Proof provider differs from the challenge.
    #[error("PDP proof provider id does not match challenge")]
    ProofProviderMismatch,
    /// Proof epoch differs from the challenge.
    #[error("PDP proof epoch does not match challenge")]
    ProofEpochMismatch,
    /// Admission record names a different provider.
    #[error("PDP challenge provider is not the supplied admitted provider")]
    AdmissionProviderMismatch,
    /// Integrity-only admission records cannot authorize production proofs.
    #[error("PDP admission record was not verified under a council trust policy")]
    AdmissionNotCouncilVerified,
    /// Challenge predates the governance admission.
    #[error(
        "PDP challenge issued at {challenge_issued_at} predates admission issued at {admission_issued_at}"
    )]
    AdmissionNotActiveAtChallenge {
        admission_issued_at: u64,
        challenge_issued_at: u64,
    },
    /// Challenge window extends past the active admission window.
    #[error(
        "PDP challenge deadline {challenge_deadline} exceeds admission retention boundary {retention_epoch}"
    )]
    ChallengeBeyondAdmission {
        retention_epoch: u64,
        challenge_deadline: u64,
    },
    /// Proof was issued after the admission expired.
    #[error(
        "PDP proof issued at {proof_issued_at} exceeds admission retention boundary {retention_epoch}"
    )]
    AdmissionExpiredAtProof {
        retention_epoch: u64,
        proof_issued_at: u64,
    },
    /// Proof signing key differs from the admission-approved advert key.
    #[error("PDP proof signer does not match the admitted provider key")]
    AdmissionKeyMismatch,
    /// Proof was generated outside the exact challenge interval.
    #[error("PDP proof timestamp {proof_at} is outside [{issued_at}, {deadline_at}]")]
    ProofOutsideChallengeWindow {
        proof_at: u64,
        issued_at: u64,
        deadline_at: u64,
    },
    /// Segment proof count differs from the challenge.
    #[error("PDP proof has {actual} segment witnesses; challenge requires {expected}")]
    SegmentCoverageCountMismatch { expected: usize, actual: usize },
    /// Segment index differs at an ordered coverage position.
    #[error("PDP proof segment {actual} does not match challenged segment {expected}")]
    SegmentCoverageMismatch { expected: u64, actual: u64 },
    /// Hot-leaf proof count differs for a segment.
    #[error("PDP segment {segment_index} has {actual} hot proofs; challenge requires {expected}")]
    HotLeafCoverageCountMismatch {
        segment_index: u64,
        expected: usize,
        actual: usize,
    },
    /// Hot-leaf index differs at an ordered coverage position.
    #[error(
        "PDP segment {segment_index} proof leaf {actual} does not match challenged leaf {expected}"
    )]
    HotLeafCoverageMismatch {
        segment_index: u64,
        expected: u16,
        actual: u16,
    },
    /// Challenged segment is outside the committed payload.
    #[error("PDP challenged segment {segment_index} is outside segment count {segment_count}")]
    SegmentOutOfRange {
        segment_index: u64,
        segment_count: u64,
    },
    /// Challenged hot leaf is outside the selected segment.
    #[error(
        "PDP challenged hot leaf {leaf_index} is outside segment {segment_index} leaf count {leaf_count}"
    )]
    HotLeafOutOfRange {
        segment_index: u64,
        leaf_index: u16,
        leaf_count: u16,
    },
    /// Proof segment geometry differs from commitment-derived geometry.
    #[error("PDP segment {segment_index} geometry does not match the commitment")]
    SegmentGeometryMismatch { segment_index: u64 },
    /// Proof hot-leaf geometry differs from commitment-derived geometry.
    #[error(
        "PDP segment {segment_index} hot leaf {leaf_index} geometry does not match the commitment"
    )]
    HotLeafGeometryMismatch { segment_index: u64, leaf_index: u16 },
    /// Geometry arithmetic overflowed.
    #[error("PDP verification geometry overflow")]
    GeometryOverflow,
    /// Strict provider signature verification failed.
    #[error("PDP proof signature verification failed: {0}")]
    Signature(PdpSignatureVerificationError),
    /// Segment-local hot authentication path failed.
    #[error("PDP segment {segment_index} hot leaf {leaf_index} segment path failed: {source}")]
    SegmentHotPath {
        segment_index: u64,
        leaf_index: u16,
        source: PdpMerklePathError,
    },
    /// Global hot authentication path failed.
    #[error("PDP segment {segment_index} hot leaf {leaf_index} global path failed: {source}")]
    GlobalHotPath {
        segment_index: u64,
        leaf_index: u16,
        source: PdpMerklePathError,
    },
    /// Multiple leaves for one segment reconstructed different segment commitments.
    #[error("PDP segment {segment_index} hot proofs reconstruct different segment commitments")]
    InconsistentSegmentCommitment { segment_index: u64 },
    /// Segment authentication path failed.
    #[error("PDP segment {segment_index} path failed: {source}")]
    SegmentPath {
        segment_index: u64,
        source: PdpMerklePathError,
    },
    /// A sampled leaf did not authenticate to the commitment's hot root.
    #[error("PDP segment {segment_index} hot leaf {leaf_index} does not match the hot root")]
    HotRootMismatch { segment_index: u64, leaf_index: u16 },
    /// A sampled segment did not authenticate to the commitment's segment root.
    #[error("PDP segment {segment_index} does not match the segment root")]
    SegmentRootMismatch { segment_index: u64 },
    /// Verified coverage counters overflowed their bounded representation.
    #[error("PDP verified coverage counters overflowed")]
    CoverageOverflow,
}

/// Exhaustively verify the PDP witness bytes, signature, geometry, coverage,
/// and both Merkle roots without authorizing production acceptance.
///
/// This diagnostic verifier deliberately does not evaluate provider admission.
/// Its success only establishes that the three supplied payloads are internally
/// consistent. Call [`verify_pdp_bundle_v1`] with an active council-verified
/// [`AdmissionRecord`] before accepting a proof in production.
pub fn verify_pdp_witnesses_v1(
    commitment: &PdpCommitmentV1,
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
) -> Result<(), PdpVerificationError> {
    let commitment_digest = validate_pdp_binding_v1(commitment, challenge, proof)?;
    verify_pdp_witnesses_after_binding_v1(commitment, challenge, proof, commitment_digest)?;
    Ok(())
}

/// Exhaustively verify a PDP proof against its commitment, exact challenge,
/// and an admission record sourced from the active council-verified registry.
///
/// Structural validation or signature verification alone is never sufficient
/// for production admission. This function is the sole PDP acceptance path: it
/// binds both Merkle roots, sampled bytes and geometry, complete coverage,
/// timestamps, provider identity, and the admission-approved Ed25519 key.
pub fn verify_pdp_bundle_v1(
    commitment: &PdpCommitmentV1,
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
    admission: &AdmissionRecord,
) -> Result<VerifiedPdpProofV1, PdpVerificationError> {
    let commitment_digest = validate_pdp_binding_v1(commitment, challenge, proof)?;
    validate_pdp_admission_v1(challenge, proof, admission)?;
    verify_pdp_witnesses_after_binding_v1(commitment, challenge, proof, commitment_digest)
}

fn validate_pdp_binding_v1(
    commitment: &PdpCommitmentV1,
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
) -> Result<[u8; 32], PdpVerificationError> {
    commitment.validate()?;
    challenge.validate()?;
    proof.validate()?;

    let commitment_digest = commitment.commitment_digest().map_err(|error| {
        PdpVerificationError::CommitmentDigestEncoding {
            reason: error.to_string(),
        }
    })?;
    if challenge.commitment_digest != commitment_digest {
        return Err(PdpVerificationError::ChallengeCommitmentMismatch);
    }
    if challenge.manifest_digest != commitment.manifest_digest {
        return Err(PdpVerificationError::ChallengeManifestMismatch);
    }
    if challenge.chunk_profile != commitment.chunk_profile {
        return Err(PdpVerificationError::ChallengeProfileMismatch);
    }
    if challenge.samples.len() > usize::from(commitment.sample_window) {
        return Err(PdpVerificationError::SampleWindowExceeded {
            maximum: commitment.sample_window,
            actual: challenge.samples.len(),
        });
    }
    if commitment.sealed_at > challenge.issued_at_unix {
        return Err(PdpVerificationError::CommitmentSealedAfterChallenge {
            sealed_at: commitment.sealed_at,
            issued_at: challenge.issued_at_unix,
        });
    }
    if proof.commitment_digest != challenge.commitment_digest {
        return Err(PdpVerificationError::ProofCommitmentMismatch);
    }
    if proof.challenge_id != challenge.challenge_id {
        return Err(PdpVerificationError::ProofChallengeMismatch);
    }
    if proof.manifest_digest != challenge.manifest_digest {
        return Err(PdpVerificationError::ProofManifestMismatch);
    }
    if proof.provider_id != challenge.provider_id {
        return Err(PdpVerificationError::ProofProviderMismatch);
    }
    if proof.epoch_id != challenge.epoch_id {
        return Err(PdpVerificationError::ProofEpochMismatch);
    }
    Ok(commitment_digest)
}

fn validate_pdp_admission_v1(
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
    admission: &AdmissionRecord,
) -> Result<(), PdpVerificationError> {
    if !admission.is_council_verified() {
        return Err(PdpVerificationError::AdmissionNotCouncilVerified);
    }
    let admission_envelope = admission.envelope();
    if challenge.issued_at_unix < admission_envelope.issued_at {
        return Err(PdpVerificationError::AdmissionNotActiveAtChallenge {
            admission_issued_at: admission_envelope.issued_at,
            challenge_issued_at: challenge.issued_at_unix,
        });
    }
    if challenge.response_deadline_unix > admission_envelope.retention_epoch {
        return Err(PdpVerificationError::ChallengeBeyondAdmission {
            retention_epoch: admission_envelope.retention_epoch,
            challenge_deadline: challenge.response_deadline_unix,
        });
    }
    if proof.issued_at_unix > admission_envelope.retention_epoch {
        return Err(PdpVerificationError::AdmissionExpiredAtProof {
            retention_epoch: admission_envelope.retention_epoch,
            proof_issued_at: proof.issued_at_unix,
        });
    }
    if admission.provider_id() != &challenge.provider_id {
        return Err(PdpVerificationError::AdmissionProviderMismatch);
    }
    if admission.advert_key() != &proof.signature.public_key {
        return Err(PdpVerificationError::AdmissionKeyMismatch);
    }
    Ok(())
}

fn verify_pdp_witnesses_after_binding_v1(
    commitment: &PdpCommitmentV1,
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
    commitment_digest: [u8; 32],
) -> Result<VerifiedPdpProofV1, PdpVerificationError> {
    if proof.issued_at_unix < challenge.issued_at_unix
        || proof.issued_at_unix > challenge.response_deadline_unix
    {
        return Err(PdpVerificationError::ProofOutsideChallengeWindow {
            proof_at: proof.issued_at_unix,
            issued_at: challenge.issued_at_unix,
            deadline_at: challenge.response_deadline_unix,
        });
    }
    validate_exact_coverage(challenge, proof)?;
    proof
        .verify_signature()
        .map_err(PdpVerificationError::Signature)?;

    let mut sampled_bytes = 0u64;
    let mut sampled_hot_leaves = 0usize;
    for (sample, segment_proof) in challenge.samples.iter().zip(&proof.proof_leaves) {
        if sample.segment_index >= commitment.segment_count {
            return Err(PdpVerificationError::SegmentOutOfRange {
                segment_index: sample.segment_index,
                segment_count: commitment.segment_count,
            });
        }
        let segment_offset = sample
            .segment_index
            .checked_mul(u64::from(PDP_SEGMENT_SIZE_V1))
            .ok_or(PdpVerificationError::GeometryOverflow)?;
        let remaining = commitment
            .payload_len
            .checked_sub(segment_offset)
            .ok_or(PdpVerificationError::GeometryOverflow)?;
        let segment_length = u32::try_from(remaining.min(u64::from(PDP_SEGMENT_SIZE_V1)))
            .map_err(|_| PdpVerificationError::GeometryOverflow)?;
        if segment_proof.segment_offset != segment_offset
            || segment_proof.segment_length != segment_length
        {
            return Err(PdpVerificationError::SegmentGeometryMismatch {
                segment_index: sample.segment_index,
            });
        }
        let segment_hot_leaf_count_u64 =
            div_ceil_u64(u64::from(segment_length), u64::from(PDP_HOT_LEAF_SIZE_V1))
                .ok_or(PdpVerificationError::GeometryOverflow)?;
        let segment_hot_leaf_count = u16::try_from(segment_hot_leaf_count_u64)
            .map_err(|_| PdpVerificationError::GeometryOverflow)?;
        let global_hot_start = sample
            .segment_index
            .checked_mul(u64::from(PDP_HOT_LEAVES_PER_SEGMENT_V1))
            .ok_or(PdpVerificationError::GeometryOverflow)?;
        let mut established_segment_commitment = None;

        for hot in &segment_proof.hot_leaves {
            if hot.leaf_index >= segment_hot_leaf_count {
                return Err(PdpVerificationError::HotLeafOutOfRange {
                    segment_index: sample.segment_index,
                    leaf_index: hot.leaf_index,
                    leaf_count: segment_hot_leaf_count,
                });
            }
            let expected_leaf_offset = segment_offset
                .checked_add(u64::from(hot.leaf_index) * u64::from(PDP_HOT_LEAF_SIZE_V1))
                .ok_or(PdpVerificationError::GeometryOverflow)?;
            let segment_end = segment_offset
                .checked_add(u64::from(segment_length))
                .ok_or(PdpVerificationError::GeometryOverflow)?;
            let expected_leaf_length = u32::try_from(
                segment_end
                    .checked_sub(expected_leaf_offset)
                    .ok_or(PdpVerificationError::GeometryOverflow)?
                    .min(u64::from(PDP_HOT_LEAF_SIZE_V1)),
            )
            .map_err(|_| PdpVerificationError::GeometryOverflow)?;
            if hot.leaf_offset != expected_leaf_offset
                || hot.leaf_length != expected_leaf_length
                || hot.leaf_bytes.len() != expected_leaf_length as usize
            {
                return Err(PdpVerificationError::HotLeafGeometryMismatch {
                    segment_index: sample.segment_index,
                    leaf_index: hot.leaf_index,
                });
            }
            let global_hot_index = global_hot_start
                .checked_add(u64::from(hot.leaf_index))
                .ok_or(PdpVerificationError::GeometryOverflow)?;
            if global_hot_index >= commitment.hot_leaf_count {
                return Err(PdpVerificationError::HotLeafOutOfRange {
                    segment_index: sample.segment_index,
                    leaf_index: hot.leaf_index,
                    leaf_count: segment_hot_leaf_count,
                });
            }
            let leaf_digest = hash_hot_leaf_v1(
                global_hot_index,
                sample.segment_index,
                hot.leaf_index,
                hot.leaf_offset,
                hot.leaf_length,
                &hot.leaf_bytes,
            );
            let segment_hot_top = fold_segment_hot_path_v1(
                u64::from(hot.leaf_index),
                segment_hot_leaf_count_u64,
                leaf_digest,
                &hot.segment_hot_merkle_path,
            )
            .map_err(|source| PdpVerificationError::SegmentHotPath {
                segment_index: sample.segment_index,
                leaf_index: hot.leaf_index,
                source,
            })?;
            let segment_commitment = hash_segment_leaf_v1(
                sample.segment_index,
                segment_offset,
                segment_length,
                segment_hot_leaf_count,
                &segment_hot_top,
            );
            if established_segment_commitment
                .is_some_and(|established| established != segment_commitment)
            {
                return Err(PdpVerificationError::InconsistentSegmentCommitment {
                    segment_index: sample.segment_index,
                });
            }
            established_segment_commitment = Some(segment_commitment);

            let hot_top = fold_global_hot_path_v1(
                global_hot_index,
                commitment.hot_leaf_count,
                leaf_digest,
                &hot.global_hot_merkle_path,
            )
            .map_err(|source| PdpVerificationError::GlobalHotPath {
                segment_index: sample.segment_index,
                leaf_index: hot.leaf_index,
                source,
            })?;
            let hot_root =
                wrap_hot_root_v1(commitment.payload_len, commitment.hot_leaf_count, &hot_top);
            if hot_root != commitment.commitment_root_hot {
                return Err(PdpVerificationError::HotRootMismatch {
                    segment_index: sample.segment_index,
                    leaf_index: hot.leaf_index,
                });
            }
            sampled_bytes = sampled_bytes
                .checked_add(u64::from(hot.leaf_length))
                .ok_or(PdpVerificationError::CoverageOverflow)?;
            sampled_hot_leaves = sampled_hot_leaves
                .checked_add(1)
                .ok_or(PdpVerificationError::CoverageOverflow)?;
        }

        let segment_commitment = established_segment_commitment.ok_or(
            PdpVerificationError::HotLeafCoverageCountMismatch {
                segment_index: sample.segment_index,
                expected: sample.hot_leaf_indices.len(),
                actual: 0,
            },
        )?;
        let segment_top = fold_segment_path_v1(
            sample.segment_index,
            commitment.segment_count,
            segment_commitment,
            &segment_proof.segment_merkle_path,
        )
        .map_err(|source| PdpVerificationError::SegmentPath {
            segment_index: sample.segment_index,
            source,
        })?;
        let segment_root = wrap_segment_root_v1(
            commitment.payload_len,
            commitment.segment_count,
            &segment_top,
        );
        if segment_root != commitment.commitment_root_segment {
            return Err(PdpVerificationError::SegmentRootMismatch {
                segment_index: sample.segment_index,
            });
        }
    }

    let proof_digest =
        proof
            .proof_digest()
            .map_err(|error| PdpVerificationError::ProofDigestEncoding {
                reason: error.to_string(),
            })?;
    Ok(VerifiedPdpProofV1 {
        commitment_digest,
        challenge_id: challenge.challenge_id,
        proof_digest,
        provider_id: challenge.provider_id,
        sampled_segments: u16::try_from(challenge.samples.len())
            .map_err(|_| PdpVerificationError::CoverageOverflow)?,
        sampled_hot_leaves: u16::try_from(sampled_hot_leaves)
            .map_err(|_| PdpVerificationError::CoverageOverflow)?,
        sampled_bytes,
    })
}

fn validate_exact_coverage(
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
) -> Result<(), PdpVerificationError> {
    if challenge.samples.len() != proof.proof_leaves.len() {
        return Err(PdpVerificationError::SegmentCoverageCountMismatch {
            expected: challenge.samples.len(),
            actual: proof.proof_leaves.len(),
        });
    }
    for (sample, segment) in challenge.samples.iter().zip(&proof.proof_leaves) {
        if sample.segment_index != segment.segment_index {
            return Err(PdpVerificationError::SegmentCoverageMismatch {
                expected: sample.segment_index,
                actual: segment.segment_index,
            });
        }
        if sample.hot_leaf_indices.len() != segment.hot_leaves.len() {
            return Err(PdpVerificationError::HotLeafCoverageCountMismatch {
                segment_index: sample.segment_index,
                expected: sample.hot_leaf_indices.len(),
                actual: segment.hot_leaves.len(),
            });
        }
        for (&expected, actual) in sample.hot_leaf_indices.iter().zip(&segment.hot_leaves) {
            if expected != actual.leaf_index {
                return Err(PdpVerificationError::HotLeafCoverageMismatch {
                    segment_index: sample.segment_index,
                    expected,
                    actual: actual.leaf_index,
                });
            }
        }
    }
    Ok(())
}

/// Bounded canonical chunk-profile failures shared by commitment/challenge validation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpChunkProfileValidationError {
    /// A string field is empty, padded, non-ASCII, or too long.
    #[error("chunk profile field {field} is not a bounded canonical ASCII value")]
    InvalidText { field: &'static str },
    /// Alias inventory is empty or exceeds its cap.
    #[error("chunk profile alias count {found} is outside 1..={PDP_CHUNK_PROFILE_MAX_ALIASES_V1}")]
    InvalidAliasCount { found: usize },
    /// Alias is empty, padded, non-ASCII, or too long.
    #[error("chunk profile alias at index {index} is invalid")]
    InvalidAlias { index: usize },
    /// Alias inventory contains a duplicate.
    #[error("chunk profile contains duplicate alias {alias}")]
    DuplicateAlias { alias: String },
    /// Chunk sizes are zero or not monotonically ordered.
    #[error("chunk profile sizes must satisfy 0 < min <= target <= max")]
    InvalidSizes,
    /// A zero content-defined chunking mask can never select a boundary.
    #[error("chunk profile break mask must be non-zero")]
    InvalidBreakMask,
    /// PDP v1 requires BLAKE3-256 chunk digests.
    #[error("chunk profile multihash must be BLAKE3-256")]
    InvalidMultihash,
    /// Non-inline profile identifier is not registered.
    #[error("chunk profile id {profile_id} is not registered")]
    UnknownProfile { profile_id: u32 },
    /// Registered profile fields do not match the registry descriptor.
    #[error("chunk profile field {field} does not match its registry descriptor")]
    DescriptorMismatch { field: &'static str },
}

fn validate_chunk_profile_v1(
    profile: &ChunkingProfileV1,
) -> Result<(), PdpChunkProfileValidationError> {
    validate_profile_text(
        "namespace",
        &profile.namespace,
        PDP_CHUNK_PROFILE_COMPONENT_MAX_BYTES_V1,
    )?;
    validate_profile_text(
        "name",
        &profile.name,
        PDP_CHUNK_PROFILE_COMPONENT_MAX_BYTES_V1,
    )?;
    validate_profile_text(
        "semver",
        &profile.semver,
        PDP_CHUNK_PROFILE_SEMVER_MAX_BYTES_V1,
    )?;
    if profile.aliases.is_empty() || profile.aliases.len() > PDP_CHUNK_PROFILE_MAX_ALIASES_V1 {
        return Err(PdpChunkProfileValidationError::InvalidAliasCount {
            found: profile.aliases.len(),
        });
    }
    let mut aliases = BTreeSet::new();
    for (index, alias) in profile.aliases.iter().enumerate() {
        if alias.is_empty()
            || alias.len() > PDP_CHUNK_PROFILE_ALIAS_MAX_BYTES_V1
            || alias.trim() != alias
            || !alias.is_ascii()
            || alias.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(PdpChunkProfileValidationError::InvalidAlias { index });
        }
        if !aliases.insert(alias.as_str()) {
            return Err(PdpChunkProfileValidationError::DuplicateAlias {
                alias: alias.clone(),
            });
        }
    }
    if profile.min_size == 0
        || profile.min_size > profile.target_size
        || profile.target_size > profile.max_size
    {
        return Err(PdpChunkProfileValidationError::InvalidSizes);
    }
    if profile.break_mask == 0 {
        return Err(PdpChunkProfileValidationError::InvalidBreakMask);
    }
    if profile.multihash_code != BLAKE3_256_MULTIHASH_CODE {
        return Err(PdpChunkProfileValidationError::InvalidMultihash);
    }

    if profile.profile_id == ProfileId(0) {
        if profile.namespace != "inline" {
            return Err(PdpChunkProfileValidationError::DescriptorMismatch { field: "namespace" });
        }
        if profile.name != "inline" {
            return Err(PdpChunkProfileValidationError::DescriptorMismatch { field: "name" });
        }
        if profile.semver != "0.0.0" {
            return Err(PdpChunkProfileValidationError::DescriptorMismatch { field: "semver" });
        }
        if profile.aliases.len() != 1
            || profile.aliases.first().map(String::as_str) != Some("inline.inline@0.0.0")
        {
            return Err(PdpChunkProfileValidationError::DescriptorMismatch { field: "aliases" });
        }
        return Ok(());
    }

    let descriptor = crate::chunker_registry::lookup(profile.profile_id).ok_or(
        PdpChunkProfileValidationError::UnknownProfile {
            profile_id: profile.profile_id.0,
        },
    )?;
    let checks = [
        (profile.namespace == descriptor.namespace, "namespace"),
        (profile.name == descriptor.name, "name"),
        (profile.semver == descriptor.semver, "semver"),
        (
            profile.min_size == descriptor.profile.min_size as u32,
            "min_size",
        ),
        (
            profile.target_size == descriptor.profile.target_size as u32,
            "target_size",
        ),
        (
            profile.max_size == descriptor.profile.max_size as u32,
            "max_size",
        ),
        (
            profile.break_mask == descriptor.profile.break_mask as u32,
            "break_mask",
        ),
        (
            profile.multihash_code == descriptor.multihash_code,
            "multihash_code",
        ),
        (
            profile.aliases
                == descriptor
                    .aliases
                    .iter()
                    .map(|alias| (*alias).to_owned())
                    .collect::<Vec<_>>(),
            "aliases",
        ),
    ];
    for (matches, field) in checks {
        if !matches {
            return Err(PdpChunkProfileValidationError::DescriptorMismatch { field });
        }
    }
    Ok(())
}

fn validate_profile_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), PdpChunkProfileValidationError> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.trim() != value
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(PdpChunkProfileValidationError::InvalidText { field });
    }
    Ok(())
}

fn domain_separated_norito_digest<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], norito::core::Error> {
    let bytes = norito::to_bytes(value)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&bytes);
    Ok(hasher.finalize().into())
}

fn ensure_canonical_size<T: norito::core::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<(), norito::core::Error> {
    let bytes = norito::to_bytes(value)?;
    if bytes.len() > maximum {
        return Err(norito::core::Error::ArchiveLengthExceeded {
            length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            limit: u64::try_from(maximum).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

fn div_ceil_u64(value: u64, divisor: u64) -> Option<u64> {
    if divisor == 0 {
        return None;
    }
    (value / divisor).checked_add(u64::from(!value.is_multiple_of(divisor)))
}

fn tree_height(count: u64) -> Result<u16, PdpCommitmentValidationError> {
    if count == 0 {
        return Err(PdpCommitmentValidationError::GeometryOverflow);
    }
    u16::try_from(merkle_path_depth(count) + 1)
        .map_err(|_| PdpCommitmentValidationError::GeometryOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, CouncilSignature,
        EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1, EndpointKind,
        PathDiversityPolicy, ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeV1,
        ProviderAdmissionProposalV1, ProviderAdvertBodyV1, ProviderVrfPublicKeyV1, QosHints,
        StakePointer, compute_advert_body_digest, compute_envelope_authorization_digest,
        compute_proposal_digest,
    };

    const PROVIDER_ID: [u8; 32] = [0x31; 32];
    const MANIFEST_DIGEST: [u8; 32] = [0x42; 32];

    struct Fixture {
        payload: Vec<u8>,
        commitment: PdpCommitmentV1,
        challenge: PdpChallengeV1,
        proof: PdpProofV1,
        signing_key: SigningKey,
        admission: AdmissionRecord,
    }

    fn canonical_profile() -> ChunkingProfileV1 {
        ChunkingProfileV1::from_descriptor(
            crate::chunker_registry::lookup(ProfileId(1)).expect("SF1 profile exists"),
        )
    }

    fn deterministic_payload(length: usize) -> Vec<u8> {
        (0..length)
            .map(|index| ((index.wrapping_mul(131).wrapping_add(17)) % 251) as u8)
            .collect()
    }

    fn synthetic_admission(provider_id: [u8; 32], advert_key: [u8; 32]) -> AdmissionRecord {
        synthetic_admission_with_window(provider_id, advert_key, 1, 1_000)
    }

    fn synthetic_admission_with_window(
        provider_id: [u8; 32],
        advert_key: [u8; 32],
        issued_at: u64,
        retention_epoch: u64,
    ) -> AdmissionRecord {
        let descriptor = crate::chunker_registry::lookup(ProfileId(1)).expect("SF1 profile");
        let profile_aliases = Some(
            descriptor
                .aliases
                .iter()
                .map(|alias| (*alias).to_owned())
                .collect(),
        );
        let stake = StakePointer {
            pool_id: [0x91; 32],
            stake_amount: crate::deal::XorQuantity::try_from_micro(1)
                .expect("legacy micro-XOR stake is representable"),
        };
        let capability = CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        };
        let endpoint = AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "pdp.example.test".to_owned(),
            metadata: Vec::new(),
        };
        let endpoint_admission = EndpointAdmissionV1 {
            endpoint: endpoint.clone(),
            attestation: EndpointAttestationV1 {
                version: crate::ENDPOINT_ATTESTATION_VERSION_V1,
                kind: EndpointAttestationKind::Mtls,
                attested_at: 1,
                expires_at: 1_000,
                leaf_certificate: vec![1],
                intermediate_certificates: Vec::new(),
                alpn_ids: vec!["h2".to_owned()],
                report: Vec::new(),
            },
        };
        let (vrf_public, vrf_private) =
            iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![0x34; 32]))
                .expect("fixture BLS keypair");
        let vrf_pair: iroha_crypto::KeyPair = (vrf_public, vrf_private).into();
        let vrf_bytes = vrf_pair.public_key().to_bytes().1;
        let proposal = ProviderAdmissionProposalV1 {
            version: crate::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases,
            stake: stake.clone(),
            capabilities: vec![capability.clone()],
            endpoints: vec![endpoint_admission],
            advert_key,
            por_vrf_key: ProviderVrfPublicKeyV1::BlsNormal(
                vrf_bytes.try_into().expect("normal BLS key is 48 bytes"),
            ),
            jurisdiction_code: "US".to_owned(),
            contact_uri: None,
            stream_budget: None,
            transport_hints: None,
        };
        let advert_body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id: proposal.profile_id.clone(),
            profile_aliases: proposal.profile_aliases.clone(),
            stake,
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1,
                max_concurrent_streams: 1,
            },
            capabilities: vec![capability],
            endpoints: vec![endpoint],
            rendezvous_topics: Vec::new(),
            path_policy: PathDiversityPolicy {
                min_guard_weight: 1,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: None,
        };
        let proposal_digest = compute_proposal_digest(&proposal).expect("proposal digest");
        let advert_body_digest =
            compute_advert_body_digest(&advert_body).expect("advert body digest");
        let mut envelope = ProviderAdmissionEnvelopeV1 {
            version: crate::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal,
            proposal_digest,
            advert_body,
            advert_body_digest,
            issued_at,
            retention_epoch,
            council_signatures: Vec::new(),
            notes: None,
        };
        let council_key = SigningKey::from_bytes(&[0x61; 32]);
        let authorization_digest = compute_envelope_authorization_digest(&envelope)
            .expect("envelope authorization digest");
        envelope.council_signatures.push(CouncilSignature {
            signer: council_key.verifying_key().to_bytes(),
            signature: council_key.sign(&authorization_digest).to_bytes().to_vec(),
        });
        let policy =
            ProviderAdmissionCouncilPolicy::new([council_key.verifying_key().to_bytes()], 1)
                .expect("council policy");
        AdmissionRecord::new(envelope, &policy).expect("council-verified admission")
    }

    fn fixture() -> Fixture {
        let payload = deterministic_payload(
            PDP_SEGMENT_SIZE_V1 as usize + PDP_HOT_LEAF_SIZE_V1 as usize + 37,
        );
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("build tree");
        let commitment =
            PdpCommitmentV1::from_tree(&tree, MANIFEST_DIGEST, canonical_profile(), 4, 100)
                .expect("commitment");
        let challenge = PdpChallengeV1::new(
            commitment.commitment_digest().expect("commitment digest"),
            MANIFEST_DIGEST,
            PROVIDER_ID,
            canonical_profile(),
            [0x51; 32],
            7,
            11,
            200,
            300,
            vec![
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0, 63],
                },
                PdpSampleV1 {
                    segment_index: 1,
                    hot_leaf_indices: vec![0, 1],
                },
            ],
        )
        .expect("challenge");
        let signing_key = SigningKey::from_bytes(&[0x21; 32]);
        let proof = sign_pdp_proof_ed25519_v1(
            PdpProofV1 {
                version: PDP_PROOF_VERSION_V1,
                commitment_digest: challenge.commitment_digest,
                challenge_id: challenge.challenge_id,
                manifest_digest: challenge.manifest_digest,
                provider_id: challenge.provider_id,
                epoch_id: challenge.epoch_id,
                proof_leaves: tree
                    .prove_samples(&challenge.samples, &payload)
                    .expect("proof leaves"),
                issued_at_unix: 250,
                signature: PdpEd25519SignatureV1 {
                    public_key: [0; 32],
                    signature: [0; 64],
                },
            },
            &signing_key,
        )
        .expect("sign proof");
        let admission = synthetic_admission(PROVIDER_ID, signing_key.verifying_key().to_bytes());
        Fixture {
            payload,
            commitment,
            challenge,
            proof,
            signing_key,
            admission,
        }
    }

    fn resign(proof: &mut PdpProofV1, key: &SigningKey) {
        *proof = sign_pdp_proof_ed25519_v1(proof.clone(), key).expect("re-sign proof");
    }

    fn rebind_challenge(challenge: &mut PdpChallengeV1) {
        challenge.challenge_id = challenge.derived_challenge_id().expect("challenge id");
    }

    fn rebind_bundle(fixture: &mut Fixture) {
        fixture.challenge.commitment_digest = fixture
            .commitment
            .commitment_digest()
            .expect("commitment digest");
        rebind_challenge(&mut fixture.challenge);
        fixture.proof.commitment_digest = fixture.challenge.commitment_digest;
        fixture.proof.challenge_id = fixture.challenge.challenge_id;
        resign(&mut fixture.proof, &fixture.signing_key);
    }

    fn accepted_archive(fixture: &Fixture) -> PdpGovernanceArchiveV1 {
        let verified = verify_pdp_bundle_v1(
            &fixture.commitment,
            &fixture.challenge,
            &fixture.proof,
            &fixture.admission,
        )
        .expect("verified archive fixture");
        PdpGovernanceArchiveV1 {
            version: PDP_GOVERNANCE_ARCHIVE_VERSION_V1,
            sequence: 1,
            challenge_id: fixture.challenge.challenge_id,
            commitment_digest: fixture.challenge.commitment_digest,
            manifest_digest: fixture.challenge.manifest_digest,
            provider_id: fixture.challenge.provider_id,
            epoch_id: fixture.challenge.epoch_id,
            decision: PdpTerminalDecisionV1::Accepted,
            proof_digest: Some(fixture.proof.proof_digest().expect("proof digest")),
            sampled_segments: verified.sampled_segments(),
            sampled_hot_leaves: verified.sampled_hot_leaves(),
            sampled_bytes: verified.sampled_bytes(),
            issued_at_unix: fixture.challenge.issued_at_unix,
            response_deadline_unix: fixture.challenge.response_deadline_unix,
            decided_at_unix: fixture.proof.issued_at_unix,
            admission_envelope_digest: *fixture.admission.envelope_digest(),
            canonical_challenge: norito::to_bytes(&fixture.challenge).expect("challenge bytes"),
            canonical_proof: Some(norito::to_bytes(&fixture.proof).expect("proof bytes")),
        }
    }

    #[test]
    fn governance_archive_is_typed_canonical_and_roundtrips() {
        let archive = accepted_archive(&fixture());
        archive.validate().expect("valid accepted archive");
        let digest = archive.digest().expect("archive digest");
        let bytes = norito::to_bytes(&archive).expect("archive bytes");
        assert!(bytes.len() <= PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1);
        let decoded: PdpGovernanceArchiveV1 =
            norito::decode_from_bytes(&bytes).expect("decode archive");
        assert_eq!(decoded, archive);
        assert_eq!(decoded.digest().expect("decoded digest"), digest);
        decoded.validate().expect("decoded archive validates");
    }

    #[test]
    fn governance_archive_rejects_identity_timeline_binding_and_payload_attacks() {
        for mutation in 0..10 {
            let mut archive = accepted_archive(&fixture());
            match mutation {
                0 => archive.version = 0,
                1 => archive.sequence = 0,
                2 => archive.manifest_digest[0] ^= 1,
                3 => archive.sampled_hot_leaves += 1,
                4 => archive.sampled_bytes = 0,
                5 => archive.decided_at_unix = archive.response_deadline_unix + 1,
                6 => archive.proof_digest = None,
                7 => archive.canonical_challenge.push(0),
                8 => archive.canonical_proof.as_mut().expect("proof")[0] ^= 1,
                9 => {
                    archive.decision =
                        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::DeadlineExpired)
                }
                _ => unreachable!(),
            }
            assert!(archive.validate().is_err(), "mutation {mutation} must fail");
        }
    }

    #[test]
    fn governance_archive_preserves_authenticated_invalid_proof_and_no_show_evidence() {
        let fixture = fixture();
        let mut invalid_proof = fixture.proof.clone();
        invalid_proof.manifest_digest[0] ^= 1;
        resign(&mut invalid_proof, &fixture.signing_key);
        let mut rejected = accepted_archive(&fixture);
        rejected.decision = PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof);
        rejected.proof_digest = Some(invalid_proof.proof_digest().expect("invalid proof digest"));
        rejected.canonical_proof =
            Some(norito::to_bytes(&invalid_proof).expect("invalid proof bytes"));
        rejected.sampled_bytes = 0;
        rejected
            .validate()
            .expect("authenticated invalid proof remains archivable");

        let mut expired = accepted_archive(&fixture);
        expired.decision = PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::DeadlineExpired);
        expired.proof_digest = None;
        expired.canonical_proof = None;
        expired.sampled_bytes = 0;
        expired.decided_at_unix = expired.response_deadline_unix + 1;
        expired.validate().expect("no-show archive");
    }

    #[test]
    fn exhaustive_verifier_accepts_real_bytes_both_roots_and_admission_key() {
        let fixture = fixture();
        let verified = verify_pdp_bundle_v1(
            &fixture.commitment,
            &fixture.challenge,
            &fixture.proof,
            &fixture.admission,
        )
        .expect("real PDP proof");

        assert_eq!(
            verified.commitment_digest(),
            &fixture.challenge.commitment_digest
        );
        assert_eq!(verified.challenge_id(), &fixture.challenge.challenge_id);
        assert_eq!(verified.provider_id(), &PROVIDER_ID);
        assert_eq!(verified.sampled_segments(), 2);
        assert_eq!(verified.sampled_hot_leaves(), 4);
        assert_eq!(
            verified.sampled_bytes(),
            u64::from(PDP_HOT_LEAF_SIZE_V1) * 3 + 37
        );
        assert_eq!(
            verified.proof_digest(),
            &fixture.proof.proof_digest().expect("proof digest")
        );
    }

    #[test]
    fn diagnostic_witness_verifier_checks_both_roots_without_authorizing_admission() {
        let mut fixture = fixture();
        verify_pdp_witnesses_v1(&fixture.commitment, &fixture.challenge, &fixture.proof)
            .expect("valid witness set");

        fixture.proof.proof_leaves[0].segment_merkle_path[0][0] ^= 1;
        resign(&mut fixture.proof, &fixture.signing_key);
        assert!(matches!(
            verify_pdp_witnesses_v1(&fixture.commitment, &fixture.challenge, &fixture.proof),
            Err(PdpVerificationError::SegmentRootMismatch { .. })
        ));
    }

    #[test]
    fn reference_production_verifier_emits_acceptance_only_with_governed_admission() {
        let fixture = fixture();
        let outcome =
            crate::reference::validate_pdp_commitment_challenge_proof_with_admission_bytes(
                &norito::to_bytes(&fixture.commitment).expect("commitment bytes"),
                &norito::to_bytes(&fixture.challenge).expect("challenge bytes"),
                &norito::to_bytes(&fixture.proof).expect("proof bytes"),
                "commitment.to",
                "challenge.to",
                "proof.to",
                &fixture.admission,
                301,
            );

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| { field.key == "production_acceptance" && field.value == "true" })
        );
        assert!(outcome.context.iter().any(|field| {
            field.key == "verification_scope" && field.value == "exhaustive_production"
        }));
    }

    #[test]
    fn tree_geometry_and_roots_cover_boundary_and_partial_payloads() {
        let cases = [
            (1usize, 1u64, 1u64),
            (PDP_HOT_LEAF_SIZE_V1 as usize, 1, 1),
            (PDP_HOT_LEAF_SIZE_V1 as usize + 1, 2, 1),
            (PDP_SEGMENT_SIZE_V1 as usize, 64, 1),
            (PDP_SEGMENT_SIZE_V1 as usize + 1, 65, 2),
        ];
        for (length, expected_hot, expected_segments) in cases {
            let payload = deterministic_payload(length);
            let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
            assert_eq!(tree.payload_len(), length as u64);
            assert_eq!(tree.hot_leaf_count(), expected_hot);
            assert_eq!(tree.segment_count(), expected_segments);
            assert_ne!(tree.hot_root(), tree.segment_root());
            assert_eq!(
                tree.hot_tree_height(),
                u16::try_from(merkle_path_depth(expected_hot) + 1).unwrap()
            );
            assert_eq!(
                tree.segment_tree_height(),
                u16::try_from(merkle_path_depth(expected_segments) + 1).unwrap()
            );
        }
        assert_eq!(
            PdpMerkleTreeV1::from_bytes(&[]),
            Err(PdpMerkleTreeError::EmptyPayload)
        );
    }

    #[test]
    fn commitment_rejects_every_geometry_drift() {
        for mutation in 0..8 {
            let mut fixture = fixture();
            match mutation {
                0 => fixture.commitment.payload_len = 0,
                1 => fixture.commitment.hot_leaf_size += 1,
                2 => fixture.commitment.segment_size += 1,
                3 => fixture.commitment.hot_leaf_count += 1,
                4 => fixture.commitment.segment_count += 1,
                5 => fixture.commitment.hot_tree_height += 1,
                6 => fixture.commitment.segment_tree_height += 1,
                7 => fixture.commitment.sample_window = 0,
                _ => unreachable!(),
            }
            assert!(
                fixture.commitment.validate().is_err(),
                "geometry mutation {mutation} must fail"
            );
        }
    }

    #[test]
    fn commitment_and_challenge_digests_bind_every_mutation() {
        let fixture = fixture();
        let digest = fixture
            .commitment
            .commitment_digest()
            .expect("commitment digest");
        let mut commitment = fixture.commitment.clone();
        commitment.sample_window += 1;
        assert_ne!(
            digest,
            commitment.commitment_digest().expect("mutated digest")
        );

        for mutation in 0..5 {
            let mut challenge = fixture.challenge.clone();
            match mutation {
                0 => challenge.seed[0] ^= 1,
                1 => challenge.epoch_id += 1,
                2 => challenge.drand_round += 1,
                3 => challenge.response_deadline_unix += 1,
                4 => challenge.samples[0].hot_leaf_indices.swap(0, 1),
                _ => unreachable!(),
            }
            assert_eq!(
                challenge.validate(),
                Err(if mutation == 4 {
                    PdpChallengeValidationError::NonCanonicalHotLeafOrder { segment_index: 0 }
                } else {
                    PdpChallengeValidationError::ChallengeIdMismatch
                })
            );
        }
    }

    #[test]
    fn challenge_rejects_duplicate_unordered_and_excess_coverage() {
        let fixture = fixture();
        let mut duplicate_segment = fixture.challenge.clone();
        duplicate_segment.samples[1].segment_index = 0;
        rebind_challenge(&mut duplicate_segment);
        assert_eq!(
            duplicate_segment.validate(),
            Err(PdpChallengeValidationError::NonCanonicalSegmentOrder)
        );

        let mut duplicate_leaf = fixture.challenge.clone();
        duplicate_leaf.samples[0].hot_leaf_indices = vec![1, 1];
        rebind_challenge(&mut duplicate_leaf);
        assert_eq!(
            duplicate_leaf.validate(),
            Err(PdpChallengeValidationError::NonCanonicalHotLeafOrder { segment_index: 0 })
        );

        let mut excessive = fixture.challenge.clone();
        excessive.samples = (0..=PDP_MAX_SEGMENT_SAMPLES_V1)
            .map(|index| PdpSampleV1 {
                segment_index: index as u64,
                hot_leaf_indices: vec![0],
            })
            .collect();
        rebind_challenge(&mut excessive);
        assert!(matches!(
            excessive.validate(),
            Err(PdpChallengeValidationError::TooManySegments { .. })
        ));
    }

    #[test]
    fn signature_is_typed_strict_domain_separated_and_admission_bound() {
        let mut tampered = fixture();
        tampered.proof.signature.signature[0] ^= 1;
        assert!(matches!(
            verify_pdp_bundle_v1(
                &tampered.commitment,
                &tampered.challenge,
                &tampered.proof,
                &tampered.admission
            ),
            Err(PdpVerificationError::Signature(_))
        ));

        let fixture = fixture();
        let wrong_key = SigningKey::from_bytes(&[0x22; 32]);
        let wrong_admission =
            synthetic_admission(PROVIDER_ID, wrong_key.verifying_key().to_bytes());
        assert_eq!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &wrong_admission,
            ),
            Err(PdpVerificationError::AdmissionKeyMismatch)
        );

        let wrong_provider = synthetic_admission([0x99; 32], fixture.proof.signature.public_key);
        assert_eq!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &wrong_provider,
            ),
            Err(PdpVerificationError::AdmissionProviderMismatch)
        );

        let mut inert = fixture.proof;
        inert.signature.public_key = [0; 32];
        assert!(matches!(
            inert.validate(),
            Err(PdpProofValidationError::InvalidSignature(
                PdpSignatureVerificationError::InvalidPublicKey { .. }
            ))
        ));
    }

    #[test]
    fn verifier_rejects_untrusted_and_inactive_admission_records() {
        let fixture = fixture();
        let untrusted =
            AdmissionRecord::new_untrusted_signers(fixture.admission.envelope().clone())
                .expect("integrity-only admission");
        assert!(!untrusted.is_council_verified());
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &untrusted,
            ),
            Err(PdpVerificationError::AdmissionNotCouncilVerified)
        ));

        let future = synthetic_admission_with_window(
            PROVIDER_ID,
            fixture.proof.signature.public_key,
            fixture.challenge.issued_at_unix + 1,
            1_000,
        );
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &future,
            ),
            Err(PdpVerificationError::AdmissionNotActiveAtChallenge { .. })
        ));

        let deadline_expired = synthetic_admission_with_window(
            PROVIDER_ID,
            fixture.proof.signature.public_key,
            1,
            fixture.challenge.response_deadline_unix - 1,
        );
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &deadline_expired,
            ),
            Err(PdpVerificationError::ChallengeBeyondAdmission { .. })
        ));

        let proof_expired = synthetic_admission_with_window(
            PROVIDER_ID,
            fixture.proof.signature.public_key,
            1,
            fixture.challenge.response_deadline_unix,
        );
        let mut late_proof = fixture.proof.clone();
        late_proof.issued_at_unix = fixture.challenge.response_deadline_unix + 1;
        resign(&mut late_proof, &fixture.signing_key);
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &late_proof,
                &proof_expired,
            ),
            Err(PdpVerificationError::AdmissionExpiredAtProof { .. })
        ));
    }

    #[test]
    fn verifier_rejects_unsigned_and_resigned_sample_byte_mutation() {
        let mut fixture = fixture();
        fixture.proof.proof_leaves[0].hot_leaves[0].leaf_bytes[0] ^= 1;
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &fixture.admission
            ),
            Err(PdpVerificationError::Signature(_))
        ));

        resign(&mut fixture.proof, &fixture.signing_key);
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &fixture.admission
            ),
            Err(PdpVerificationError::HotRootMismatch { .. })
                | Err(PdpVerificationError::InconsistentSegmentCommitment { .. })
        ));
    }

    #[test]
    fn verifier_rejects_geometry_and_exact_coverage_mutations() {
        for mutation in 0..6 {
            let mut fixture = fixture();
            match mutation {
                0 => {
                    fixture.proof.proof_leaves.pop();
                }
                1 => {
                    fixture.proof.proof_leaves[0].hot_leaves.pop();
                }
                2 => fixture.proof.proof_leaves[0].segment_offset += 1,
                3 => fixture.proof.proof_leaves[0].hot_leaves[0].leaf_offset += 1,
                4 => {
                    fixture.proof.proof_leaves[1].hot_leaves[1].leaf_length -= 1;
                }
                5 => {
                    fixture.proof.proof_leaves[1].hot_leaves[1].leaf_bytes.pop();
                }
                _ => unreachable!(),
            }
            // Coverage omissions remain structurally valid, so re-sign them to
            // ensure the exhaustive coverage check—not merely the signature—
            // rejects the proof. Geometry corruptions are intentionally left
            // unsigned because the signing helper refuses malformed proofs.
            if mutation < 2 {
                resign(&mut fixture.proof, &fixture.signing_key);
            }
            assert!(
                verify_pdp_bundle_v1(
                    &fixture.commitment,
                    &fixture.challenge,
                    &fixture.proof,
                    &fixture.admission,
                )
                .is_err(),
                "geometry/coverage mutation {mutation} must fail"
            );
        }
    }

    #[test]
    fn verifier_rejects_each_path_class_and_cross_segment_substitution() {
        for mutation in 0..4 {
            let mut fixture = fixture();
            match mutation {
                0 => {
                    fixture.proof.proof_leaves[0].hot_leaves[0]
                        .segment_hot_merkle_path
                        .pop();
                }
                1 => {
                    fixture.proof.proof_leaves[0].hot_leaves[0]
                        .global_hot_merkle_path
                        .pop();
                }
                2 => {
                    fixture.proof.proof_leaves[0].segment_merkle_path.pop();
                }
                3 => {
                    fixture.proof.proof_leaves[0].segment_merkle_path =
                        fixture.proof.proof_leaves[1].segment_merkle_path.clone();
                }
                _ => unreachable!(),
            }
            resign(&mut fixture.proof, &fixture.signing_key);
            assert!(
                verify_pdp_bundle_v1(
                    &fixture.commitment,
                    &fixture.challenge,
                    &fixture.proof,
                    &fixture.admission,
                )
                .is_err(),
                "path mutation {mutation} must fail"
            );
        }
    }

    #[test]
    fn verifier_rejects_noncanonical_odd_padding_sibling() {
        let mut fixture = fixture();
        // Global leaf 65 is the unpaired node after the first reduction of a
        // 66-leaf tree, so level one must self-duplicate canonically.
        let path = &mut fixture.proof.proof_leaves[1].hot_leaves[1].global_hot_merkle_path;
        path[1][0] ^= 1;
        resign(&mut fixture.proof, &fixture.signing_key);
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &fixture.admission,
            ),
            Err(PdpVerificationError::GlobalHotPath {
                source: PdpMerklePathError::NonCanonicalOddSibling { .. },
                ..
            })
        ));
    }

    #[test]
    fn verifier_rejects_rebound_hot_and_segment_root_mutations() {
        for mutate_hot in [true, false] {
            let mut fixture = fixture();
            if mutate_hot {
                fixture.commitment.commitment_root_hot[0] ^= 1;
            } else {
                fixture.commitment.commitment_root_segment[0] ^= 1;
            }
            rebind_bundle(&mut fixture);
            let error = verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &fixture.admission,
            )
            .expect_err("mutated root must fail");
            assert!(matches!(
                (mutate_hot, error),
                (true, PdpVerificationError::HotRootMismatch { .. })
                    | (false, PdpVerificationError::SegmentRootMismatch { .. })
            ));
        }
    }

    #[test]
    fn verifier_enforces_both_deadline_boundaries_and_seal_order() {
        for proof_at in [199, 200, 300, 301] {
            let mut fixture = fixture();
            fixture.proof.issued_at_unix = proof_at;
            resign(&mut fixture.proof, &fixture.signing_key);
            let result = verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &fixture.admission,
            );
            assert_eq!(result.is_ok(), matches!(proof_at, 200 | 300));
        }

        let mut fixture = fixture();
        fixture.commitment.sealed_at = fixture.challenge.issued_at_unix + 1;
        rebind_bundle(&mut fixture);
        assert!(matches!(
            verify_pdp_bundle_v1(
                &fixture.commitment,
                &fixture.challenge,
                &fixture.proof,
                &fixture.admission,
            ),
            Err(PdpVerificationError::CommitmentSealedAfterChallenge { .. })
        ));
    }

    #[test]
    fn proof_digest_binds_bytes_paths_geometry_time_and_signer() {
        let fixture = fixture();
        let original = fixture.proof.proof_digest().expect("proof digest");
        for mutation in 0..6 {
            let mut proof = fixture.proof.clone();
            match mutation {
                0 => proof.proof_leaves[0].hot_leaves[0].leaf_bytes[0] ^= 1,
                1 => proof.proof_leaves[0].hot_leaves[0].leaf_offset += 1,
                2 => proof.proof_leaves[0].hot_leaves[0].global_hot_merkle_path[0][0] ^= 1,
                3 => proof.proof_leaves[0].segment_merkle_path[0][0] ^= 1,
                4 => proof.issued_at_unix += 1,
                5 => proof.signature.public_key[0] ^= 1,
                _ => unreachable!(),
            }
            assert_ne!(original, proof.proof_digest().expect("mutated digest"));
        }
    }

    #[test]
    fn tree_proof_constructor_rejects_wrong_payload_and_out_of_range_samples() {
        let fixture = fixture();
        let tree = PdpMerkleTreeV1::from_bytes(&fixture.payload).expect("tree");
        let mut wrong_payload = fixture.payload.clone();
        wrong_payload[0] ^= 1;
        assert!(matches!(
            tree.prove_samples(&fixture.challenge.samples, &wrong_payload),
            Err(PdpMerkleTreeError::PayloadDigestMismatch { .. })
        ));
        assert!(matches!(
            tree.prove_samples(
                &[PdpSampleV1 {
                    segment_index: tree.segment_count(),
                    hot_leaf_indices: vec![0],
                }],
                &fixture.payload,
            ),
            Err(PdpMerkleTreeError::SegmentOutOfRange { .. })
        ));
        assert!(matches!(
            tree.prove_samples(
                &[PdpSampleV1 {
                    segment_index: 1,
                    hot_leaf_indices: vec![2],
                }],
                &fixture.payload,
            ),
            Err(PdpMerkleTreeError::HotLeafOutOfRange { .. })
        ));
    }

    #[test]
    fn profile_validation_rejects_unknown_drift_duplicate_alias_and_unbounded_text() {
        for mutation in 0..5 {
            let mut profile = canonical_profile();
            match mutation {
                0 => profile.profile_id = ProfileId(999),
                1 => profile.target_size += 1,
                2 => profile.aliases.push(profile.aliases[0].clone()),
                3 => profile.namespace = "x".repeat(PDP_CHUNK_PROFILE_COMPONENT_MAX_BYTES_V1 + 1),
                4 => profile.break_mask = 0,
                _ => unreachable!(),
            }
            assert!(
                validate_chunk_profile_v1(&profile).is_err(),
                "profile mutation {mutation} must fail"
            );
        }
    }
}
