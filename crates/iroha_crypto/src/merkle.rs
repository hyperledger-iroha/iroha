//! Merkle tree implementation for light clients to efficiently verify transaction inclusion proofs.

#[cfg(not(feature = "std"))]
use alloc::{collections::VecDeque, format, string::String, vec, vec::Vec};
#[cfg(feature = "std")]
use std::collections::VecDeque;

use derive_more::Display;
use iroha_schema::{IntoSchema, TypeId};
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{Hash, HashOf};

/// Array representation of [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree)
/// for verifying elements of type `T`.
#[derive(
    Debug,
    Display,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    TypeId,
)]
#[repr(transparent)]
pub struct MerkleTree<T>(Vec<Option<HashOf<T>>>);

/// A Merkle proof: index of a leaf among all leaves, and the shortest list of additional nodes to recompute the root.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, Deserialize, Serialize, IntoSchema,
)]
pub struct MerkleProof<T> {
    /// Zero-based index of the leaf among all leaves.
    leaf_index: u32,
    /// List of missing nodes required to recompute the nodes leading from a leaf to the root.
    audit_path: Vec<Option<HashOf<T>>>,
}

/// Iterator over the leaf hashes of a [`MerkleTree`], yielding each leaf in left-to-right order.
pub struct LeafHashIterator<'a, T> {
    tree: &'a MerkleTree<T>,
    index: usize,
    index_back: usize,
}

/// A complete binary tree supporting indexed access to nodes in breadth-first order.
trait CompleteBinaryTree {
    /// The type of value stored in each node.
    type NodeValue;

    /// Returns the total number of nodes in the tree.
    fn len(&self) -> usize;

    /// Returns a reference to the node value at `index`, in breadth-first order from the root.
    fn get(&self, index: usize) -> Option<&Self::NodeValue>;

    /// Returns a reference to the leaf node value at `index`, in left-to-right order among leaves.
    fn get_leaf(&self, index: usize) -> Option<&Self::NodeValue> {
        let offset = (1 << self.height()) - 1_usize;
        offset.checked_add(index).and_then(|i| self.get(i))
    }

    /// Returns the height of the tree, defined as the number of edges from root to any leaf.
    fn height(&self) -> u32 {
        (usize::BITS - self.len().leading_zeros()).saturating_sub(1)
    }

    /// Returns the height of a complete binary tree with the given number of leaves.
    fn height_from_n_leaves(n: usize) -> u32 {
        usize::BITS - n.saturating_sub(1).leading_zeros()
    }

    /// Returns the maximum number of nodes the tree can contain without increasing its height.
    fn capacity(&self) -> usize {
        (1 << (self.height() + 1)) - 1
    }

    /// Returns the index of the given leaf, in breadth-first order from the root.
    fn index_in_tree(&self, leaf_index: usize) -> Option<usize> {
        let index = Self::index_in_tree_unchecked(leaf_index, self.height() as usize);

        (index < self.len()).then_some(index)
    }

    /// Returns the index of the given leaf, in breadth-first order from the root.
    ///
    /// Does not check if the result is within bounds.
    fn index_in_tree_unchecked(leaf_index: usize, height: usize) -> usize {
        let offset = (1 << height) - 1_usize;
        offset.saturating_add(leaf_index)
    }

    /// Returns the index of the parent of the node at `index`, or `None` if the node is the root.
    fn parent_index(&self, index: usize) -> Option<usize> {
        if 0 == index {
            return None;
        }
        let index = (index - 1) >> 1;
        (index < self.len()).then_some(index)
    }

    /// Returns the index of the left child of the node at `index`, if it exists.
    fn l_child_index(&self, index: usize) -> Option<usize> {
        let index = (index << 1) + 1;
        (index < self.len()).then_some(index)
    }

    /// Returns the index of the right child of the node at `index`, if it exists.
    fn r_child_index(&self, index: usize) -> Option<usize> {
        let index = (index << 1) + 2;
        (index < self.len()).then_some(index)
    }

