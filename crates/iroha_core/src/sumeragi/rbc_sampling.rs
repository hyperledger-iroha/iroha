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
    /// Random sampling RNG could not be seeded from OS entropy.
    #[error("RBC chunk sampling RNG seed failed: {0}")]
    RandomSeed(String),
}

fn sampling_rng(seed: Option<u64>) -> Result<StdRng, SamplingError> {
    match seed {
        Some(seed) => Ok(StdRng::seed_from_u64(seed)),
        None => {
            StdRng::try_from_os_rng().map_err(|error| SamplingError::RandomSeed(error.to_string()))
        }
    }
}

#[cfg(test)]
fn sampling_rng_from_rng<R: rand::rand_core::TryCryptoRng>(
    rng: &mut R,
) -> Result<StdRng, SamplingError> {
    StdRng::try_from_rng(rng).map_err(|error| SamplingError::RandomSeed(error.to_string()))
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

    let mut rng = sampling_rng(seed)?;
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
    use std::{fmt, path::Path, time::Duration};

    use iroha_crypto::Hash;
    use iroha_data_model::prelude::BlockHeader;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::*;
    use crate::sumeragi::{main_loop::RbcSession, rbc_store::ChunkStore};

    struct FailingSamplingRng;

    #[derive(Debug)]
    struct FailingSamplingRngError;

    impl fmt::Display for FailingSamplingRngError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("failing RBC sampling RNG")
        }
    }

    impl rand::rand_core::TryRngCore for FailingSamplingRng {
        type Error = FailingSamplingRngError;

        fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
            Err(FailingSamplingRngError)
        }

        fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
            Err(FailingSamplingRngError)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> std::result::Result<(), Self::Error> {
            Err(FailingSamplingRngError)
        }
    }

    impl rand::rand_core::TryCryptoRng for FailingSamplingRng {}

    fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
        let digest = Sha256::digest(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn chunk_root_for(chunks: &[Vec<u8>]) -> Hash {
        let digests: Vec<[u8; 32]> = chunks.iter().map(|chunk| digest_bytes(chunk)).collect();
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests);
        Hash::from(tree.root().expect("chunk root"))
    }

    fn chunk_store_at(dir: &Path) -> ChunkStore {
        ChunkStore::new(
            dir.to_path_buf(),
            Duration::from_secs(300),
            4,
            1 << 20,
            8,
            1 << 20,
        )
        .expect("chunk store init")
    }

    fn persist_session_for_sampling(
        dir: &Path,
        key: SessionKey,
        chain_hash: &Hash,
        manifest: &SoftwareManifest,
        session: &RbcSession,
    ) {
        chunk_store_at(dir)
            .persist_session(key, session, chain_hash, manifest, &[])
            .expect("persist session");
    }

    #[test]
    fn sampling_rng_reports_seed_failure() {
        let mut rng = FailingSamplingRng;

        let error =
            sampling_rng_from_rng(&mut rng).expect_err("failing RNG must report seed failure");

        assert!(
            matches!(error, SamplingError::RandomSeed(message) if message.contains("failing RBC sampling RNG"))
        );
    }

    #[test]
    fn sampling_returns_none_when_session_is_absent() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; 32])),
            7,
            0,
        );

        let sampled = sample_from_store(dir.path(), key, &chain_hash, &manifest, 1, Some(1))
            .expect("sampling absent session should not fail");

        assert!(sampled.is_none());
    }

    #[test]
    fn sampling_generates_proof_from_store() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let chunk0 = b"hello".to_vec();
        let chunk1 = b"world".to_vec();
        let chunks = vec![chunk0.clone(), chunk1.clone()];
        let root_hash = chunk_root_for(&chunks);

        let mut session = RbcSession::test_new(2, None, Some(root_hash), 0);
        session.test_note_chunk(0, chunk0.clone(), 0);
        session.test_note_chunk(1, chunk1.clone(), 0);

        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([1; 32])),
            5,
            0,
        );
        persist_session_for_sampling(dir.path(), key, &chain_hash, &manifest, &session);

        let sampled = sample_from_store(dir.path(), key, &chain_hash, &manifest, 1, None)
            .expect("sampling call")
            .expect("session present");

        assert_eq!(sampled.block_hash, key.0);
        assert_eq!(sampled.height, key.1);
        assert_eq!(sampled.view, key.2);
        assert_eq!(sampled.total_chunks, 2);
        assert_eq!(sampled.chunk_root, root_hash);
        assert_eq!(sampled.payload_hash, None);
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
    fn sampling_uses_seeded_unique_sorted_indices_and_exact_metadata() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let chunks = vec![
            b"alpha".to_vec(),
            b"bravo".to_vec(),
            b"charlie".to_vec(),
            b"delta".to_vec(),
            b"echo".to_vec(),
        ];
        let mut payload = Vec::new();
        for chunk in &chunks {
            payload.extend_from_slice(chunk);
        }
        let payload_hash = Hash::new(&payload);
        let root_hash = chunk_root_for(&chunks);
        let mut session = RbcSession::test_new(
            u32::try_from(chunks.len()).expect("chunk count fits"),
            Some(payload_hash),
            Some(root_hash),
            0,
        );
        for (idx, chunk) in chunks.iter().enumerate() {
            session.test_note_chunk(
                u32::try_from(idx).expect("chunk index fits"),
                chunk.clone(),
                0,
            );
        }

        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA2; 32])),
            8,
            2,
        );
        persist_session_for_sampling(dir.path(), key, &chain_hash, &manifest, &session);

        let first = sample_from_store(dir.path(), key, &chain_hash, &manifest, 3, Some(99))
            .expect("seeded sampling call")
            .expect("session present");
        let second = sample_from_store(dir.path(), key, &chain_hash, &manifest, 3, Some(99))
            .expect("second seeded sampling call")
            .expect("session present");

        assert_eq!(first.block_hash, key.0);
        assert_eq!(first.height, key.1);
        assert_eq!(first.view, key.2);
        assert_eq!(first.total_chunks, chunks.len() as u32);
        assert_eq!(first.chunk_root, root_hash);
        assert_eq!(first.payload_hash, Some(payload_hash));
        assert_eq!(first.samples.len(), 3);

        let indices: Vec<u32> = first.samples.iter().map(|sample| sample.index).collect();
        let second_indices: Vec<u32> = second.samples.iter().map(|sample| sample.index).collect();
        assert_eq!(indices, second_indices);
        assert!(
            indices.windows(2).all(|window| window[0] < window[1]),
            "sampled indices must be sorted and unique: {indices:?}"
        );
        assert!(indices.iter().all(|idx| *idx < chunks.len() as u32));

        let root_typed = HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(first.chunk_root);
        for sample in &first.samples {
            let idx = usize::try_from(sample.index).expect("sample index fits");
            assert_eq!(sample.bytes, chunks[idx]);
            assert_eq!(sample.digest, digest_bytes(&chunks[idx]));
            let leaf_hash =
                HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(sample.digest));
            assert!(
                sample
                    .proof
                    .clone()
                    .verify_sha256(&leaf_hash, &root_typed, 16),
                "proof verifies for sampled chunk {}",
                sample.index
            );
        }
    }

    #[test]
    fn sampling_rejects_zero_requested_chunks() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let chunk = b"hello".to_vec();
        let root_hash = chunk_root_for(std::slice::from_ref(&chunk));
        let mut session = RbcSession::test_new(1, None, Some(root_hash), 0);
        session.test_note_chunk(0, chunk, 0);
        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA3; 32])),
            9,
            0,
        );
        persist_session_for_sampling(dir.path(), key, &chain_hash, &manifest, &session);

        let err = sample_from_store(dir.path(), key, &chain_hash, &manifest, 0, Some(1))
            .expect_err("zero sample count should fail");

        assert!(matches!(err, SamplingError::InvalidSampleCount));
    }

    #[test]
    fn sampling_rejects_incomplete_persisted_session() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let mut session = RbcSession::test_new(3, None, None, 0);
        session.test_note_chunk(0, b"only-one-chunk".to_vec(), 0);
        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA4; 32])),
            10,
            0,
        );
        persist_session_for_sampling(dir.path(), key, &chain_hash, &manifest, &session);

        let err = sample_from_store(dir.path(), key, &chain_hash, &manifest, 1, Some(1))
            .expect_err("incomplete sessions cannot provide Merkle proofs");

        assert!(matches!(err, SamplingError::IncompleteSession));
    }

    #[test]
    fn sampling_rejects_chain_mismatched_snapshot_without_accepting_later() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let other_chain_hash = Hash::new(b"other-chain");
        let manifest = SoftwareManifest::current();
        let chunk = b"hello".to_vec();
        let root_hash = chunk_root_for(std::slice::from_ref(&chunk));
        let mut session = RbcSession::test_new(1, None, Some(root_hash), 0);
        session.test_note_chunk(0, chunk, 0);
        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32])),
            11,
            0,
        );
        persist_session_for_sampling(dir.path(), key, &chain_hash, &manifest, &session);

        let sampled = sample_from_store(dir.path(), key, &other_chain_hash, &manifest, 1, Some(1))
            .expect("chain mismatch should be treated as an unusable persisted snapshot");
        assert!(sampled.is_none());

        let sampled_after_delete =
            sample_from_store(dir.path(), key, &chain_hash, &manifest, 1, Some(1))
                .expect("sampling after rejected snapshot");
        assert!(
            sampled_after_delete.is_none(),
            "strict guard should remove the mismatched snapshot instead of accepting it later"
        );
    }

    #[test]
    fn sampling_rejects_request_larger_than_total_chunks() {
        let dir = tempdir().unwrap();
        let chain_hash = Hash::new(b"chain");
        let manifest = SoftwareManifest::current();
        let chunk0 = b"hello".to_vec();
        let chunk1 = b"world".to_vec();
        let chunks = vec![chunk0.clone(), chunk1.clone()];
        let root_hash = chunk_root_for(&chunks);

        let mut session = RbcSession::test_new(2, None, Some(root_hash), 0);
        session.test_note_chunk(0, chunk0.clone(), 0);
        session.test_note_chunk(1, chunk1.clone(), 0);

        let key = (
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([2; 32])),
            6,
            0,
        );
        persist_session_for_sampling(dir.path(), key, &chain_hash, &manifest, &session);

        let err = sample_from_store(dir.path(), key, &chain_hash, &manifest, 3, None)
            .expect_err("oversized request should fail");
        assert!(matches!(err, SamplingError::InvalidSampleCount));
    }
}
