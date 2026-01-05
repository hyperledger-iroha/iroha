//! Cross-crate Merkle compatibility: ensures `ivm` and `iroha_crypto`
//! produce identical roots and proofs for the same byte data.

use iroha_crypto::{HashOf, MerkleTree};

#[test]
fn byte_merkle_roots_and_proofs_match_across_crates() {
    // Prepare deterministic input bytes
    let data: Vec<u8> = (0..150u32).map(|i| (i % 251) as u8).collect();

    // Build via iroha_crypto
    let tree_crypto: MerkleTree<[u8; 32]> =
        MerkleTree::from_byte_chunks(&data, 32).expect("valid chunk");
    let root_crypto: HashOf<MerkleTree<[u8; 32]>> = tree_crypto.root().unwrap();

    // Build via IVM re-export (type alias to the same canonical type)
    let tree_ivm: ivm::MerkleTree<[u8; 32]> =
        ivm::MerkleTree::from_byte_chunks(&data, 32).expect("valid chunk");
    let root_ivm = tree_ivm.root().unwrap();

    assert_eq!(root_crypto, root_ivm, "roots must match across crates");

    // Take a proof and verify against the other tree's root
    let idx = 2u32;
    let proof = tree_crypto.get_proof(idx).expect("proof exists");
    let leaf = tree_crypto.leaves().nth(idx as usize).unwrap();
    assert!(
        proof.clone().verify_sha256(&leaf, &root_ivm, 9),
        "proof must verify against ivm root"
    );
    assert!(
        proof.verify_sha256(&leaf, &root_crypto, 9),
        "proof must verify against crypto root"
    );
}