    /// Returns the index of the sibling node of the node at `index`, if it exists.
    fn sibling_index(&self, index: usize) -> Option<usize> {
        if index.is_multiple_of(2) {
            (0 < index).then(|| index - 1)
        } else {
            (index < self.len() - 1).then(|| index + 1)
        }
    }

    /// Returns a reference to the left child of the node at `index`, if it exists.
    fn get_l_child(&self, index: usize) -> Option<&Self::NodeValue> {
        self.l_child_index(index).and_then(|i| self.get(i))
    }

    /// Returns a reference to the right child of the node at `index`, if it exists.
    fn get_r_child(&self, index: usize) -> Option<&Self::NodeValue> {
        self.r_child_index(index).and_then(|i| self.get(i))
    }
}

impl<T> CompleteBinaryTree for MerkleTree<T> {
    type NodeValue = HashOf<T>;

    fn len(&self) -> usize {
        self.0.len()
    }

    fn get(&self, index: usize) -> Option<&Self::NodeValue> {
        self.0.get(index).and_then(|opt| opt.as_ref())
    }
}

impl<T> FromIterator<HashOf<T>> for MerkleTree<T> {
    fn from_iter<I: IntoIterator<Item = HashOf<T>>>(iter: I) -> Self {
        let mut queue = iter.into_iter().map(Some).collect::<VecDeque<_>>();

        let height = Self::height_from_n_leaves(queue.len());
        let n_complement = (1 << height) - queue.len();
        for _ in 0..n_complement {
            queue.push_back(None);
        }

        let mut tree = Vec::with_capacity(1 << (height + 1));
        while let Some(r_node) = queue.pop_back() {
            if let Some(l_node) = queue.pop_back() {
                queue.push_front(Self::pair_hash(l_node.as_ref(), r_node.as_ref()));
                tree.push(r_node);
                tree.push(l_node);
            } else {
                tree.push(r_node);
                break;
            }
        }
        tree.reverse();

        for _ in 0..n_complement {
            tree.pop();
        }

        Self(tree)
    }
}

// NOTE: Leaf nodes in the order of insertion
impl<T: IntoSchema> IntoSchema for MerkleTree<T> {
    fn type_name() -> String {
        format!("MerkleTree<{}>", T::type_name())
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(iroha_schema::Metadata::Vec(iroha_schema::VecMeta {
                ty: core::any::TypeId::of::<HashOf<T>>(),
            }));

            HashOf::<T>::update_schema_map(map);
        }
    }
}

impl<T> Default for MerkleTree<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<'a, T> MerkleTree<T> {
    /// Leaf hashes of this Merkle tree.
    pub fn leaves(&'a self) -> LeafHashIterator<'a, T> {
        LeafHashIterator::new(self)
    }
}

impl<T> MerkleTree<T> {
    /// Returns the hash of the root node, or `None` if the tree has no nodes.
    pub fn root(&self) -> Option<HashOf<Self>> {
        self.get(0).copied().map(HashOf::transmute)
    }

    /// Constructs a Merkle proof for the leaf at the given index among all leaves.
    pub fn get_proof(&self, leaf_index: u32) -> Option<MerkleProof<T>> {
        let mut index = self.index_in_tree(leaf_index as usize)?;
        let mut audit_path = Vec::new();

        while let Some(parent_index) = self.parent_index(index) {
            let sibling = self.sibling_index(index).and_then(|i| self.get(i));
            audit_path.push(sibling.copied());
            index = parent_index;
        }

        Some(MerkleProof {
            leaf_index,
            audit_path,
        })
    }

    /// Appends a leaf hash to the tree and updates all affected parent nodes.
    pub fn add(&mut self, hash: HashOf<T>) {
        // If the tree is perfect, increment its height to double the leaf capacity.
        if self.capacity() == self.len() {
            let height = self.height();
            let mut new_array = vec![None];
            let mut array = core::mem::take(&mut self.0);
            for depth in 0..height {
                let capacity_at_depth = 1 << depth;
                let tail = array.split_off(capacity_at_depth);
                array.extend(vec![None; capacity_at_depth]);
                new_array.append(&mut array);
                array = tail;
            }
            new_array.append(&mut array);
            self.0 = new_array;
        }

        self.0.push(Some(hash));
        self.update(self.len() - 1);
    }

