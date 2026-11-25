//! Source-bound application data availability commitments.
use crate::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1,
    pdp::{PdpCommitmentV1, PdpCommitmentValidationError, PdpMerkleTreeError, PdpMerkleTreeV1},
};
use sorafs_chunker::{ChunkProfile, por::PorRootBuilder};
use thiserror::Error;

/// Maximum transmitted or decompressed application blob size.
pub const MAX_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;
/// Smallest source chunk width accepted for DA.
pub const MIN_CHUNK_SIZE_BYTES: u32 = 1024;
/// Largest source chunk width accepted for DA.
pub const MAX_CHUNK_SIZE_BYTES: u32 = 2 * 1024 * 1024;
/// Maximum number of source chunks in a DA blob.
pub const MAX_DATA_CHUNKS: usize = 1024;

/// Failure to derive canonical DA commitments from source bytes.
#[derive(Debug, Error)]
pub enum DaSourceCommitmentError {
    /// The source is empty or exceeds the V1 byte limit.
    #[error("DA source length must be in 1..={MAX_PAYLOAD_BYTES} bytes")]
    InvalidLength,
    /// The configured chunk size is outside the admitted range.
    #[error(
        "DA chunk width must be a power of two in {MIN_CHUNK_SIZE_BYTES}..={MAX_CHUNK_SIZE_BYTES} bytes"
    )]
    InvalidChunkSize,
    /// The source requires more chunks than the V1 inventory permits.
    #[error("DA source exceeds the limit of {MAX_DATA_CHUNKS} chunks")]
    TooManyChunks,
    /// The source geometry cannot form a canonical PDP tree.
    #[error(transparent)]
    PdpTree(#[from] PdpMerkleTreeError),
    /// The manifest, sample policy, or sealing time cannot form a valid commitment.
    #[error(transparent)]
    PdpCommitment(#[from] PdpCommitmentValidationError),
}

/// Fixed-width profile committed to by the DA receipt.
#[must_use]
pub fn chunk_profile_for_request(chunk_size: u32) -> ChunkProfile {
    let size = chunk_size as usize;
    ChunkProfile {
        min_size: size,
        target_size: size,
        max_size: size,
        break_mask: 1,
    }
}

/// Bind the canonical PDP tree and chunking profile to the receipt manifest and policy.
///
/// # Errors
/// Returns the PDP protocol validation error for invalid commitment fields.
pub fn pdp_commitment_for_tree(
    tree: &PdpMerkleTreeV1,
    profile: ChunkProfile,
    manifest: [u8; 32],
    sample_window: u16,
    sealed_at: u64,
) -> Result<PdpCommitmentV1, PdpCommitmentValidationError> {
    PdpCommitmentV1::from_tree(
        tree,
        manifest,
        ChunkingProfileV1::from_profile(profile, BLAKE3_256_MULTIHASH_CODE),
        sample_window,
        sealed_at,
    )
}

/// Validate the first-release source extent, power-of-two chunk width, and chunk inventory.
///
/// # Errors
/// Returns the first invalid dimension before allocating or hashing source bytes.
pub fn validate_source_geometry(
    payload_len: u64,
    chunk_size: u32,
) -> Result<(), DaSourceCommitmentError> {
    if payload_len == 0 || payload_len > MAX_PAYLOAD_BYTES {
        return Err(DaSourceCommitmentError::InvalidLength);
    }
    if !chunk_size.is_power_of_two()
        || !(MIN_CHUNK_SIZE_BYTES..=MAX_CHUNK_SIZE_BYTES).contains(&chunk_size)
    {
        return Err(DaSourceCommitmentError::InvalidChunkSize);
    }
    if payload_len.div_ceil(u64::from(chunk_size)) > MAX_DATA_CHUNKS as u64 {
        return Err(DaSourceCommitmentError::TooManyChunks);
    }
    Ok(())
}

/// Derive the raw PoR root and complete PDP commitment from the decompressed DA source.
///
/// # Errors
/// Rejects sources outside the V1 payload and chunk bounds or malformed PDP fields.
pub fn source_commitments(
    payload: &[u8],
    chunk_size: u32,
    manifest: [u8; 32],
    sample_window: u16,
    sealed_at: u64,
) -> Result<([u8; 32], PdpCommitmentV1), DaSourceCommitmentError> {
    validate_source_geometry(payload.len() as u64, chunk_size)?;
    let pdp = pdp_commitment_for_tree(
        &PdpMerkleTreeV1::from_bytes(payload)?,
        chunk_profile_for_request(chunk_size),
        manifest,
        sample_window,
        sealed_at,
    )?;
    let mut builder = PorRootBuilder::default();
    for chunk in payload.chunks(chunk_size as usize) {
        builder
            .push_chunk(chunk)
            .ok_or(DaSourceCommitmentError::InvalidLength)?;
    }
    let root = builder
        .finish()
        .ok_or(DaSourceCommitmentError::InvalidLength)?;
    Ok((root, pdp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_geometry_enforces_power_of_two_and_exact_limits() {
        assert!(matches!(
            validate_source_geometry(0, MIN_CHUNK_SIZE_BYTES),
            Err(DaSourceCommitmentError::InvalidLength)
        ));
        assert!(matches!(
            validate_source_geometry(MAX_PAYLOAD_BYTES + 1, MAX_CHUNK_SIZE_BYTES),
            Err(DaSourceCommitmentError::InvalidLength)
        ));
        for width in [
            0,
            MIN_CHUNK_SIZE_BYTES - 1,
            MIN_CHUNK_SIZE_BYTES + 1,
            MAX_CHUNK_SIZE_BYTES + 1,
        ] {
            assert!(matches!(
                validate_source_geometry(1, width),
                Err(DaSourceCommitmentError::InvalidChunkSize)
            ));
        }
        let max_small_source = u64::from(MIN_CHUNK_SIZE_BYTES) * MAX_DATA_CHUNKS as u64;
        assert!(validate_source_geometry(max_small_source, MIN_CHUNK_SIZE_BYTES).is_ok());
        assert!(matches!(
            validate_source_geometry(max_small_source + 1, MIN_CHUNK_SIZE_BYTES),
            Err(DaSourceCommitmentError::TooManyChunks)
        ));
        assert!(validate_source_geometry(MAX_PAYLOAD_BYTES, MAX_CHUNK_SIZE_BYTES).is_ok());
    }

    #[test]
    fn source_commitments_refuse_invalid_manifest_policy_and_time() {
        for (manifest, sample_window, sealed_at) in
            [([0; 32], 1, 1), ([1; 32], 0, 1), ([1; 32], 1, 0)]
        {
            assert!(matches!(
                source_commitments(b"a", 1024, manifest, sample_window, sealed_at),
                Err(DaSourceCommitmentError::PdpCommitment(_))
            ));
        }
    }
}
