//! Canonical traversal parity, exact length and independently verified roots.

use super::*;
use crate::{Hash, MerkleProof};

// Independent parent-chain reference for both borrowed and owned proof APIs.
fn parent_chain_reference<T>(tree: &MerkleTree<T>, leaf: u32) -> Option<Vec<Option<HashOf<T>>>> {
    let mut index = tree.index_in_tree(leaf as usize)?;
    let mut path = Vec::new();
    while let Some(parent) = tree.parent_index(index) {
        path.push(
            tree.sibling_index(index)
                .and_then(|sibling| tree.get(sibling))
                .copied(),
        );
        index = parent;
    }
    Some(path)
}

#[test]
fn borrowed_siblings_preserve_empty_single_ragged_and_full_tree_geometry() {
    for count in [0, 1, 2, 3, 5, 31, 32, 255, 256, 257] {
        let leaves: Vec<_> = (0..count)
            .map(|index: usize| Hash::new(index.to_le_bytes()).into())
            .collect();
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.iter().copied());
        for (index, leaf) in leaves
            .iter()
            .map(Some)
            .chain(core::iter::once(None))
            .enumerate()
        {
            let index = u32::try_from(index).expect("test leaf count fits u32");
            let expected = parent_chain_reference(&tree, index);
            let actual = tree.proof_siblings(index).map(Iterator::collect::<Vec<_>>);
            assert_eq!(actual, expected, "count={count} index={index}");
            let owned = tree.get_proof(index);
            assert_eq!(
                owned.as_ref().map(MerkleProof::audit_path),
                actual.as_deref()
            );
            if let Some(leaf) = leaf {
                let reference = MerkleProof::from_audit_path(index, expected.unwrap());
                let proof = owned.unwrap();
                assert_eq!(
                    norito::encode_canonical(&proof).unwrap(),
                    norito::encode_canonical(&reference).unwrap(),
                    "owned canonical proof bytes count={count} index={index}"
                );
                let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(*leaf));
                assert!(proof.verify_sha256(&leaf, &tree.commitment().unwrap()));
            }
        }
    }
}

#[test]
fn borrowed_siblings_size_hint_counts_every_remaining_level_and_is_fused() {
    let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256([[3; 32]; 256]);
    for index in 0..256 {
        let mut path = tree.proof_siblings(index).unwrap();
        for remaining in (1..=8).rev() {
            assert_eq!(path.len(), remaining);
            assert_eq!(path.size_hint(), (remaining, Some(remaining)));
            assert!(path.next().unwrap().is_some());
        }
        assert_eq!(path.len(), 0);
        assert_eq!(path.size_hint(), (0, Some(0)));
        assert!(path.next().is_none());
        assert!(path.next().is_none());
    }
    assert!(tree.proof_siblings(256).is_none());
    assert!(tree.proof_siblings(u32::MAX).is_none());
}
