use ivm::{MerkleTree, Registers};
use sha2::{Digest, Sha256};

fn reg_leaf(value: u64, tag: bool) -> [u8; 32] {
    let mut bytes = [0u8; 9];
    bytes[0] = if tag { 1 } else { 0 };
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(bytes));
    out
}

#[test]
fn registers_merkle_root_matches_canonical() {
    let mut regs = Registers::new();
    // set a couple of registers and tags
    regs.set(10, 0xdead_beef_dead_beefu64);
    regs.set_tag(10, true);
    regs.set(123, 42);
    regs.set_tag(123, false);

    // Build expected leaves (256-register file)
    let mut leaves = vec![reg_leaf(0, false); 256];
    leaves[10] = reg_leaf(0xdead_beef_dead_beefu64, true);
    leaves[123] = reg_leaf(42, false);
    let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves);
    let expected_root = tree.root().unwrap();

    assert_eq!(regs.merkle_root(), expected_root);
}

#[test]
fn registers_root_and_path_combined_matches_separate() {
    let mut regs = Registers::new();
    // Modify a few registers and tags to ensure non-trivial paths
    regs.set(5, 123456789);
    regs.set_tag(5, true);
    regs.set(77, 0xA5A5);
    regs.set_tag(77, false);

    for &idx in &[0usize, 5, 17, 77, 255] {
        let path_s = regs.merkle_path(idx);
        let root_s = regs.merkle_root();
        let (root_c, path_c) = regs.merkle_root_and_path(idx);
        assert_eq!(root_s, root_c, "root mismatch at idx={idx}");
        assert_eq!(path_s, path_c, "path mismatch at idx={idx}");
    }
}
