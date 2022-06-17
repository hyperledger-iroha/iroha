//! Merkle tree implementation.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::vec_deque::VecDeque, format, string::String, vec};
#[cfg(feature = "std")]
use std::collections::VecDeque;

use iroha_schema::prelude::*;

use crate::HashOf;

/// [Merkle Tree](https://en.wikipedia.org/wiki/Merkle_tree) used to validate and prove data at
/// each block height. Implemented as a binary hash tree.
#[derive(Debug)]
pub struct MerkleTree<T> {
    root_node: Node<T>,
    height: u32,
}

impl<T: IntoSchema> IntoSchema for MerkleTree<T> {
    fn type_name() -> String {
        format!("{}::MerkleTree<{}>", module_path!(), T::type_name())
    }
    fn schema(map: &mut MetaMap) {
        map.entry(Self::type_name()).or_insert_with(|| {
            // BFS ordered list of leaf nodes
            Metadata::Vec(VecMeta {
                ty: HashOf::<T>::type_name(),
                sorted: true,
            })
        });
        if !map.contains_key(&HashOf::<T>::type_name()) {
            HashOf::<T>::schema(map);
        }
    }
}

/// Represents a subtree rooted by the current node.
#[derive(Debug)]
struct Subtree<T> {
    /// Left child node.
    left: Box<Node<T>>,
    /// Right child node.
    right: Box<Node<T>>,
    /// Hash of this node.
    hash: Option<HashOf<Node<T>>>,
}

/// Represents a leaf node.
#[derive(Debug)]
struct Leaf<T> {
    /// Hash of this node.
    hash: HashOf<T>,
}

/// Binary tree node: [`Subtree`], [`Leaf`] (with data or links to data), or `Empty`.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
enum Node<T> {
    /// [`Subtree`] node.
    Subtree(Subtree<T>),
    /// [`Leaf`] node.
    Leaf(Leaf<T>),
    /// Empty node.
    Empty,
}

#[derive(Debug)]
/// BFS iterator over [`MerkleTree`].
struct BreadthFirstNodeIterator<'itm, T> {
    queue: VecDeque<&'itm Node<T>>,
}

#[cfg(feature = "std")]
impl<T> FromIterator<HashOf<T>> for MerkleTree<T> {
    fn from_iter<I: IntoIterator<Item = HashOf<T>>>(iter: I) -> Self {
        let mut nodes = iter
            .into_iter()
            .map(|hash| Node::Leaf(Leaf { hash }))
            .collect::<VecDeque<_>>();
        nodes.make_contiguous().sort_unstable_by_key(Node::hash);

        let n_leaves = nodes.len();
        let mut height = 0;
        let mut base_len = 1;
        for depth in 0.. {
            base_len = 2_usize.pow(depth);
            if n_leaves <= base_len {
                height = depth;
                break;
            }
        }
        for _ in n_leaves..base_len {
            nodes.push_back(Node::Empty);
        }

        while let Some(node_l) = nodes.pop_front() {
            match nodes.pop_front() {
                Some(node_r) => nodes.push_back(Node::from_nodes(node_l, node_r)),
                None => {
                    nodes.push_back(node_l);
                    break;
                }
            }
        }

        Self {
            root_node: nodes.pop_front().unwrap_or(Node::Empty),
            height,
        }
    }
}

impl<T> MerkleTree<T> {
    /// Construct [`MerkleTree`].
    pub const fn new() -> Self {
        MerkleTree {
            root_node: Node::Empty,
            height: 0,
        }
    }

    /// Get the hash of the `idx`-th leaf node.
    pub fn get_leaf(&self, idx: usize) -> Option<HashOf<T>> {
        if 2_usize.pow(self.height) <= idx {
            return None;
        }
        let mut node = &self.root_node;
        let mut path = idx;
        for depth in 1..=self.height {
            let divisor = 2_usize.pow(self.height - depth);
            if let Node::Subtree(subtree) = node {
                match path.div_euclid(divisor) {
                    0 => node = &*subtree.left,
                    1 => node = &*subtree.right,
                    _ => unreachable!(),
                }
            }
            path %= divisor;
        }
        node.leaf_hash()
    }

    /// Get the hashes of the leaf nodes.
    pub fn iter(&self) -> impl Iterator<Item = HashOf<T>> + '_ {
        self.nodes().filter_map(Node::leaf_hash)
    }

    /// Get the hash of [`MerkleTree`] as the hash of its root node.
    pub const fn hash(&self) -> Option<HashOf<Self>> {
        if let Some(hash) = self.root_node.hash() {
            return Some(hash.transmute());
        }
        None
    }

    /// Get a BFS iterator over the tree.
    fn nodes(&self) -> BreadthFirstNodeIterator<T> {
        BreadthFirstNodeIterator::new(self)
    }

    /// Insert `hash` into the tree.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn add(&self, hash: HashOf<T>) -> Self {
        self.iter().chain([hash]).collect()
    }
}

