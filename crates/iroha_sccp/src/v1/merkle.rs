//! SCCP v1 promote-odd Merkle trees (spec §3.4).
//!
//! Level 0 is the leaf list. Each next level pairs elements left to right with
//! `node(l, r) = keccak256("SCCP/NODE/V1" ‖ l ‖ r)`, and an unpaired last element is promoted
//! unchanged. The root of a single-leaf tree is that leaf. Block commitment trees hold
//! 1..=512 leaves (paths of at most 9 siblings); the history accumulator (§3.5) uses the same
//! rule over up to `2^32` leaves (at most 32 siblings).
//!
//! Verification is positional: [`merkle_root`] binds the leaf index and the leaf count exactly
//! as the §3.4 pseudo-code, so a promoted element never consumes a sibling.

use super::{
    constants::{MAX_BLOCK_LEAVES, MAX_BLOCK_PATH},
    hashes::node,
};

unit_error! {
    /// Merkle construction and verification errors.
    pub enum MerkleError {
        /// The tree has no leaves.
        Empty => "a Merkle tree needs at least one leaf",
        /// The tree exceeds its leaf bound.
        TooManyLeaves => "the Merkle tree exceeds its leaf bound",
        /// `index >= count`.
        IndexOutOfRange => "leaf index is not below the leaf count",
        /// The path has fewer siblings than the positional walk consumes.
        PathTooShort => "Merkle path has too few siblings",
        /// The path has more siblings than the positional walk consumes.
        PathTooLong => "Merkle path has unused siblings",
        /// The recomputed root differs from the expected root.
        RootMismatch => "Merkle path does not reach the expected root",
    }
}

/// All levels of a promote-odd tree, from the leaves (level 0) to the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteOddTree {
    levels: Vec<Vec<[u8; 32]>>,
}

impl PromoteOddTree {
    /// Build the tree over `leaves` (at least one).
    ///
    /// # Errors
    ///
    /// Returns [`MerkleError::Empty`] for an empty leaf list.
    pub fn new(leaves: &[[u8; 32]]) -> Result<Self, MerkleError> {
        if leaves.is_empty() {
            return Err(MerkleError::Empty);
        }
        let mut levels = vec![leaves.to_vec()];
        while levels.last().is_some_and(|level| level.len() > 1) {
            let current = levels.last().expect("non-empty levels");
            let next = current
                .chunks(2)
                .map(|pair| match pair {
                    [left, right] => node(left, right),
                    [single] => *single,
                    _ => unreachable!("chunks(2) yields one or two elements"),
                })
                .collect();
            levels.push(next);
        }
        Ok(Self { levels })
    }

    /// Build a block commitment tree (1..=512 leaves).
    ///
    /// # Errors
    ///
    /// Returns [`MerkleError::Empty`] or [`MerkleError::TooManyLeaves`].
    pub fn block(leaves: &[[u8; 32]]) -> Result<Self, MerkleError> {
        if leaves.len() > MAX_BLOCK_LEAVES as usize {
            return Err(MerkleError::TooManyLeaves);
        }
        Self::new(leaves)
    }

    /// Number of leaves.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.levels[0].len()
    }

    /// The leaves.
    #[must_use]
    pub fn leaves(&self) -> &[[u8; 32]] {
        &self.levels[0]
    }

    /// Every level, leaves first and the one-element root level last.
    #[must_use]
    pub fn levels(&self) -> &[Vec<[u8; 32]>] {
        &self.levels
    }

    /// The root.
    #[must_use]
    pub fn root(&self) -> [u8; 32] {
        self.levels.last().expect("non-empty levels")[0]
    }

    /// The positional sibling path of leaf `index`, bottom-up, skipping promoted levels.
    ///
    /// # Errors
    ///
    /// Returns [`MerkleError::IndexOutOfRange`] for `index >= leaf_count`.
    pub fn path(&self, index: usize) -> Result<Vec<[u8; 32]>, MerkleError> {
        if index >= self.leaf_count() {
            return Err(MerkleError::IndexOutOfRange);
        }
        let mut path = Vec::new();
        let mut position = index;
        for level in &self.levels[..self.levels.len() - 1] {
            if position % 2 == 1 {
                path.push(level[position - 1]);
            } else if position + 1 < level.len() {
                path.push(level[position + 1]);
            }
            position >>= 1;
        }
        Ok(path)
    }
}

