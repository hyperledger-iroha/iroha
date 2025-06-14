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

    /// Returns the maximum number of nodes the tree can contain without increasing its height.
    fn capacity(&self) -> usize {
        (1 << (self.height() + 1)) - 1
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
    type NodeValue = Option<HashOf<T>>;

    fn len(&self) -> usize {
        self.0.len()
    }

    fn get(&self, index: usize) -> Option<&Self::NodeValue> {
        self.0.get(index)
    }
}

impl<T> FromIterator<HashOf<T>> for MerkleTree<T> {
    fn from_iter<I: IntoIterator<Item = HashOf<T>>>(iter: I) -> Self {
        let mut queue = iter.into_iter().map(Some).collect::<VecDeque<_>>();

        let height = usize::BITS - queue.len().saturating_sub(1).leading_zeros();
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
        self.get(0).and_then(|node| node.map(HashOf::transmute))
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
        let Some(node) = self.get(index) else { return };
        let mut node = *node;
        while let Some(parent_index) = self.parent_index(index) {
            let (l_node, r_node_opt) = match index % 2 {
                0 => (self.get_l_child(parent_index).unwrap(), Some(&node)),
                1 => (&node, self.get_r_child(parent_index)),
                _ => unreachable!(),
            };
            let parent_node = r_node_opt.map_or(*l_node, |r_node| {
                Self::pair_hash(l_node.as_ref(), r_node.as_ref())
            });
            let parent_mut = self.0.get_mut(parent_index).unwrap();
            *parent_mut = parent_node;
            index = parent_index;
            node = parent_node;
        }
    }

    /// Combines two child hashes into a parent hash.
    fn pair_hash(l_node: Option<&HashOf<T>>, r_node: Option<&HashOf<T>>) -> Option<HashOf<T>> {
        let (l_hash, r_hash) = match (l_node, r_node) {
            (Some(l_hash), Some(r_hash)) => (l_hash, r_hash),
            (Some(l_hash), None) => return Some(*l_hash),
            (None, Some(_)) => unreachable!(),
            (None, None) => return None,
        };
        let sum: Vec<_> = l_hash
            .as_ref()
            .iter()
            .zip(r_hash.as_ref().iter())
            .map(|(l, r)| l.wrapping_add(*r))
            .collect();
        Some(HashOf::from_untyped_unchecked(Hash::new(sum)))
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
            .and_then(|opt| opt.as_ref().copied())
            .unwrap();
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
            .and_then(|opt| opt.as_ref().copied())
            .unwrap();
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
        let last_capacity = (1 << tree.height()) - 1;
        let n_leaves = tree.len() - last_capacity;

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
        (1..=n_hashes)
            .map(|i| Hash::prehashed([i; Hash::LENGTH]))
            .map(HashOf::from_untyped_unchecked)
            .collect()
    }

    #[test]
    fn construction() {
        let tree = test_hashes(5).into_iter().collect::<MerkleTree<_>>();
        //               #-: iteration order
        //               9b: first 2 hex of hash
        //         ______/\______
        //       #-              #-
        //       0a              05
        //     __/\__          __/\__
        //   #-      #-      #-      #-
        //   fc      17      05      **
        //   /\      /\      /
        // #0  #1  #2  #3  #4
        // 01  02  03  04  05
        assert_eq!(tree.height(), 3);
        assert_eq!(tree.len(), 12);
        assert!(matches!(tree.get(6), Some(None)));
        assert!(matches!(tree.get(11), Some(Some(_))));
        assert!(tree.get(12).is_none());
    }

    #[test]
    fn iteration() {
        let hashes = test_hashes(5);
        let tree: MerkleTree<_> = hashes.clone().into_iter().collect();
        let leaves: Vec<_> = tree.leaves().collect();
        assert_eq!(hashes, leaves);

        let mut leaves_iter = tree.leaves();
        assert_eq!(leaves_iter.len(), 5);
        assert_eq!(leaves_iter.next(), Some(hashes[0]));
        assert_eq!(leaves_iter.next_back(), Some(hashes[4]));
        assert_eq!(leaves_iter.len(), 3);
        assert_eq!(leaves_iter.next(), Some(hashes[1]));
        assert_eq!(leaves_iter.next_back(), Some(hashes[3]));
        assert_eq!(leaves_iter.next(), Some(hashes[2]));
        assert_eq!(leaves_iter.len(), 0);
        assert_eq!(leaves_iter.next_back(), None);
        assert_eq!(leaves_iter.next(), None);
        assert_eq!(leaves_iter.len(), 0);
    }

    #[test]
    fn reproduction() {
        let hashes = test_hashes(5);
        let tree: MerkleTree<_> = hashes.clone().into_iter().collect();

        let mut tree_reproduced = MerkleTree::default();
        for hash in hashes {
            tree_reproduced.add(hash);
        }

        assert_eq!(tree_reproduced.root(), tree.root());
        assert_eq!(tree_reproduced, tree);
    }
}
