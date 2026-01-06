//! Reconstruct canonical Merkle proofs from IVM sibling paths and
//! verify them against `iroha_crypto` roots.

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};

fn path_to_audit_path(path: &[[u8; 32]]) -> Vec<Option<HashOf<[u8; 32]>>> {
    path.iter()
        .map(|sib| {
            if sib.iter().all(|&b| b == 0) {
                None
            } else {
                Some(HashOf::from_untyped_unchecked(Hash::prehashed(*sib)))
            }
        })
        .collect()
}

#[test]
fn construct_proof_from_ivm_path_and_verify() {
    let data: Vec<u8> = (0..77u8).collect();
    let chunk = 32;
    let ivm_tree = ivm::ByteMerkleTree::from_bytes(&data, chunk);
    let crypto_tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk).expect("valid chunk");
    let root = crypto_tree.root().unwrap();

    for idx in 0..data.len().div_ceil(chunk) {
        let path = ivm_tree.path(idx);
        let audit_path = path_to_audit_path(&path);
        let proof = MerkleProof::<[u8; 32]>::from_audit_path(
            u32::try_from(idx).expect("index fits u32"),
            audit_path,
        );
        let leaf = crypto_tree.leaves().nth(idx).unwrap();
        assert!(
            proof.verify_sha256(&leaf, &root, 9),
            "proof must verify at {idx}"
        );
    }
}
