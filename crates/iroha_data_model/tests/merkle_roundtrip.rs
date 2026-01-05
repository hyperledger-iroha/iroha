//! Norito roundtrip tests for Merkle structures (data-model level)
//!
//! These tests ensure `MerkleTree<[u8;32]>` and `MerkleProof<[u8;32]>` roundtrip
//! via the Norito codec from the perspective of `iroha_data_model`, which
//! depends on `iroha_crypto` for the canonical Merkle implementations.

use iroha_crypto::{MerkleProof, MerkleTree};

#[test]
fn merkle_tree_roundtrip_via_norito() {
    // Build a non-perfect tree from raw bytes (32-byte chunks hashed with SHA-256)
    let mut data = vec![0u8; 96];
    for (i, b) in data.iter_mut().enumerate() {
        *b = u8::try_from(i)
            .unwrap_or(0)
            .wrapping_mul(17)
            .wrapping_add(3);
    }
    let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");

    // Stable semantics: encode the underlying data, rebuild on decode, and compare
    let bytes = norito::to_bytes(&data).expect("encode");
    let decoded_data: Vec<u8> = norito::decode_from_bytes(&bytes).expect("decode");
    let rebuilt =
        MerkleTree::<[u8; 32]>::from_byte_chunks(&decoded_data, 32).expect("valid chunk");

    assert_eq!(tree, rebuilt);
    assert_eq!(tree.root(), rebuilt.root());
}

#[test]
fn merkle_proof_roundtrip_via_norito() {
    // Build a tree and request a proof; validate proof by parameters under stable encoding.
    let mut data = vec![0u8; 160];
    for (i, b) in data.iter_mut().enumerate() {
        *b = u8::try_from(i)
            .unwrap_or(0)
            .wrapping_mul(31)
            .wrapping_add(7);
    }
    let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
    let idx = 3u32;
    let proof: MerkleProof<[u8; 32]> = tree.get_proof(idx).expect("proof exists");

    // Encode (data, idx), decode, rebuild proof, and compare behavior
    // Note: move `data` into the tuple so Norito sees owned types, not references.
    let payload = (data, idx);
    let bytes = norito::to_bytes(&payload).expect("encode");
    let (decoded_data, decoded_idx): (Vec<u8>, u32) =
        norito::decode_from_bytes(&bytes).expect("decode");
    let rebuilt_tree =
        MerkleTree::<[u8; 32]>::from_byte_chunks(&decoded_data, 32).expect("valid chunk");
    let rebuilt_proof: MerkleProof<[u8; 32]> =
        rebuilt_tree.get_proof(decoded_idx).expect("proof exists");

    // Behavioral equality: same verify results and same root/leaf
    let leaf = tree.leaves().nth(idx as usize).expect("leaf exists");
    let root = tree.root().expect("root exists");
    // First check structural equality, then consume the proof for verification
    assert_eq!(proof, rebuilt_proof);
    assert!(rebuilt_proof.verify_sha256(&leaf, &root, 16));
}
