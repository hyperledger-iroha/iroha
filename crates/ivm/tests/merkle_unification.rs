//! Cross-crate Merkle unification tests
//!
//! These tests ensure that IVM’s Merkle helpers (`ByteMerkleTree` and
//! register-state wrapper) are equivalent to the canonical
//! `iroha_crypto::MerkleTree` implementation in roots and proofs.

use iroha_crypto::{MerkleProof, MerkleTree};
use ivm::ByteMerkleTree;
// No extra helpers needed in this test; keep dependencies minimal.

#[test]
fn byte_tree_parallel_cross_crate_equivalence() {
    // Larger corpus to exercise parallel chunking
    let data: Vec<u8> = (0..10_000u32)
        .map(|i| ((i * 131_071) % 251) as u8)
        .collect();
    let chunk = 32;

    // Canonical sequential vs parallel must match
    let canonical_seq = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk)
        .expect("valid chunk");
    let canonical_par = MerkleTree::<[u8; 32]>::from_chunked_bytes_parallel(&data, chunk)
        .expect("valid chunk");
    assert_eq!(canonical_seq.root(), canonical_par.root());

    // Cross-crate: IVM helpers (seq and par) must match canonical
    let ivm_seq = ByteMerkleTree::from_bytes(&data, chunk);
    let ivm_par = ByteMerkleTree::from_bytes_parallel(&data, chunk);

    let root_seq = canonical_seq.root().expect("root");
    assert_eq!(root_seq.as_ref(), &ivm_seq.root());
    assert_eq!(root_seq.as_ref(), &ivm_par.root());
}

#[test]
fn byte_tree_root_and_proof_equivalence() {
    // Arbitrary data; covers multiple chunk paths, including a partial tail.
    let data: Vec<u8> = (0..150u32).map(|i| (i % 251) as u8).collect();
    let chunk = 32;

    // Canonical tree from iroha_crypto
    let canonical =
        MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk).expect("valid chunk");
    let canonical_root = canonical.root().expect("root");

    // IVM byte-chunk helper
    let ivm_tree = ByteMerkleTree::from_bytes(&data, chunk);
    let ivm_root = ivm_tree.root();

    // Roots must match exactly
    assert_eq!(canonical_root.as_ref(), &ivm_root);

    // Pick a few indices (including boundaries) and compare proof shapes.
    let indices = [0usize, 1, 2, (data.len() / chunk).max(1) - 1];
    for &idx in &indices {
        // IVM path as raw sibling bytes
        let ivm_path = ivm_tree.path(idx);
        // Wrap into a canonical proof and verify it
        let proof = MerkleProof::from_audit_path_bytes(idx as u32, ivm_path.clone());
        let leaf = canonical
            .leaves()
            .nth(idx)
            .expect("leaf must exist for valid index");
        assert!(proof.clone().verify_sha256(&leaf, &canonical_root, 16));

        // Now produce a canonical proof and compare with IVM path byte-wise.
        let canon_proof = canonical.get_proof(idx as u32).expect("proof");
        let canon_path_bytes: Vec<[u8; 32]> = canon_proof
            .audit_path()
            .iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        assert_eq!(canon_path_bytes, ivm_path, "paths differ at idx={idx}");
    }
}
