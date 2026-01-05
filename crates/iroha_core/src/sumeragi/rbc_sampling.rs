//! Helpers for generating Merkle inclusion proofs over persisted RBC chunk payloads.

use std::{collections::BTreeSet, path::Path};

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};
use iroha_data_model::block::BlockHeader;
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};

use super::{
    main_loop::{PersistedLoadError, RbcSession},
    rbc_store::{self, SessionKey, SoftwareManifest},
};

/// Maximum Merkle proof depth we expect for RBC chunk trees (log2 of chunk cap).
const MAX_PROOF_HEIGHT: usize = 16; // supports up to 2^16 leaves

/// Sampled chunk proof with payload bytes, digest, and Merkle path information.
#[derive(Debug)]
pub struct ChunkSample {
    /// Index of the chunk within the session payload.
    pub index: u32,
    /// Raw chunk payload bytes used to reconstruct the message.
    pub bytes: Vec<u8>,
    /// 32-byte SHA-256 digest associated with the chunk.
    pub digest: [u8; 32],
    /// Merkle inclusion proof for the chunk digest.
    pub proof: MerkleProof<[u8; 32]>,
}

/// Aggregated proof response describing the session metadata and sampled chunks.
#[derive(Debug)]
pub struct SessionSample {
    /// Block hash associated with the session key.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height of the RBC session.
    pub height: u64,
    /// Consensus view identifier for the RBC session.
    pub view: u64,
    /// Total number of chunks expected for this payload.
    pub total_chunks: u32,
    /// Merkle root committed in the RBC init message.
    pub chunk_root: Hash,
    /// Optional payload hash advertised by the broadcaster.
    pub payload_hash: Option<Hash>,
    /// Sampled chunks with inclusion proofs ready to return to clients.
    pub samples: Vec<ChunkSample>,
}

/// Errors that may occur during chunk sampling.
#[derive(Debug, thiserror::Error)]
pub enum SamplingError {
    /// I/O failure while accessing the RBC store on disk.
    #[error("RBC chunk store I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Persisted session failed validation before sampling.
    #[error("Persisted session invalid: {0:?}")]
    Persisted(PersistedLoadError),
    /// Requested sample count was zero or exceeded available chunks.
    #[error("Requested chunk count is zero or exceeds available chunks")]
    InvalidSampleCount,
    /// Session does not yet contain enough chunk data to rebuild Merkle root.
    #[error("Session does not have complete chunk data yet")]
    IncompleteSession,
    /// Merkle proof generation failed for the specified chunk index.
    #[error("Merkle proof generation failed for chunk {0}")]
    ProofGeneration(u32),
}

