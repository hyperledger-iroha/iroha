//! Parity test for Merkle inclusion gadget using direction bits vs index-based
//! variant while exercising the Halo2-backed field types.

use ivm::halo2::{verify_merkle_path_depth, verify_merkle_path_with_dirs};

#[test]
fn merkle_dirs_parity_backend() {
    // Synthetic path values; actual values are arbitrary field elements
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = (i as u64).wrapping_mul(31).wrapping_add(7);
    }
    let leaf = 0xDEAD_BEEF_u64;
    let index = 0b_1010_0110_0011u32;

    for depth in [8usize, 16, 32] {
        let dirs = index; // lower bits encode directions per level
        let r_dirs = verify_merkle_path_with_dirs(leaf, dirs, &path, depth);
        let r_idx = verify_merkle_path_depth(leaf, index, &path, depth);
        assert_eq!(r_dirs, r_idx, "dirs vs index parity at depth {depth}");
    }
}