impl<T> Node<T> {
    #[cfg(feature = "std")]
    fn from_nodes(left: Self, right: Self) -> Self {
        Self::Subtree(Subtree {
            hash: Self::nodes_pair_hash(&left, &right),
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    /// Get the hash of this node.
    const fn hash(&self) -> Option<HashOf<Self>> {
        match self {
            Node::Subtree(Subtree { hash, .. }) => *hash,
            Node::Leaf(Leaf { hash }) => Some((*hash).transmute()),
            Node::Empty => None,
        }
    }

    /// Get the hash of this node as a leaf.
    const fn leaf_hash(&self) -> Option<HashOf<T>> {
        if let Self::Leaf(Leaf { hash }) = *self {
            Some(hash)
        } else {
            None
        }
    }

    #[cfg(feature = "std")]
    fn nodes_pair_hash(left: &Self, right: &Self) -> Option<HashOf<Self>> {
        let (left_hash, right_hash) = match (left.hash(), right.hash()) {
            (None, None) => return None,
            (None, Some(r_hash)) => return Some(r_hash),
            (Some(l_hash), None) => return Some(l_hash),
            (Some(l_hash), Some(r_hash)) => (l_hash, r_hash),
        };
        let sum: Vec<_> = left_hash
            .as_ref()
            .iter()
            .zip(right_hash.as_ref().iter())
            .map(|(l, r)| l.wrapping_add(*r))
            .collect();
        Some(crate::Hash::new(sum).typed())
    }

    fn children(&self) -> Option<[&Self; 2]> {
        if let Node::Subtree(subtree) = self {
            return Some([&*subtree.left, &*subtree.right]);
        }
        None
    }
}

impl<'itm, T> BreadthFirstNodeIterator<'itm, T> {
    #[inline]
    fn new(tree: &'itm MerkleTree<T>) -> Self {
        let mut queue = VecDeque::with_capacity(2_usize.pow(tree.height));
        queue.push_back(&tree.root_node);
        Self { queue }
    }
}

/// `Iterator` impl for `BreadthFirstNodeIterator` case of iteration over `MerkleTree`.
/// `'itm` lifetime specified for `Node`. Because `Node` is recursive data structure with self
/// composition in case of `Node::Subtree` we use `Box` to know size of each `Node` object in
/// memory.
impl<'itm, T> Iterator for BreadthFirstNodeIterator<'itm, T> {
    type Item = &'itm Node<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.queue.pop_front() {
            if let Some(children) = node.children() {
                self.queue.extend(children);
            }
            Some(node)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::prelude::SliceRandom;

    use super::*;
    use crate::Hash;

    impl<T> MerkleTree<T> {
        fn leaf_indices(&self) -> Vec<usize> {
            self.nodes()
                .enumerate()
                .filter_map(|(i, node)| matches!(node, Node::Leaf(_)).then(|| i))
                .collect()
        }
    }

    fn test_hashes(n_hashes: u8) -> Vec<HashOf<()>> {
        (1..=n_hashes)
            .map(|i| Hash::prehashed([i; Hash::LENGTH]).typed())
            .collect()
    }

    #[test]
    fn construction() {
        let tree = test_hashes(5).into_iter().collect::<MerkleTree<_>>();
        //               #0: iteration order
        //               83: first 2 hex of hash
        //         ______/\______
        //       #1              #2
        //       c0              05
        //     __/\__          __/\__
        //   #3      #4      #5      #6
        //   fc      17      05      **
        //   /\      /\      /\      /\
        // #7  #8  #9  #a  #b  #c  #d  #e
        // 01  02  03  04  05  **  **  **
        assert_eq!(tree.nodes().count(), 0b1111);
        assert_eq!(tree.height, 3);
        assert_eq!(tree.leaf_indices(), (0x7..=0xb).collect::<Vec<_>>());
    }

    #[test]
    fn iteration() {
        const N_LEAVES: u8 = 5;

        let hashes_sorted = test_hashes(N_LEAVES);
        let mut hashes_randomized = hashes_sorted.clone();
        hashes_randomized.shuffle(&mut rand::thread_rng());
        let tree = hashes_randomized.into_iter().collect::<MerkleTree<_>>();

        for i in 0..N_LEAVES as usize * 2 {
            assert_eq!(tree.get_leaf(i).as_ref(), hashes_sorted.get(i))
        }
        for (testee_hash, tester_hash) in tree.iter().zip(hashes_sorted) {
            assert_eq!(testee_hash, tester_hash);
        }
    }

    #[test]
    fn reproduction() {
        const N_LEAVES: u8 = 5;

        let hashes_sorted = test_hashes(N_LEAVES);
        let tree = hashes_sorted.clone().into_iter().collect::<MerkleTree<_>>();

        let mut hashes_randomized = hashes_sorted;
        hashes_randomized.shuffle(&mut rand::thread_rng());
        let mut tree_reproduced = MerkleTree::new();
        for leaf_hash in hashes_randomized {
            tree_reproduced = tree_reproduced.add(leaf_hash)
        }

        assert_eq!(tree_reproduced.hash(), tree.hash());
        for (testee_node, tester_node) in tree_reproduced.nodes().zip(tree.nodes()) {
            assert_eq!(testee_node.hash(), tester_node.hash());
        }
    }
}
