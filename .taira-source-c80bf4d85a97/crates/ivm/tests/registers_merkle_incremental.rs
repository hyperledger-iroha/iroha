//! Conformance tests for the Registers Merkle incremental path vs canonical rebuild.
//! These tests run under the `merkle_incremental` feature and ensure equivalence
//! of roots and paths with a fresh canonical rebuild from cached leaves.

#[cfg(feature = "merkle_incremental")]
use iroha_crypto::MerkleTree;
#[cfg(feature = "merkle_incremental")]
use ivm::Registers;

#[cfg(feature = "merkle_incremental")]
fn register_leaf_digest(value: u64, tag: bool) -> [u8; 32] {
    let mut bytes = [0u8; 9];
    bytes[0] = if tag { 1 } else { 0 };
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    sha2::Sha256::digest(&bytes).into()
}

#[cfg(feature = "merkle_incremental")]
#[test]
fn registers_incremental_matches_canonical_rebuild() {
    let mut regs = Registers::new();
    let mut gpr = [0u64; 512];
    let mut tags = [false; 512];

    // Perform a sequence of mixed writes and tag updates
    for i in 1..=2000u32 {
        let idx = (i as usize) % 512;
        if i % 7 == 0 {
            tags[idx] = !tags[idx];
            regs.set_tag(idx, tags[idx]);
        } else {
            gpr[idx] = gpr[idx].wrapping_add(i as u64);
            regs.set(idx, gpr[idx]);
        }
    }

    // Build canonical tree from cached leaf digests
    let leaves: Vec<[u8; 32]> = gpr
        .iter()
        .zip(tags.iter())
        .map(|(&v, &t)| register_leaf_digest(v, t))
        .collect();
    let canonical = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
    let root = canonical.root().expect("root");

    // Root equality
    assert_eq!(root, regs.merkle_root());

    // Path equality at a few indices
    for &idx in &[0usize, 3, 17, 255, 511] {
        let proof = canonical.get_proof(idx as u32).expect("proof");
        let path_bytes: Vec<[u8; 32]> = proof
            .audit_path()
            .iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        assert_eq!(path_bytes, regs.merkle_path(idx), "path mismatch at {idx}");
    }
}