    /// Recomputes hashes along the path from the leaf at `index` up to the root.
    fn update(&mut self, mut index: usize) {
        let mut node = self.get(index).copied();
        while let Some(parent_index) = self.parent_index(index) {
            let (l_node, r_node) = match index % 2 {
                0 => (self.get_l_child(parent_index), node.as_ref()),
                1 => (node.as_ref(), self.get_r_child(parent_index)),
                _ => unreachable!(),
            };
            let parent_node = Self::pair_hash(l_node, r_node);
            let parent_mut = self
                .0
                .get_mut(parent_index)
                .expect("bug: missing parent node at computed index");
            *parent_mut = parent_node;
            index = parent_index;
            node = parent_node;
        }
    }

    /// Combines two child hashes into a parent hash.
    ///
    /// Pre-hash processing:
    /// - If both children are present, concatenates their hashes.
    ///   The order is non-commutative and essential for index verification.
    /// - If only the left child is present, promotes it to the next level without hashing.
    /// - If the left child is absent, returns `None`.
    #[inline]
    fn pair_hash(l_node: Option<&HashOf<T>>, r_node: Option<&HashOf<T>>) -> Option<HashOf<T>> {
        let (l_hash, r_hash) = match (l_node, r_node) {
            (Some(l_hash), Some(r_hash)) => (l_hash, r_hash),
            (Some(l_hash), None) => return Some(*l_hash),
            (None, Some(_)) => {
                // Invalid Merkle path: a right-only child cannot exist in a complete tree.
                // Return None instead of panicking to allow graceful rejection during verification.
                return None;
            }
            (None, None) => return None,
        };

        let mut concat = [0u8; Hash::LENGTH * 2];
        concat[..Hash::LENGTH].copy_from_slice(l_hash.as_ref());
        concat[Hash::LENGTH..].copy_from_slice(r_hash.as_ref());

        Some(HashOf::from_untyped_unchecked(Hash::new(concat)))
    }
}

impl<T> MerkleProof<T> {
    /// Verifies the Merkle proof against the given leaf and root hash.
    /// Returns true if the computed root from the proof matches the given root.
    pub fn verify(self, leaf: &HashOf<T>, root: &HashOf<MerkleTree<T>>, max_height: usize) -> bool {
        let height = self.audit_path.len();
        // Reject if the proof claims a tree taller than allowed.
        if max_height < height {
            return false;
        }
        let mut index = MerkleTree::<T>::index_in_tree_unchecked(self.leaf_index as usize, height);
        let Some(computed_root) = self.audit_path.into_iter().fold(Some(*leaf), |acc, e| {
            let (l_node, r_node) = match index % 2 {
                0 => (e, acc),
                1 => (acc, e),
                _ => unreachable!(),
            };
            index = index.saturating_sub(1) >> 1;

            MerkleTree::pair_hash(l_node.as_ref(), r_node.as_ref())
        }) else {
            // pair_hash returned None, implying the proof path is malformed.
            return false;
        };

        *root == computed_root.transmute()
    }
}

impl<T> Iterator for LeafHashIterator<'_, T> {
    type Item = HashOf<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index_back <= self.index {
            return None;
        }
        let leaf = self
            .tree
            .get_leaf(self.index)
            .copied()
            .expect("bug: missing leaf at valid index");
        // Increment the front index eagerly.
        self.index += 1;
        Some(leaf)
    }
}

impl<T> DoubleEndedIterator for LeafHashIterator<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index_back <= self.index {
            return None;
        }
        // Decrement the back index lazily.
        self.index_back -= 1;
        let leaf = self
            .tree
            .get_leaf(self.index_back)
            .copied()
            .expect("bug: missing leaf at valid index");
        Some(leaf)
    }
}

