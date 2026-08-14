//! Norito roundtrip tests for Merkle structures.
//!
//! Verifies that `MerkleTree<[u8;32]>` serializes only canonical leaves plus
//! its hash scheme, rebuilds cached nodes on decode, and that proofs remain
//! lossless via the Norito codec.
use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree, MerkleTreeCommitment};
use std::num::NonZeroU64;
fn leaf_hash(payload: &[u8]) -> HashOf<[u8; 32]> {
    // Domain-tag example for TX entry leaves (not strictly required for roundtrip,
    // but keeps consistency with other Merkle tests and docs):
    const TAG_TX_ENTRY: &[u8] = b"iroha:merkle:tx_entry:v1\x00";
    let digest = Hash::new([TAG_TX_ENTRY, payload].concat());
    HashOf::from_untyped_unchecked(digest)
}
#[test]
fn merkle_tree_roundtrips_via_norito() {
    // Build a small non-perfect tree (odd number of leaves) to exercise promotion semantics.
    let leaves = [leaf_hash(b"TX1"), leaf_hash(b"TX2"), leaf_hash(b"TX3")];
    let tree: MerkleTree<[u8; 32]> = leaves.into_iter().collect();
    // Encode with Norito (header + payload)
    let bytes = norito::to_bytes(&tree).expect("encode");
    // Decode back
    let decoded: MerkleTree<[u8; 32]> = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(tree, decoded, "MerkleTree must roundtrip exactly");
    // Sanity: roots match and are Some(..)
    assert_eq!(tree.root(), decoded.root());
    assert!(decoded.root().is_some());
}
#[test]
fn merkle_proof_roundtrips_via_norito() {
    // Build a deeper tree and extract a proof for the middle leaf.
    let leaves = [
        leaf_hash(b"TX1"),
        leaf_hash(b"TX2"),
        leaf_hash(b"TX3"),
        leaf_hash(b"TX4"),
        leaf_hash(b"TX5"),
    ];
    let tree: MerkleTree<[u8; 32]> = leaves.into_iter().collect();
    // Get a proof for leaf index 2 (third leaf)
    let proof: MerkleProof<[u8; 32]> = tree.get_proof(2).expect("proof exists");
    // Encode with Norito (header + payload)
    let bytes = norito::to_bytes(&proof).expect("encode");
    // Decode back
    let decoded: MerkleProof<[u8; 32]> = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(proof, decoded, "MerkleProof must roundtrip exactly");
    // Optional verification sanity check: decoded proof still verifies
    let leaf = leaves[2];
    let commitment = tree.commitment().expect("commitment");
    assert!(
        decoded.verify(&leaf, &commitment),
        "decoded proof should verify against the exact commitment"
    );
    let wrong_count_commitment = MerkleTreeCommitment::new(
        *commitment.root(),
        NonZeroU64::new(commitment.leaf_count().get() * 2)
            .expect("wrong test count remains non-zero"),
    );
    assert!(
        !decoded.verify(&leaf, &wrong_count_commitment),
        "decoded proof must reject the same root paired with a mismatched count"
    );
}
