//! Original fixed backing, padded/truncated input and mutation-free acceptance.

use super::*;
use crate::{byte_merkle_tree::canonical_nodes, vector::SimdChoice};
use iroha_crypto::MerkleTree;

fn scalar_context() -> Sha256Context {
    let previous = crate::vector::set_thread_forced_simd(Some(SimdChoice::Scalar));
    let context = Sha256Context::production();
    crate::vector::set_thread_forced_simd(previous);
    context
}

#[test]
fn retained_rehash_matches_fixed_zero_tail_and_ignored_suffix_semantics() {
    let context = scalar_context();
    for leaves in [1, 3, 65] {
        for chunk in [1, 17, 32] {
            let capacity = leaves * chunk;
            let tree = ByteMerkleTree::new(leaves, chunk).unwrap();
            let leaf_pointer = tree.leaves.lock().as_ptr();
            let allocations = canonical_nodes::ALLOCATIONS.get();
            for length in [0, 1, capacity - 1, capacity, capacity + 7] {
                let input = vec![0x63; length];
                let mut padded = vec![0; capacity];
                let consumed = length.min(capacity);
                padded[..consumed].copy_from_slice(&input[..consumed]);
                let canonical = MerkleTree::<[u8; 32]>::from_byte_chunks(&padded, chunk).unwrap();
                let observed = tree.rehash_parallel_in_context(&input, context);
                assert!(observed.matches(crate::vector::Sha256Backend::Scalar, consumed > 0));
                assert!(!tree.nodes.lock().is_current());
                assert_eq!(tree.root_hash(), canonical.root().unwrap());
                assert!(tree.nodes.lock().is_current());
                for index in 0..leaves {
                    assert_eq!(
                        tree.proof(index).unwrap(),
                        canonical.get_proof(index as u32).unwrap()
                    );
                }
                assert_eq!(tree.leaves.lock().as_ptr(), leaf_pointer);
                assert_eq!(canonical_nodes::ALLOCATIONS.get(), allocations);
            }
        }
    }
}

#[test]
fn locked_rehash_rejection_and_unwind_leave_original_tree_untouched() {
    let tree = ByteMerkleTree::from_bytes(&[0x53; 3 * 17], 17).unwrap();
    let original = tree.root();
    let old_leaves = tree.leaves.lock().to_vec();
    let digests = [[0x63; 32]; 3];
    assert!(tree.lock_leaf_update(&digests[..2]).is_none());
    {
        let pending = tree.lock_leaf_update(&digests).unwrap();
        assert!(tree.nodes.try_lock().is_none());
        assert!(tree.leaves.try_lock().is_none());
        drop(pending);
    }
    assert_eq!(tree.root(), original);
    assert_eq!(&**tree.leaves.lock(), old_leaves);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _pending = tree.lock_leaf_update(&digests).unwrap();
            panic!("owner acceptance unwinds before installation");
        }))
        .is_err()
    );
    assert_eq!(tree.root(), original);
    assert_eq!(&**tree.leaves.lock(), old_leaves);
    let pointer = tree.leaves.lock().as_ptr();
    let allocations = canonical_nodes::ALLOCATIONS.get();
    tree.lock_leaf_update(&digests).unwrap().install();
    assert!(tree.nodes.lock().is_current());
    assert_eq!(&**tree.leaves.lock(), digests);
    let expected = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests);
    assert_eq!(tree.root_hash(), expected.root().unwrap());
    assert_eq!(tree.leaves.lock().as_ptr(), pointer);
    assert_eq!(canonical_nodes::ALLOCATIONS.get(), allocations);
}
