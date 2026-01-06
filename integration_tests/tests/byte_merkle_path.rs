//! Validate that IVM Merkle sibling paths correspond 1:1 with the
//! canonical `iroha_crypto::MerkleProof` audit path representation.

use iroha_crypto::{Hash, MerkleTree};
use sha2::{Digest, Sha256};

fn proof_to_siblings(proof: iroha_crypto::MerkleProof<[u8; 32]>) -> Vec<[u8; 32]> {
    proof
        .into_audit_path()
        .into_iter()
        .map(|opt| opt.map_or([0u8; 32], |h| *h.as_ref()))
        .collect()
}

fn recompute_root_from_path(leaf: [u8; 32], path: &[[u8; 32]], leaf_idx: usize) -> [u8; 32] {
    // The MerkleProof in iroha_crypto is constructed relative to the
    // complete binary tree indexing in breadth-first order. The parity of
    // the node index at each level depends on the leaf index plus an offset
    // of (2^height - 1). Mirror that here so the concatenation order matches
    // the proof semantics.
    let height = path.len();
    let mut index = ((1usize << height) - 1).saturating_add(leaf_idx);

    let mut acc = leaf;
    for sib in path {
        let sib_opt = if sib.iter().all(|&b| b == 0) {
            None
        } else {
            Some(*sib)
        };
        // Match iroha_crypto::MerkleProof::verify_sha256 ordering and prehashing:
        // even index -> (l = prehashed(sibling), r = prehashed(acc))
        // odd index  -> (l = prehashed(acc),    r = prehashed(sibling))
        match (index % 2, sib_opt) {
            (0, None) => unreachable!("malformed proof: missing left sibling at even index"),
            (0, Some(l)) => {
                let mut buf = [0u8; 64];
                // prehash both children before concatenation
                let l_pre = *Hash::prehashed(l).as_ref();
                let r_pre = *Hash::prehashed(acc).as_ref();
                buf[..32].copy_from_slice(&l_pre);
                buf[32..].copy_from_slice(&r_pre);
                let mut out = [0u8; 32];
                out.copy_from_slice(&Sha256::digest(buf));
                acc = out;
            }
            (1, None) => {
                // promote left child when right sibling is missing
                // (no hashing occurs), but bytes are the prehashed child bytes
                acc = *Hash::prehashed(acc).as_ref();
            }
            (1, Some(r)) => {
                let mut buf = [0u8; 64];
                let l_pre = *Hash::prehashed(acc).as_ref();
                let r_pre = *Hash::prehashed(r).as_ref();
                buf[..32].copy_from_slice(&l_pre);
                buf[32..].copy_from_slice(&r_pre);
                let mut out = [0u8; 32];
                out.copy_from_slice(&Sha256::digest(buf));
                acc = out;
            }
            _ => unreachable!(),
        }
        index = (index.saturating_sub(1)) >> 1;
    }
    // Canonical root bytes are stored as Hash with LSB set
    *Hash::prehashed(acc).as_ref()
}

#[test]
fn ivm_paths_match_canonical_proofs_even_leaf_count() {
    // 4 chunks (3 full + 1 padded)
    let data: Vec<u8> = (0..100u32).map(|i| (i % 251) as u8).collect();
    let chunk = 32;
    let ivm_tree = ivm::ByteMerkleTree::from_bytes(&data, chunk);
    let crypto_tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk).expect("valid chunk");

    for &idx in &[0usize, 1, 3] {
        let path_ivm = ivm_tree.path(idx);
        let proof = crypto_tree
            .get_proof(u32::try_from(idx).expect("index fits u32"))
            .expect("proof");
        let siblings = proof_to_siblings(proof);
        assert_eq!(path_ivm, siblings, "mismatch at index {idx}");

        // Cross-check: IVM root equals canonical root and path verifies
        let root_crypto = crypto_tree.root().unwrap();
        assert_eq!(ivm_tree.root(), *root_crypto.as_ref());
        // Note: proof verification covered elsewhere; here we only check
        // sibling path identity and root equality.
        // We deliberately avoid recomputing roots manually here; canonical
        // verification is covered in iroha_crypto tests and in
        // integration_tests/tests/proof_from_path.rs
    }
}

#[test]
fn ivm_paths_match_canonical_proofs_odd_leaf_count() {
    // 3 chunks (2 full + 1 padded) ensures missing-right sibling at some levels
    let data: Vec<u8> = (0..80u32).map(|i| (i % 200) as u8).collect();
    let chunk = 32;
    let ivm_tree = ivm::ByteMerkleTree::from_bytes(&data, chunk);
    let crypto_tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk).expect("valid chunk");

    for &idx in &[0usize, 2] {
        let path_ivm = ivm_tree.path(idx);
        let proof = crypto_tree
            .get_proof(u32::try_from(idx).expect("index fits u32"))
            .expect("proof");
        let siblings = proof_to_siblings(proof);
        assert_eq!(path_ivm, siblings, "mismatch at index {idx}");

        // Cross-check: IVM root equals canonical root and path verifies
        let root_crypto = crypto_tree.root().unwrap();
        assert_eq!(ivm_tree.root(), *root_crypto.as_ref());
        // Note: proof verification covered elsewhere; here we only check
        // sibling path identity and root equality.
        // We deliberately avoid recomputing roots manually here; canonical
        // verification is covered in iroha_crypto tests and in
        // integration_tests/tests/proof_from_path.rs
    }
}

#[test]
fn tampering_leaf_or_sibling_breaks_root() {
    // Ensure non-empty path: at least 2 leaves
    let data: Vec<u8> = (0..70u32).map(|i| (i % 239) as u8).collect();
    let chunk = 32;
    let ivm_tree = ivm::ByteMerkleTree::from_bytes(&data, chunk);
    let crypto_tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk).expect("valid chunk");

    let idx = 0usize;
    let path_ivm = ivm_tree.path(idx);
    assert!(!path_ivm.is_empty(), "expect at least one sibling in path");

    // Tamper with the first sibling
    let mut tampered = path_ivm.clone();
    tampered[0][0] ^= 0x01;
    let leaf = *crypto_tree.leaves().nth(idx).unwrap().as_ref();
    let recomputed = recompute_root_from_path(leaf, &tampered, idx);
    assert_ne!(
        recomputed,
        ivm_tree.root(),
        "tampered path should not verify"
    );

    // Tamper with the leaf
    let mut tampered_leaf = leaf;
    tampered_leaf[0] ^= 0x01;
    let recomputed2 = recompute_root_from_path(tampered_leaf, &path_ivm, idx);
    assert_ne!(
        recomputed2,
        ivm_tree.root(),
        "tampered leaf should not verify"
    );
}
