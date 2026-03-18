use ivm::Memory;
use sha2::{Digest, Sha256};

// Paths are ordered leaf → root; verify recomputation matches Memory::root().
#[test]
fn test_merkle_path_matches_root() {
    let mut mem = Memory::new(0);
    // Write a value into heap and fetch the merkle path for that address
    let addr = Memory::HEAP_START;
    mem.store_u32(addr, 0xAA55AA55).unwrap();
    mem.commit();
    let path = mem.merkle_path(addr);
    let root = *mem.root().as_ref();
    // Recompute root from leaf and path
    const CHUNK: usize = 32;
    let mut leaf_data = [0u8; CHUNK];
    leaf_data[..4].copy_from_slice(&0xAA55AA55u32.to_le_bytes());
    let mut current: [u8; 32] = Sha256::digest(leaf_data).into();
    let mut idx = (addr as usize) / CHUNK;
    for sib in path.iter() {
        // All‑zero sibling denotes a missing node; promote the existing child.
        let zero = [0u8; 32];
        let is_even = idx.is_multiple_of(2);
        match (is_even, *sib == zero) {
            (true, true) => { /* left child only */ }
            (false, true) => {
                // invalid path shape (left missing); force mismatch
                current = [0u8; 32];
            }
            _ => {
                let (mut left, mut right) = if is_even {
                    (current, *sib)
                } else {
                    (*sib, current)
                };
                // Match the parent hash semantics of iroha_crypto: set the LSB bit of each
                // child digest (prehashed marker) before computing SHA-256(left||right).
                left[31] |= 1;
                right[31] |= 1;
                let mut h = Sha256::new();
                h.update(left);
                h.update(right);
                current = h.finalize().into();
            }
        }
        idx /= 2;
    }
    // Root in ByteMerkleTree is stored as Hash::prehashed, so set the marker bit.
    current[31] |= 1;
    assert_eq!(current, root);
}