/// Promote-odd root over `leaves` (any count of at least one).
///
/// # Errors
///
/// Returns [`MerkleError::Empty`] for an empty list.
pub fn promote_odd_root(leaves: &[[u8; 32]]) -> Result<[u8; 32], MerkleError> {
    Ok(PromoteOddTree::new(leaves)?.root())
}

/// Root of a block commitment tree (1..=512 leaves).
///
/// # Errors
///
/// Returns [`MerkleError::Empty`] or [`MerkleError::TooManyLeaves`].
pub fn block_root(leaves: &[[u8; 32]]) -> Result<[u8; 32], MerkleError> {
    Ok(PromoteOddTree::block(leaves)?.root())
}

/// The positional verifier of §3.4: recompute the root from `leaf`, its `index`, the leaf
/// `count` and the sibling path.
///
/// ```text
/// require count ≥ 1 and index < count
/// h, k = leaf, 0
/// while count > 1:
///     if index is odd:            h = node(siblings[k], h); k += 1
///     elif index + 1 < count:     h = node(h, siblings[k]); k += 1
///     # else: promoted, no sibling at this level
///     index >>= 1; count = (count + 1) >> 1
/// require k == len(siblings)
/// ```
///
/// # Errors
///
/// Returns [`MerkleError::Empty`] for `count = 0`, [`MerkleError::IndexOutOfRange`],
/// [`MerkleError::PathTooShort`] or [`MerkleError::PathTooLong`].
pub fn merkle_root(
    leaf: &[u8; 32],
    index: u64,
    count: u64,
    siblings: &[[u8; 32]],
) -> Result<[u8; 32], MerkleError> {
    if count == 0 {
        return Err(MerkleError::Empty);
    }
    if index >= count {
        return Err(MerkleError::IndexOutOfRange);
    }
    let (mut hash, mut index, mut count, mut used) = (*leaf, index, count, 0_usize);
    while count > 1 {
        if index % 2 == 1 {
            let sibling = siblings.get(used).ok_or(MerkleError::PathTooShort)?;
            hash = node(sibling, &hash);
            used += 1;
        } else if index + 1 < count {
            let sibling = siblings.get(used).ok_or(MerkleError::PathTooShort)?;
            hash = node(&hash, sibling);
            used += 1;
        }
        index >>= 1;
        count = count.div_ceil(2);
    }
    if used != siblings.len() {
        return Err(MerkleError::PathTooLong);
    }
    Ok(hash)
}

