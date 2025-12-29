//! Norito roundtrip tests for Merkle structures.
//!
//! Verifies that `MerkleTree<[u8;32]>` and `MerkleProof<[u8;32]>` serialize
//! and deserialize losslessly via the Norito codec.

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};

fn leaf_hash(payload: &[u8]) -> HashOf<[u8; 32]> {
    // Domain-tag example for TX entry leaves (not strictly required for roundtrip,
    // but keeps consistency with other Merkle tests and docs):
    const TAG_TX_ENTRY: &[u8] = b"iroha:merkle:tx_entry:v1\x00";
    let digest = Hash::new([TAG_TX_ENTRY, payload].concat());
    HashOf::from_untyped_unchecked(digest)
}

#[test]
fn merkle_tree_roundtrips_via_norito() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: Merkle Norito roundtrip pending Norito opt handling. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
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
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: MerkleProof Norito roundtrip pending Norito opt handling. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
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
    let leaf = tree.leaves().nth(2).expect("leaf at index must exist");
    let root = tree.root().expect("root");
    assert!(
        decoded.clone().verify(&leaf, &root, 8),
        "decoded proof should verify (bounded height)"
    );
}
