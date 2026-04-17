//! Deterministic IVM memory Merkle proof regression tests.

use iroha_crypto::{Hash, HashOf};
use ivm::Memory;
use sha2::Digest as _;

const CHUNK_BYTES: usize = 32;
const MAX_INPUT_CHUNKS: usize = (Memory::INPUT_SIZE as usize) / CHUNK_BYTES;

fn leaf_hash(bytes: &[u8; CHUNK_BYTES]) -> HashOf<[u8; CHUNK_BYTES]> {
    let digest = sha2::Sha256::digest(bytes);
    let mut arr = [0u8; CHUNK_BYTES];
    arr.copy_from_slice(&digest);
    HashOf::from_untyped_unchecked(Hash::prehashed(arr))
}

fn deterministic_chunk(chunk_idx: usize, seed: u8) -> [u8; CHUNK_BYTES] {
    let mut data = [0u8; CHUNK_BYTES];
    for (idx, byte) in data.iter_mut().enumerate() {
        *byte = seed
            .wrapping_add(idx as u8)
            .wrapping_add((chunk_idx as u8).wrapping_mul(17));
    }
    data
}

#[test]
fn memory_merkle_compact_proofs_verify_for_deterministic_chunks() {
    let chunk_indices = [0, 1, MAX_INPUT_CHUNKS / 2, MAX_INPUT_CHUNKS - 1];
    let depth_caps = [None, Some(0), Some(1), Some(4), Some(10)];

    for chunk_idx in chunk_indices {
        for depth_cap in depth_caps {
            let data = deterministic_chunk(chunk_idx, depth_cap.unwrap_or(13) as u8);
            let mut memory = Memory::new(0);
            let offset = (chunk_idx * CHUNK_BYTES) as u64;
            memory
                .preload_input(offset, &data)
                .expect("preload input chunk");
            memory.commit();

            let addr = Memory::INPUT_START + offset;
            let (proof, root) = memory.merkle_compact(addr, depth_cap);
            let leaf = leaf_hash(&data);

            assert!(proof.clone().verify_sha256(&leaf, &root));

            let mut tampered = data;
            tampered[0] ^= 0x01;
            let tampered_leaf = leaf_hash(&tampered);
            assert!(!proof.verify_sha256(&tampered_leaf, &root));
        }
    }
}