/// Verify a block inclusion: `count` in 1..=512, at most 9 siblings, and the positional root
/// equals `root`.
///
/// # Errors
///
/// Returns a [`MerkleError`] describing the first failed check.
pub fn verify_block_inclusion(
    leaf: &[u8; 32],
    index: u32,
    count: u32,
    path: &[[u8; 32]],
    root: &[u8; 32],
) -> Result<(), MerkleError> {
    if count > MAX_BLOCK_LEAVES {
        return Err(MerkleError::TooManyLeaves);
    }
    if path.len() > MAX_BLOCK_PATH {
        return Err(MerkleError::PathTooLong);
    }
    let computed = merkle_root(leaf, u64::from(index), u64::from(count), path)?;
    if &computed == root {
        Ok(())
    } else {
        Err(MerkleError::RootMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::hashes::keccak256;

    fn leaves(count: usize) -> Vec<[u8; 32]> {
        (0..count)
            .map(|index| keccak256(&[b"leaf", &(index as u64).to_be_bytes()]))
            .collect()
    }

    #[test]
    fn small_trees_follow_the_promote_odd_rule() {
        let l = leaves(5);
        assert_eq!(promote_odd_root(&l[..1]).unwrap(), l[0]);
        assert_eq!(promote_odd_root(&l[..2]).unwrap(), node(&l[0], &l[1]));
        assert_eq!(
            promote_odd_root(&l[..3]).unwrap(),
            node(&node(&l[0], &l[1]), &l[2])
        );
        assert_eq!(
            promote_odd_root(&l[..5]).unwrap(),
            node(&node(&node(&l[0], &l[1]), &node(&l[2], &l[3])), &l[4])
        );
        assert_eq!(promote_odd_root(&[]), Err(MerkleError::Empty));
    }

    #[test]
    fn every_path_verifies_for_counts_1_to_40_and_512() {
        for count in (1..=40).chain([511, 512]) {
            let l = leaves(count);
            let tree = PromoteOddTree::block(&l).unwrap();
            for (index, leaf) in l.iter().enumerate() {
                let path = tree.path(index).unwrap();
                assert!(path.len() <= MAX_BLOCK_PATH);
                assert_eq!(
                    merkle_root(leaf, index as u64, count as u64, &path),
                    Ok(tree.root()),
                    "count {count} index {index}"
                );
                verify_block_inclusion(
                    leaf,
                    u32::try_from(index).unwrap(),
                    u32::try_from(count).unwrap(),
                    &path,
                    &tree.root(),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn promoted_leaves_skip_levels() {
        // In a 5-leaf tree, leaf 4 is promoted twice and pairs only at the top.
        let l = leaves(5);
        let tree = PromoteOddTree::new(&l).unwrap();
        let path = tree.path(4).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0], node(&node(&l[0], &l[1]), &node(&l[2], &l[3])));
        // A full 512-leaf tree has 9-sibling paths.
        let tree = PromoteOddTree::block(&leaves(512)).unwrap();
        assert_eq!(tree.path(0).unwrap().len(), 9);
        assert_eq!(tree.path(511).unwrap().len(), 9);
    }

    #[test]
    fn positional_verifier_rejects_mismatches() {
        let l = leaves(6);
        let tree = PromoteOddTree::new(&l).unwrap();
        let path = tree.path(2).unwrap();
        assert_eq!(merkle_root(&l[2], 2, 0, &path), Err(MerkleError::Empty));
        assert_eq!(
            merkle_root(&l[2], 6, 6, &path),
            Err(MerkleError::IndexOutOfRange)
        );
        assert_eq!(
            merkle_root(&l[2], 2, 6, &path[..path.len() - 1]),
            Err(MerkleError::PathTooShort)
        );
        let mut long = path.clone();
        long.push([0; 32]);
        assert_eq!(
            merkle_root(&l[2], 2, 6, &long),
            Err(MerkleError::PathTooLong)
        );
        // Wrong index or count reaches a different root.
        assert_ne!(merkle_root(&l[2], 3, 6, &path).ok(), Some(tree.root()));
        assert_ne!(merkle_root(&l[2], 2, 3, &path).ok(), Some(tree.root()));
        assert_eq!(
            verify_block_inclusion(&l[3], 2, 6, &path, &tree.root()),
            Err(MerkleError::RootMismatch)
        );
        assert_eq!(
            verify_block_inclusion(&l[2], 2, 513, &path, &tree.root()),
            Err(MerkleError::TooManyLeaves)
        );
        assert_eq!(
            verify_block_inclusion(&l[2], 2, 6, &[[0; 32]; 10], &tree.root()),
            Err(MerkleError::PathTooLong)
        );
    }

    #[test]
    fn internal_nodes_cannot_pose_as_leaves() {
        // node(l0, l1) offered as leaf 0 of the 4-leaf tree needs two siblings; with the
        // one-sibling path of a 2-leaf tree the count binding rejects it.
        let l = leaves(4);
        let tree = PromoteOddTree::new(&l).unwrap();
        let inner = node(&l[0], &l[1]);
        let sibling = node(&l[2], &l[3]);
        assert_eq!(node(&inner, &sibling), tree.root());
        assert_eq!(
            merkle_root(&inner, 0, 4, &[sibling]),
            Err(MerkleError::PathTooShort)
        );
    }

    #[test]
    fn block_tree_bounds() {
        assert_eq!(
            PromoteOddTree::block(&leaves(513)),
            Err(MerkleError::TooManyLeaves)
        );
        assert_eq!(block_root(&[]), Err(MerkleError::Empty));
        let l = leaves(3);
        assert_eq!(block_root(&l), promote_odd_root(&l));
        let tree = PromoteOddTree::new(&l).unwrap();
        assert_eq!(tree.leaf_count(), 3);
        assert_eq!(tree.leaves(), l.as_slice());
        assert_eq!(tree.levels().len(), 3);
        assert_eq!(tree.path(3), Err(MerkleError::IndexOutOfRange));
    }
}
