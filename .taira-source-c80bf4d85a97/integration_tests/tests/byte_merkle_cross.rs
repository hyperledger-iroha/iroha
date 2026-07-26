#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Cross-check that IVM byte-merkle root matches the canonical
//! `iroha_crypto::MerkleTree` root for the same input.

use iroha_crypto::MerkleTree;

#[test]
fn ivm_byte_merkle_matches_crypto_helpers() {
    let data: Vec<u8> = (0..77u8).collect();
    let ivm_root = ivm::ByteMerkleTree::from_bytes(&data, 32).root();
    let crypto_tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
    let crypto_root = crypto_tree.root().unwrap();
    assert_eq!(ivm_root, *crypto_root.as_ref());
}