impl<T> ExactSizeIterator for LeafHashIterator<'_, T> {
    fn len(&self) -> usize {
        self.index_back - self.index
    }
}

impl<'a, T> LeafHashIterator<'a, T> {
    fn new(tree: &'a MerkleTree<T>) -> Self {
        let offset = (1 << tree.height()) - 1;
        let n_leaves = tree.len() - offset;

        Self {
            tree,
            index: 0,
            index_back: n_leaves,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hash;

    fn test_hashes(n_hashes: u8) -> Vec<HashOf<()>> {
        (0..n_hashes)
            .map(|i| Hash::prehashed([i; Hash::LENGTH]))
            .map(HashOf::from_untyped_unchecked)
            .collect()
    }

    #[test]
    fn builds_tree_from_hashes() {
        let tree: MerkleTree<_> = test_hashes(5).into_iter().collect();
        //               a3: first 2 hex digits of the hash
        //         ______/\______
        //       60              04
        //     __/\__          __/\__
        //   43      46      04      **
        //   /\      /\      /
        // 00  01  02  03  04
        for (i, node) in tree.0.iter().enumerate() {
            println!("{i:02}th node: {node:?}")
        }
        assert_eq!(tree.height(), 3);
        assert_eq!(tree.len(), 12);
        assert!(tree.get(6).is_none());
        assert!(tree.get(11).is_some());
        assert!(tree.get(12).is_none());
    }

    #[test]
    fn iterates_over_leaves() {
        let hashes = test_hashes(5);
        let tree: MerkleTree<_> = hashes.clone().into_iter().collect();
        let leaves: Vec<_> = tree.leaves().collect();
        assert_eq!(hashes, leaves);

        let mut leaves_iter = tree.leaves();
        assert_eq!(leaves_iter.len(), 5);
        assert_eq!(leaves_iter.next(), Some(leaves[0]));
        assert_eq!(leaves_iter.next_back(), Some(leaves[4]));
        assert_eq!(leaves_iter.len(), 3);
        assert_eq!(leaves_iter.next(), Some(leaves[1]));
        assert_eq!(leaves_iter.next_back(), Some(leaves[3]));
        assert_eq!(leaves_iter.next(), Some(leaves[2]));
        assert_eq!(leaves_iter.len(), 0);
        assert_eq!(leaves_iter.next_back(), None);
        assert_eq!(leaves_iter.next(), None);
        assert_eq!(leaves_iter.len(), 0);
    }

    #[test]
    fn grows_incrementally_and_matches_prebuilt_tree() {
        let leaves = test_hashes(5);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();

        let mut growing_tree = MerkleTree::default();
        for leaf in leaves {
            growing_tree.add(leaf);
        }

        assert_eq!(growing_tree.root(), tree.root());
        assert_eq!(growing_tree, tree);
    }

    #[test]
    fn provides_and_verifies_inclusion_proofs() {
        let leaves = test_hashes(5);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();

        // Generate proofs.
        let mut proofs: Vec<_> = (0..5).map(|i| tree.get_proof(i).unwrap()).collect();

        // Verify: valid proofs should succeed.
        for (leaf, proof) in leaves.iter().zip(proofs.clone()) {
            // Assumes up to 2^9 (512) transactions per block.
            assert!(proof.verify(leaf, &tree.root().unwrap(), 9));
        }

        // Mirror the leaf index to invalidate proofs.
        let mirror_index = |index: usize| {
            let height = MerkleTree::<()>::height_from_n_leaves(5);
            let capacity_for_leaves = 1 << height;
            let mirrored = capacity_for_leaves - 1 - index;
            println!("mirroring index from {index} to {mirrored}");
            u32::try_from(mirrored).unwrap()
        };

        // Corrupt each proof by modifying its leaf index.
        for (i, proof) in proofs.iter_mut().enumerate() {
            proof.leaf_index = mirror_index(i);
        }

        // Verify: corrupted proofs should fail.
        for (leaf, proof) in leaves.iter().zip(proofs) {
            assert!(!proof.verify(leaf, &tree.root().unwrap(), 9));
        }
    }
}
