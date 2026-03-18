#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{
    MerkleCircuit, MerklePublic, MerkleWitness, verify_merkle_path, verify_merkle_path_with_dirs,
};

#[test]
fn test_merkle_circuit_verify_ok() {
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = i as u64 + 1;
    }
    let witness = MerkleWitness {
        leaf: 5,
        index: 3,
        path,
    };
    let public = MerklePublic { root: 0 }; // will set below
    let mut circuit = MerkleCircuit::new(witness, public);
    circuit.public.root = verify_merkle_path(
        circuit.witness.leaf,
        circuit.witness.index,
        &circuit.witness.path,
    );
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_merkle_circuit_bad_root() {
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = i as u64 + 1;
    }
    let witness = MerkleWitness {
        leaf: 5,
        index: 3,
        path,
    };
    let public = MerklePublic { root: 123 }; // incorrect root
    let circuit = MerkleCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_merkle_verify_with_dirs_matches_index() {
    // Construct a simple synthetic path and compare the index-based and
    // direction-bit based verifiers for a few depths.
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = (i as u64).wrapping_mul(17).wrapping_add(5);
    }
    let leaf = 12345u64;
    let index = 0b_10110u32; // arbitrary index bits
    // Compute roots using both index-based and dirs-based verifiers at
    // different depths, and ensure they agree at each requested depth.
    // The index-based helper consumes exactly `depth` levels for parity.

    // Build dirs bitmask from index and compare at multiple depths
    for depth in [8usize, 16, 32] {
        let dirs = index; // lower bits encode directions per level
        let root_dirs = verify_merkle_path_with_dirs(leaf, dirs, &path, depth);
        let root_idx = ivm::halo2::verify_merkle_path_depth(leaf, index, &path, depth);
        assert_eq!(root_idx, root_dirs);
    }
}

#[test]
fn test_merkle_verify_with_dirs_rejects_wrong_dirs() {
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = (i as u64).wrapping_mul(11).wrapping_add(7);
    }
    let leaf = 4242u64;
    let index = 0b_00101u32;
    let root_idx = verify_merkle_path(leaf, index, &path);

    // Flip one direction bit; result must differ from the index-based root
    let dirs_wrong = index ^ 0b_00100u32; // flip bit at level 2
    let root_dirs = verify_merkle_path_with_dirs(leaf, dirs_wrong, &path, 8);
    assert_ne!(root_idx, root_dirs);
}
