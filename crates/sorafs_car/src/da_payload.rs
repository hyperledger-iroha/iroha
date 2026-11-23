//! SoraFS chunking and PDP commitments for retained DA sources.
pub use sorafs_manifest::da::chunk_profile_for_request;

/// Construct the DA receipt's PDP commitment from the same canonical bytes and chunker
/// that produced the source digest and PoR root.
///
/// # Errors
/// Rejects empty payloads and invalid PDP commitment fields.
pub fn pdp_commitment(
    chunks: &crate::ChunkStore,
    manifest_digest: [u8; 32],
    sample_window: u16,
    sealed_at: u64,
) -> Result<sorafs_manifest::pdp::PdpCommitmentV1, sorafs_manifest::pdp::PdpCommitmentValidationError>
{
    use sorafs_manifest::pdp::PdpCommitmentValidationError;
    sorafs_manifest::da::pdp_commitment_for_tree(
        chunks
            .pdp_tree()
            .ok_or(PdpCommitmentValidationError::InvalidPayloadLength)?,
        chunks.profile(),
        manifest_digest,
        sample_window,
        sealed_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn da_chunk_profile_is_fixed_width() {
        let profile = chunk_profile_for_request(1024);
        assert_eq!(
            (
                profile.min_size,
                profile.target_size,
                profile.max_size,
                profile.break_mask
            ),
            (1024, 1024, 1024, 1)
        );
    }

    #[test]
    fn da_source_commitments_match_retained_store_at_tree_boundaries() {
        use sorafs_manifest::da::source_commitments;
        for (length, chunk_size) in [
            (1, 1024),
            (1024, 1024),
            (1025, 1024),
            (4097, 1024),
            (64 * 1024, 64 * 1024),
            (2 * 64 * 1024 + 17, 64 * 1024),
            (2 * 256 * 1024 + 4097, 128 * 1024),
        ] {
            let payload: Vec<_> = (0..length).map(|i| (i % 251) as u8).collect();
            let mut store = crate::ChunkStore::with_profile(chunk_profile_for_request(chunk_size));
            store.ingest_bytes(&payload).unwrap();
            let (root, pdp) = source_commitments(&payload, chunk_size, [19; 32], 3, 12).unwrap();
            assert_eq!(&root, store.por_tree().root());
            assert_eq!(pdp, pdp_commitment(&store, [19; 32], 3, 12).unwrap());
            assert_eq!(pdp.chunk_profile.min_size, chunk_size);
            assert_eq!(pdp.payload_len, length as u64);
            assert_eq!(pdp.sample_window, 3);
            assert_eq!(pdp.sealed_at, 12);
        }
    }
}