/// Load a persisted session from disk and sample `count` randomly selected chunks, returning
/// inclusion proofs suitable for light-client verification.
///
/// When the session file is absent the function returns `Ok(None)`.
///
/// # Errors
/// Returns [`SamplingError`] if the persisted session cannot be loaded, the sample configuration
/// is invalid, or proof generation fails for any selected chunk.
pub fn sample_from_store(
    dir: &Path,
    key: SessionKey,
    expected_chain_hash: &Hash,
    expected_manifest: &SoftwareManifest,
    count: u32,
    seed: Option<u64>,
) -> Result<Option<SessionSample>, SamplingError> {
    let persisted = match rbc_store::load_session_from_dir(
        dir,
        &key,
        expected_chain_hash,
        expected_manifest,
    )? {
        Some(p) => p,
        None => return Ok(None),
    };
    let session =
        RbcSession::from_persisted_unchecked(&persisted).map_err(SamplingError::Persisted)?;
    if session.total_chunks() == 0 {
        return Err(SamplingError::InvalidSampleCount);
    }
    let total_chunks = session.total_chunks();
    if count == 0 || count > total_chunks {
        return Err(SamplingError::InvalidSampleCount);
    }
    let sample_count = count;

    let mut rng = seed.map_or_else(
        || {
            let mut thread_rng = rand::rng();
            StdRng::from_rng(&mut thread_rng)
        },
        StdRng::seed_from_u64,
    );
    let mut indices: Vec<u32> = (0..total_chunks).collect();
    indices.shuffle(&mut rng);
    let sample_limit = usize::try_from(sample_count).expect("sample count fits in usize");
    indices.truncate(sample_limit);
    indices.sort_unstable();

    let digests = session
        .all_chunk_digests()
        .ok_or(SamplingError::IncompleteSession)?;
    let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests.clone());
    let mut visited_indices = BTreeSet::new();
    let mut samples = Vec::with_capacity(indices.len());
    for idx in indices {
        if !visited_indices.insert(idx) {
            continue;
        }
        let bytes = session
            .chunk_bytes(idx)
            .map(<[u8]>::to_vec)
            .ok_or(SamplingError::ProofGeneration(idx))?;
        let digest = session
            .chunk_digest(idx)
            .ok_or(SamplingError::ProofGeneration(idx))?;
        let proof = tree
            .get_proof(idx)
            .ok_or(SamplingError::ProofGeneration(idx))?;
        if proof.audit_path().len() > MAX_PROOF_HEIGHT {
            return Err(SamplingError::ProofGeneration(idx));
        }
        samples.push(ChunkSample {
            index: idx,
            bytes,
            digest,
            proof,
        });
    }

    let chunk_root = session
        .chunk_root()
        .ok_or(SamplingError::IncompleteSession)?;

    let (block_hash, height, view) = key;

    Ok(Some(SessionSample {
        block_hash,
        height,
        view,
        total_chunks,
        chunk_root,
        payload_hash: session.payload_hash(),
        samples,
    }))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iroha_crypto::Hash;
    use iroha_data_model::prelude::BlockHeader;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::{main_loop::RbcSession, rbc_store::ChunkStore};

    #[test]
    fn sampling_generates_proof_from_store() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let chunk0 = b"hello".to_vec();
        let chunk1 = b"world".to_vec();
        let digests: Vec<[u8; 32]> = [chunk0.clone(), chunk1.clone()]
            .iter()
            .map(|bytes| {
                let digest = Sha256::digest(bytes);
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&digest);
                arr
            })
            .collect();
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests.clone());
        let root_hash = Hash::from(tree.root().expect("root"));

        let mut session = RbcSession::test_new(2, None, Some(root_hash), 0);
        session.test_note_chunk(0, chunk0.clone(), 0);
        session.test_note_chunk(1, chunk1.clone(), 0);

        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1; 32])),
            5,
            0,
        );
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(300),
            4,
            1 << 20,
            8,
            1 << 20,
        )
        .expect("chunk store init");
        store
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist session");

        let sampled = sample_from_store(dir.path(), key, &chain_hash, &manifest, 1, None)
            .expect("sampling call")
            .expect("session present");

        assert_eq!(sampled.total_chunks, 2);
        assert_eq!(sampled.samples.len(), 1);
        let sample = &sampled.samples[0];
        assert!(sample.index < 2);
        let root_typed = HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(sampled.chunk_root);
        let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(sample.digest));
        assert!(
            sample
                .proof
                .clone()
                .verify_sha256(&leaf_hash, &root_typed, 16),
            "proof verifies"
        );
    }

    #[test]
    fn sampling_rejects_request_larger_than_total_chunks() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let chunk0 = b"hello".to_vec();
        let chunk1 = b"world".to_vec();
        let digests: Vec<[u8; 32]> = [chunk0.clone(), chunk1.clone()]
            .iter()
            .map(|bytes| {
                let digest = Sha256::digest(bytes);
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&digest);
                arr
            })
            .collect();
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests.clone());
        let root_hash = Hash::from(tree.root().expect("root"));

        let mut session = RbcSession::test_new(2, None, Some(root_hash), 0);
        session.test_note_chunk(0, chunk0.clone(), 0);
        session.test_note_chunk(1, chunk1.clone(), 0);

        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2; 32])),
            6,
            0,
        );
        let store = ChunkStore::new(
            dir.path().to_path_buf(),
            Duration::from_secs(300),
            4,
            1 << 20,
            8,
            1 << 20,
        )
        .expect("chunk store init");
        store
            .persist_session(key, &session, &chain_hash, &manifest, &[])
            .expect("persist session");

        let err = sample_from_store(dir.path(), key, &chain_hash, &manifest, 3, None)
            .err()
            .expect("oversized request should fail");
        assert!(matches!(err, SamplingError::InvalidSampleCount));
    }
}
