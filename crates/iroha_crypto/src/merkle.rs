//! Merkle tree implementation for light clients to efficiently verify transaction inclusion proofs.
//! This is the canonical Merkle type used across the workspace (node and IVM).

use std::{collections::VecDeque, format, string::String, vec, vec::Vec};

use iroha_schema::{IntoSchema, TypeId};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};
#[cfg(feature = "rayon")]
use rayon::prelude::*;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{Hash, HashOf};

/// Array representation of [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree)
/// for verifying elements of type `T`.
///
/// The canonical encoding stores nodes in breadth-first order; leaves and the
/// root must be present when the tree is non-empty. Internal nodes may be
/// `None` only for rightmost padding in incomplete trees (where both children
/// are missing). Missing nodes in proofs belong only in `MerkleProof` audit
/// paths.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, TypeId)]
#[repr(transparent)]
pub struct MerkleTree<T>(Vec<Option<HashOf<T>>>);

/// Errors returned by Merkle tree helpers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MerkleError {
    /// Chunk size must be in the range `1..=32`.
    #[error("invalid chunk size {chunk}; expected 1..=32 bytes")]
    InvalidChunkSize {
        /// The invalid chunk size provided by the caller.
        chunk: usize,
    },
    /// Merkle tree layout is malformed.
    #[error("invalid merkle tree layout: {0}")]
    InvalidLayout(String),
}

fn validate_chunk_size(chunk: usize) -> Result<(), MerkleError> {
    if (1..=32).contains(&chunk) {
        Ok(())
    } else {
        Err(MerkleError::InvalidChunkSize { chunk })
    }
}

crate::ffi::ffi_item! {
    /// A Merkle proof: index of a leaf among all leaves, and the shortest list of additional nodes to recompute the root.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
    pub struct MerkleProof<T> {
        /// Zero-based index of the leaf among all leaves.
        leaf_index: u32,
        /// List of missing nodes required to recompute the nodes leading from a leaf to the root.
        audit_path: Vec<Option<HashOf<T>>>,
    }
}

/// Compact Merkle proof using a direction bitset and sibling nodes.
///
/// - `depth`: number of levels used (<= 32 for `dirs` to cover)
/// - `dirs`: bit i encodes the direction at level i (0: accumulator is left child; 1: right child)
/// - `siblings`: sibling nodes from leaf → root; missing nodes encoded as `None`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
pub struct CompactMerkleProof<T> {
    depth: u8,
    dirs: u32,
    siblings: Vec<Option<HashOf<T>>>,
}

impl<T> norito::core::NoritoSerialize for MerkleTree<T> {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&self.0, writer)
    }
}

// -------------------------------
// Shielded commitments (ZK) helpers
// -------------------------------

/// Domain tag for shielded commitment leaves (Merkle leaves for ZK shielded pools).
/// Kept stable across platforms; trailing NUL disambiguates from adjacent tags.
const TAG_ZK_SHIELD_CM_V1: &[u8] = b"iroha:zk:shield:cm:v1\x00";

impl MerkleTree<[u8; 32]> {
    /// Compute a domain‑tagged leaf hash for a 32‑byte commitment using Blake2b‑32.
    /// The tag is `b"iroha:zk:shield:cm:v1\0"` and is concatenated with the commitment bytes.
    pub fn shielded_leaf_from_commitment(cm: [u8; 32]) -> HashOf<[u8; 32]> {
        let mut buf = [0u8; TAG_ZK_SHIELD_CM_V1.len() + 32];
        buf[..TAG_ZK_SHIELD_CM_V1.len()].copy_from_slice(TAG_ZK_SHIELD_CM_V1);
        buf[TAG_ZK_SHIELD_CM_V1.len()..].copy_from_slice(&cm);
        HashOf::from_untyped_unchecked(Hash::new(buf))
    }

    /// Explicit empty‑tree root for a fixed height `depth` (binary tree).
    ///
    /// Construction:
    /// - Base leaf L0 = Hash(tag || `[0u8; 32]`)
    /// - For each level, parent = Hash(prev || prev)
    ///
    /// Returns the raw 32‑byte digest underlying `Hash` (LSB set by `Hash`).
    pub fn shielded_empty_root(depth: u8) -> [u8; 32] {
        // L0 — domain‑tagged zero leaf
        let mut leaf_buf = [0u8; TAG_ZK_SHIELD_CM_V1.len() + 32];
        leaf_buf[..TAG_ZK_SHIELD_CM_V1.len()].copy_from_slice(TAG_ZK_SHIELD_CM_V1);
        // trailing bytes already zeroed
        let mut h = Hash::new(leaf_buf);
        for _ in 0..depth {
            let mut concat = [0u8; 64];
            concat[..32].copy_from_slice(h.as_ref());
            concat[32..].copy_from_slice(h.as_ref());
            h = Hash::new(concat);
        }
        h.into()
    }
}

impl<'de, T> norito::core::NoritoDeserialize<'de> for MerkleTree<T> {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("MerkleTree decode")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        #[allow(unsafe_code)]
        let inner = norito::core::NoritoDeserialize::try_deserialize(unsafe {
            &*core::ptr::from_ref(archived).cast::<norito::core::Archived<Vec<Option<HashOf<T>>>>>()
        })?;
        Self::from_nodes_checked(inner).map_err(|err| norito::core::Error::Message(err.to_string()))
    }
}

#[cfg(feature = "json")]
impl<T> JsonSerialize for MerkleTree<T> {
    fn json_serialize(&self, out: &mut String) {
        json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl<T> JsonDeserialize for MerkleTree<T> {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let nodes = json::JsonDeserialize::json_deserialize(parser)?;
        MerkleTree::from_nodes_checked(nodes).map_err(|err| json::Error::Message(err.to_string()))
    }
}

impl<T> norito::core::NoritoSerialize for MerkleProof<T> {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(
            &(self.leaf_index, self.audit_path.clone()),
            writer,
        )
    }
}

impl<'de, T> norito::core::NoritoDeserialize<'de> for MerkleProof<T> {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        #[allow(unsafe_code)]
        let (leaf_index, audit_path) = norito::core::NoritoDeserialize::deserialize(unsafe {
            &*core::ptr::from_ref(archived)
                .cast::<norito::core::Archived<(u32, Vec<Option<HashOf<T>>>)>>()
        });
        MerkleProof {
            leaf_index,
            audit_path,
        }
    }
}

#[cfg(feature = "json")]
impl<T> JsonSerialize for MerkleProof<T> {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        json::write_json_string("leaf_index", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&self.leaf_index, out);
        out.push(',');
        json::write_json_string("audit_path", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&self.audit_path, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<T> JsonDeserialize for MerkleProof<T> {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;

        let mut leaf_index: Option<u32> = None;
        let mut audit_path: Option<Vec<Option<HashOf<T>>>> = None;

        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }

            let key = parser.parse_key()?;
            parser.consume_char(b':')?;
            match key.as_str() {
                "leaf_index" => {
                    if leaf_index.is_some() {
                        return Err(json::Error::duplicate_field("leaf_index"));
                    }
                    leaf_index = Some(u32::json_deserialize(parser)?);
                }
                "audit_path" => {
                    if audit_path.is_some() {
                        return Err(json::Error::duplicate_field("audit_path"));
                    }
                    audit_path = Some(Vec::<Option<HashOf<T>>>::json_deserialize(parser)?);
                }
                other => return Err(json::Error::unknown_field(other.to_owned())),
            }

            parser.skip_ws();
            if parser.try_consume_char(b',')? {
                continue;
            }
            parser.consume_char(b'}')?;
            break;
        }

        Ok(MerkleProof {
            leaf_index: leaf_index.ok_or_else(|| json::Error::missing_field("leaf_index"))?,
            audit_path: audit_path.ok_or_else(|| json::Error::missing_field("audit_path"))?,
        })
    }
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

impl<T> MerkleTree<T> {
    fn from_nodes_checked(nodes: Vec<Option<HashOf<T>>>) -> Result<Self, MerkleError> {
        Self::validate_nodes(&nodes)?;
        Ok(Self(nodes))
    }

    fn validate_nodes(nodes: &[Option<HashOf<T>>]) -> Result<(), MerkleError> {
        if nodes.is_empty() {
            return Ok(());
        }
        let len = nodes.len();
        let height = (usize::BITS - len.leading_zeros()).saturating_sub(1);
        let height_usize = height as usize;
        if height_usize >= usize::BITS as usize {
            return Err(MerkleError::InvalidLayout(
                "merkle tree height exceeds platform bit width".to_owned(),
            ));
        }
        let pow2 = 1usize << height_usize;
        let offset = pow2 - 1;
        if len <= offset {
            return Err(MerkleError::InvalidLayout(
                "merkle tree node count underflows leaf offset".to_owned(),
            ));
        }
        let leaf_count = len - offset;
        let expected_height = <Self as CompleteBinaryTree>::height_from_n_leaves(leaf_count);
        if expected_height != height {
            return Err(MerkleError::InvalidLayout(format!(
                "merkle tree node count {len} does not match leaf count {leaf_count}"
            )));
        }
        if nodes[0].is_none() {
            return Err(MerkleError::InvalidLayout(
                "merkle tree root must be present".to_owned(),
            ));
        }
        if nodes[offset..].iter().any(Option::is_none) {
            return Err(MerkleError::InvalidLayout(
                "merkle tree leaves must be present".to_owned(),
            ));
        }
        for i in 0..offset {
            let li = (i << 1) + 1;
            let ri = li + 1;
            let left = nodes.get(li).and_then(|node| node.as_ref());
            let right = nodes.get(ri).and_then(|node| node.as_ref());
            let parent = nodes[i].as_ref();
            if left.is_none() && right.is_some() {
                return Err(MerkleError::InvalidLayout(format!(
                    "merkle tree has right-only child at node {i}"
                )));
            }
            if parent.is_none() && (left.is_some() || right.is_some()) {
                return Err(MerkleError::InvalidLayout(format!(
                    "merkle tree missing parent for existing child at node {i}"
                )));
            }
            if left.is_none() && right.is_none() && parent.is_some() {
                return Err(MerkleError::InvalidLayout(format!(
                    "merkle tree has parent without children at node {i}"
                )));
            }
        }
        Ok(())
    }

    /// Return the depth (number of edges from root to any leaf) of the complete tree.
    #[must_use]
    pub fn depth(&self) -> u32 {
        <Self as CompleteBinaryTree>::height(self)
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

    /// Incrementally update the leaf at `leaf_index` with a new typed hash and
    /// recompute parents up to the root using the canonical `pair_hash` logic.
    pub fn update_typed_leaf(&mut self, leaf_index: usize, new_leaf: HashOf<T>) {
        let height = self.height() as usize;
        let index = MerkleTree::<T>::index_in_tree_unchecked(leaf_index, height);
        if let Some(slot) = self.0.get_mut(index) {
            *slot = Some(new_leaf);
            self.update(index);
        }
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

    /// Parallel builder from typed leaf hashes using canonical pair-hash
    /// semantics. This constructs the complete BFS array bottom-up, computing
    /// parents in parallel per level. Semantics match the sequential
    /// `FromIterator<HashOf<T>>` implementation.
    #[cfg(feature = "rayon")]
    pub fn from_typed_leaves_parallel<I>(leaves: I) -> Self
    where
        T: Send + Sync,
        I: IntoIterator<Item = HashOf<T>>,
    {
        let leaves_vec: Vec<HashOf<T>> = leaves.into_iter().collect();
        if leaves_vec.is_empty() {
            return Self::default();
        }

        let n = leaves_vec.len();
        let height = Self::height_from_n_leaves(n);
        let pow2 = 1usize << height;
        let capacity = (1usize << (height + 1)) - 1;
        let offset = pow2 - 1; // index of first leaf in BFS layout

        // Initialize BFS node array with None, then place leaves.
        let mut nodes: Vec<Option<HashOf<T>>> = vec![None; capacity];
        for (i, leaf) in leaves_vec.into_iter().enumerate() {
            nodes[offset + i] = Some(leaf);
        }

        // Compute parents bottom-up, in parallel per level.
        for lvl in (0..height).rev() {
            let start = (1usize << lvl) - 1;
            let end = (1usize << (lvl + 1)) - 1; // exclusive upper bound

            for i in start..end {
                let li = (i << 1) + 1;
                let ri = li + 1;
                let l = nodes.get(li).and_then(|o| o.as_ref());
                let r = nodes.get(ri).and_then(|o| o.as_ref());
                nodes[i] = Self::pair_hash(l, r);
            }
        }

        let complement = pow2 - n;
        if complement > 0 {
            nodes.truncate(nodes.len() - complement);
        }

        Self(nodes)
    }
}

impl<T> MerkleProof<T> {
    /// Construct a Merkle proof from a leaf index and an audit path.
    ///
    /// This constructor does not validate the path; it is primarily intended
    /// for tests and interoperability scenarios where the sibling list is
    /// produced externally (e.g., from another crate) and needs to be wrapped
    /// into a proof structure.
    pub fn from_audit_path(leaf_index: u32, audit_path: Vec<Option<HashOf<T>>>) -> Self {
        MerkleProof {
            leaf_index,
            audit_path,
        }
    }
    /// Verifies the Merkle proof against the given leaf and root hash.
    /// Returns true if the computed root from the proof matches the given root.
    /// Rejects proofs where `leaf_index` cannot fit within the implied height.
    /// Rejects proofs whose audit path length exceeds the platform bit width.
    pub fn verify(self, leaf: &HashOf<T>, root: &HashOf<MerkleTree<T>>, max_height: usize) -> bool {
        let height = self.audit_path.len();
        if height >= usize::BITS as usize {
            return false;
        }
        // Reject if the proof claims a tree taller than allowed.
        if max_height < height {
            return false;
        }
        if height < u32::BITS as usize {
            let max_leaves = 1usize << height;
            if self.leaf_index as usize >= max_leaves {
                return false;
            }
        }
        let mut index = MerkleTree::<T>::index_in_tree_unchecked(self.leaf_index as usize, height);
        let Some(computed_root) = self.audit_path.into_iter().try_fold(*leaf, |acc, e| {
            let (l_node, r_node) = match index % 2 {
                0 => (e, Some(acc)),
                1 => (Some(acc), e),
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

    /// Borrow the audit path (list of sibling nodes) for this proof.
    pub fn audit_path(&self) -> &[Option<HashOf<T>>] {
        &self.audit_path
    }
    /// Consume the proof and return its audit path.
    pub fn into_audit_path(self) -> Vec<Option<HashOf<T>>> {
        self.audit_path
    }
    /// Leaf index among all leaves for this proof.
    pub fn leaf_index(&self) -> u32 {
        self.leaf_index
    }
}

impl<T> CompactMerkleProof<T> {
    /// Construct a compact proof from raw parts. Depth is limited to 32 by
    /// the encoding used in VM syscalls, and `siblings.len()` should be equal
    /// to `depth`.
    pub fn from_parts(depth: u8, dirs: u32, siblings: Vec<Option<HashOf<T>>>) -> Self {
        CompactMerkleProof {
            depth,
            dirs,
            siblings,
        }
    }
    /// Number of levels used in this proof.
    pub fn depth(&self) -> u8 {
        self.depth
    }
    /// Direction bitset for each level.
    pub fn dirs(&self) -> u32 {
        self.dirs
    }
    /// Borrow the sibling list.
    pub fn siblings(&self) -> &[Option<HashOf<T>>] {
        &self.siblings
    }
    /// Construct a compact proof from a full `MerkleProof` by deriving the
    /// direction bitset from `leaf_index` and the path depth. If the audit path
    /// is longer than 32 levels, it is truncated to fit the compact encoding.
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_full(full: MerkleProof<T>) -> Self {
        let depth = full.audit_path.len().min(32) as u8;
        // Direction bits use leaf-index semantics: bit i = 0 (left), 1 (right).
        let depth_bits = u32::from(depth);
        let mask = if depth == 32 {
            u32::MAX
        } else {
            (1u32 << depth_bits) - 1
        };
        let dirs = full.leaf_index & mask;
        let siblings = full.audit_path.into_iter().take(depth as usize).collect();
        CompactMerkleProof {
            depth,
            dirs,
            siblings,
        }
    }

    /// Expand into a full `MerkleProof` given an explicit `leaf_index`.
    /// The index is required because the compact proof carries direction bits
    /// directly rather than the original index value.
    pub fn into_full_with_index(self, leaf_index: u32) -> MerkleProof<T> {
        MerkleProof::from_audit_path(leaf_index, self.siblings)
    }

    /// Verify this compact proof using an explicit leaf hash and root hash by
    /// reconstructing the parent hashes guided by the `dirs` bitset. Returns
    /// `false` if `depth` exceeds 32 or `siblings.len()` does not match `depth`.
    pub fn verify(self, leaf: &HashOf<T>, root: &HashOf<MerkleTree<T>>) -> bool {
        let max_depth = u8::try_from(u32::BITS).expect("u32::BITS fits in u8");
        if self.depth > max_depth {
            return false;
        }
        let depth = self.depth as usize;
        if self.siblings.len() != depth {
            return false;
        }
        let mut acc = *leaf;
        let mut dirs = self.dirs;
        let mut iter = self.siblings.into_iter();
        for _ in 0..depth {
            let sib = match iter.next() {
                Some(sib) => sib,
                None => return false,
            };
            let (l, r) = match dirs & 1 {
                0 => (Some(&acc), sib.as_ref()),
                1 => (sib.as_ref(), Some(&acc)),
                _ => unreachable!(),
            };
            dirs >>= 1;
            let Some(next) = MerkleTree::pair_hash(l, r) else {
                return false;
            };
            acc = next;
        }
        &acc.transmute() == root
    }
}

#[cfg(feature = "json")]
impl<T> JsonSerialize for CompactMerkleProof<T> {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        json::write_json_string("depth", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&self.depth, out);
        out.push(',');
        json::write_json_string("dirs", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&self.dirs, out);
        out.push(',');
        json::write_json_string("siblings", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&self.siblings, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<T> JsonDeserialize for CompactMerkleProof<T> {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;

        let mut depth: Option<u8> = None;
        let mut dirs: Option<u32> = None;
        let mut siblings: Option<Vec<Option<HashOf<T>>>> = None;

        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }

            let key = parser.parse_key()?;
            parser.consume_char(b':')?;
            match key.as_str() {
                "depth" => {
                    if depth.is_some() {
                        return Err(json::Error::duplicate_field("depth"));
                    }
                    depth = Some(u8::json_deserialize(parser)?);
                }
                "dirs" => {
                    if dirs.is_some() {
                        return Err(json::Error::duplicate_field("dirs"));
                    }
                    dirs = Some(u32::json_deserialize(parser)?);
                }
                "siblings" => {
                    if siblings.is_some() {
                        return Err(json::Error::duplicate_field("siblings"));
                    }
                    siblings = Some(Vec::<Option<HashOf<T>>>::json_deserialize(parser)?);
                }
                other => return Err(json::Error::unknown_field(other.to_owned())),
            }

            parser.skip_ws();
            if parser.try_consume_char(b',')? {
                continue;
            }
            parser.consume_char(b'}')?;
            break;
        }

        Ok(CompactMerkleProof {
            depth: depth.ok_or_else(|| json::Error::missing_field("depth"))?,
            dirs: dirs.ok_or_else(|| json::Error::missing_field("dirs"))?,
            siblings: siblings.ok_or_else(|| json::Error::missing_field("siblings"))?,
        })
    }
}

impl CompactMerkleProof<[u8; 32]> {
    /// Verify a compact proof where inner nodes are combined using SHA-256 of
    /// left||right and leaves are SHA-256 digests. Returns `false` if `depth`
    /// exceeds 32 or `siblings.len()` does not match `depth`.
    pub fn verify_sha256(
        self,
        leaf: &HashOf<[u8; 32]>,
        root: &HashOf<MerkleTree<[u8; 32]>>,
    ) -> bool {
        use crate::Hash;
        let max_depth = u8::try_from(u32::BITS).expect("u32::BITS fits in u8");
        if self.depth > max_depth {
            return false;
        }
        let mut acc_bytes: [u8; 32] = *leaf.as_ref();
        let mut dirs = self.dirs;
        let depth = self.depth as usize;
        if self.siblings.len() != depth {
            return false;
        }
        let mut iter = self.siblings.into_iter();
        for _ in 0..depth {
            let sib = match iter.next() {
                Some(sib) => sib,
                None => return false,
            };
            let acc_hash = HashOf::from_untyped_unchecked(Hash::prehashed(acc_bytes));
            let (l_opt, r_opt) = match dirs & 1 {
                0 => (Some(acc_hash), sib),
                1 => (sib, Some(acc_hash)),
                _ => unreachable!(),
            };
            dirs >>= 1;
            let combined = match (l_opt, r_opt) {
                (Some(lh), Some(rh)) => {
                    let mut buf = [0u8; 64];
                    buf[..32].copy_from_slice(lh.as_ref());
                    buf[32..].copy_from_slice(rh.as_ref());
                    let digest = Sha256::digest(buf);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&digest);
                    arr
                }
                (Some(lh), None) => *lh.as_ref(),
                (None, Some(_) | None) => return false,
            };
            acc_bytes = combined;
        }
        let computed =
            HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(acc_bytes));
        *root == computed
    }
}

// Specialized helpers for byte-oriented Merkle trees using SHA-256 as the
// node-combining hash. These helpers build a MerkleTree<[u8;32]> whose internal
// nodes store HashOf<[u8;32]>. The underlying bytes for each node are the
// SHA-256 digests of either a leaf chunk, or the concatenation of two child
// nodes. The resulting 32-byte digests are wrapped with Hash::prehashed to
// maintain the Hash invariants (LSB set) while preserving the SHA-256 layout.
impl MerkleTree<[u8; 32]> {
    /// Build a Merkle tree from an iterator of pre-hashed 32-byte leaves.
    /// Each leaf is assumed to be the SHA-256 digest of a chunk, and inner
    /// nodes are computed as SHA-256 of left||right. If a right child is
    /// missing, the left child is promoted unchanged.
    /// Empty input yields an empty tree (no root).
    pub fn from_hashed_leaves_sha256<I>(leaves: I) -> Self
    where
        I: IntoIterator<Item = [u8; 32]>,
    {
        // Mirror the construction used by FromIterator<HashOf<T>> but operate
        // on raw 32-byte digests and then wrap into HashOf at the end.
        let mut queue: std::collections::VecDeque<Option<[u8; 32]>> =
            leaves.into_iter().map(Some).collect();

        let height = Self::height_from_n_leaves(queue.len());
        let n_complement = (1 << height) - queue.len();
        for _ in 0..n_complement {
            queue.push_back(None);
        }

        let mut tree: Vec<Option<[u8; 32]>> = Vec::with_capacity(1 << (height + 1));
        while let Some(r_node) = queue.pop_back() {
            if let Some(l_node) = queue.pop_back() {
                // Keep tree construction consistent with verification which operates
                // on `HashOf::as_ref()` bytes (LSB set by `Hash::prehashed`).
                let parent = match (l_node, r_node) {
                    (Some(mut l), Some(mut r)) => {
                        l[Hash::LENGTH - 1] |= 1;
                        r[Hash::LENGTH - 1] |= 1;
                        let mut buf = [0u8; 64];
                        buf[..32].copy_from_slice(&l);
                        buf[32..].copy_from_slice(&r);
                        let digest = Sha256::digest(buf);
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&digest);
                        Some(arr)
                    }
                    (Some(mut l), None) => {
                        l[Hash::LENGTH - 1] |= 1;
                        Some(l)
                    }
                    (None, Some(_) | None) => None,
                };
                queue.push_front(parent);
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

        // Wrap raw digests into HashOf<[u8;32]> (using Hash::prehashed).
        let inner = tree
            .into_iter()
            .map(|opt| opt.map(|d| HashOf::from_untyped_unchecked(Hash::prehashed(d))))
            .collect();
        Self(inner)
    }

    /// Build a Merkle tree from raw bytes by splitting them into `chunk`-sized
    /// pieces and hashing each zero-padded chunk with SHA-256. The final chunk,
    /// if shorter, is padded with zeros up to `chunk` bytes before hashing.
    /// Inner nodes are combined as SHA-256(left||right).
    ///
    /// # Errors
    ///
    /// Returns [`MerkleError::InvalidChunkSize`] when `chunk` is outside `1..=32`.
    pub fn from_byte_chunks(data: &[u8], chunk: usize) -> Result<Self, MerkleError> {
        validate_chunk_size(chunk)?;

        let mut leaves = Vec::new();
        let mut exact = data.chunks_exact(chunk);
        for c in &mut exact {
            let digest = Sha256::digest(c);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&digest);
            leaves.push(arr);
        }
        let rem = exact.remainder();
        if !rem.is_empty() {
            let mut buf = [0u8; 32];
            buf[..rem.len()].copy_from_slice(rem);
            let digest = Sha256::digest(&buf[..chunk]);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&digest);
            leaves.push(arr);
        }
        if leaves.is_empty() {
            // by convention, at least one zero leaf: hash of `chunk` zero bytes
            let buf = [0u8; 32];
            let digest = Sha256::digest(&buf[..chunk]);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&digest);
            leaves.push(arr);
        }

        Ok(Self::from_hashed_leaves_sha256(leaves))
    }

    /// Build a Merkle tree from an owned vector of pre-hashed 32-byte leaves,
    /// computing internal nodes in parallel. Semantics match
    /// `from_hashed_leaves_sha256` exactly and remain deterministic. Empty input
    /// yields an empty tree (no root).
    #[cfg(feature = "rayon")]
    pub fn from_hashed_leaves_sha256_parallel(leaves: Vec<[u8; 32]>) -> Self {
        use crate::Hash;

        let n = leaves.len();
        let height = Self::height_from_n_leaves(n);
        let pow2 = 1usize << height;
        let capacity = (1usize << (height + 1)) - 1;
        let offset = pow2 - 1; // index of first leaf in BFS layout

        // Initialize BFS node array with None, then place leaves.
        let mut nodes: Vec<Option<[u8; 32]>> = vec![None; capacity];
        for (i, leaf) in leaves.into_iter().enumerate() {
            nodes[offset + i] = Some(leaf);
        }

        // Compute parents bottom-up, level by level, in parallel per level.
        for lvl in (0..height).rev() {
            let start = (1usize << lvl) - 1;
            let end = (1usize << (lvl + 1)) - 1; // exclusive upper bound

            for i in start..end {
                let li = (i << 1) + 1;
                let ri = li + 1;
                let l = nodes.get(li).and_then(|o| *o);
                let r = nodes.get(ri).and_then(|o| *o);
                nodes[i] = match (l, r) {
                    (Some(mut l), Some(mut r)) => {
                        // Match canonical: set prehashed marker on child digests
                        l[Hash::LENGTH - 1] |= 1;
                        r[Hash::LENGTH - 1] |= 1;
                        let mut buf = [0u8; 64];
                        buf[..32].copy_from_slice(&l);
                        buf[32..].copy_from_slice(&r);
                        let digest = Sha256::digest(buf);
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&digest);
                        Some(arr)
                    }
                    (Some(l), None) => Some(l),
                    (None, _) => None,
                };
            }
        }

        let complement = pow2 - n;
        if complement > 0 {
            nodes.truncate(nodes.len() - complement);
        }

        // Wrap raw digests into HashOf<[u8;32]> (using Hash::prehashed)
        let inner = nodes
            .into_iter()
            .map(|opt| opt.map(|d| HashOf::from_untyped_unchecked(Hash::prehashed(d))))
            .collect();
        Self(inner)
    }

    /// Parallel variant of `from_byte_chunks` guarded by the `rayon` feature.
    ///
    /// # Errors
    /// Returns [`MerkleError::InvalidChunkSize`] when `chunk` is outside `1..=32`.
    #[cfg(feature = "rayon")]
    pub fn from_chunked_bytes_parallel(data: &[u8], chunk: usize) -> Result<Self, MerkleError> {
        validate_chunk_size(chunk)?;

        let num_chunks = data.len().div_ceil(chunk);
        let mut leaves = vec![[0u8; 32]; num_chunks.max(1)];
        leaves.par_iter_mut().enumerate().for_each(|(i, slot)| {
            let start = i * chunk;
            if start >= data.len() {
                // only possible when data is empty; compute hash of zero-padded chunk
                let buf = [0u8; 32];
                let digest = Sha256::digest(&buf[..chunk]);
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&digest);
                *slot = arr;
                return;
            }
            let end = (start + chunk).min(data.len());
            let mut buf = [0u8; 32];
            buf[..(end - start)].copy_from_slice(&data[start..end]);
            let digest = Sha256::digest(&buf[..chunk]);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&digest);
            *slot = arr;
        });
        Ok(Self::from_hashed_leaves_sha256_parallel(leaves))
    }

    /// Incrementally update the leaf at `leaf_index` with a new SHA-256 digest
    /// and recompute parent nodes up to the root using SHA-256(left||right)
    /// semantics with left-promotion.
    pub fn update_hashed_leaf_sha256(&mut self, leaf_index: usize, new_digest: [u8; 32]) {
        use crate::Hash;
        // Compute the tree index of the leaf and set it to the new digest
        let height = self.height() as usize;
        let mut idx = MerkleTree::<[u8; 32]>::index_in_tree_unchecked(leaf_index, height);
        let new_leaf = HashOf::from_untyped_unchecked(Hash::prehashed(new_digest));
        if let Some(slot) = self.0.get_mut(idx) {
            *slot = Some(new_leaf);
        } else {
            // Out of bounds leaf update; ignore (or could panic). Keep graceful.
            return;
        }

        // Recompute parents up to the root with SHA-256(left||right)
        while let Some(parent_index) = self.parent_index(idx) {
            let l_opt = self.get_l_child(parent_index).copied();
            let r_opt = self.get_r_child(parent_index).copied();
            let parent_val = match (l_opt, r_opt) {
                (Some(lh), Some(rh)) => {
                    let mut buf = [0u8; 64];
                    buf[..32].copy_from_slice(lh.as_ref());
                    buf[32..].copy_from_slice(rh.as_ref());
                    let digest = Sha256::digest(buf);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&digest);
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(arr)))
                }
                (Some(lh), None) => Some(lh),
                (None, Some(_) | None) => None,
            };
            if let Some(slot) = self.0.get_mut(parent_index) {
                *slot = parent_val;
            }
            idx = parent_index;
        }
    }
}

impl MerkleProof<[u8; 32]> {
    /// Compute the Merkle root implied by this proof and `leaf` using
    /// SHA-256(left || right) semantics. Returns `None` if the proof is
    /// malformed, exceeds `max_height`, or encodes an out-of-range leaf index.
    pub fn compute_root_sha256(
        &self,
        leaf: &HashOf<[u8; 32]>,
        max_height: usize,
    ) -> Option<HashOf<MerkleTree<[u8; 32]>>> {
        let height = self.audit_path.len();
        if max_height < height {
            return None;
        }
        if height < u32::BITS as usize {
            let max_leaves = 1usize << height;
            if self.leaf_index as usize >= max_leaves {
                return None;
            }
        }
        let mut index =
            MerkleTree::<[u8; 32]>::index_in_tree_unchecked(self.leaf_index as usize, height);
        let mut acc_bytes: [u8; 32] = *leaf.as_ref();

        for sibling in &self.audit_path {
            let (l_opt, r_opt) = match index % 2 {
                0 => (
                    sibling.as_ref().copied(),
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(acc_bytes))),
                ),
                1 => (
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(acc_bytes))),
                    sibling.as_ref().copied(),
                ),
                _ => unreachable!(),
            };
            index = index.saturating_sub(1) >> 1;

            let combined = match (l_opt, r_opt) {
                (Some(lh), Some(rh)) => {
                    let mut buf = [0u8; 64];
                    buf[..32].copy_from_slice(lh.as_ref());
                    buf[32..].copy_from_slice(rh.as_ref());
                    let digest = Sha256::digest(buf);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&digest);
                    arr
                }
                (Some(lh), None) => *lh.as_ref(),
                (None, Some(_) | None) => return None,
            };
            acc_bytes = combined;
        }

        Some(HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(
            Hash::prehashed(acc_bytes),
        ))
    }

    /// Verify a proof where inner nodes are combined using SHA-256 of
    /// left||right and leaves are SHA-256 digests. Rejects out-of-range
    /// leaf indices for the implied height.
    pub fn verify_sha256(
        self,
        leaf: &HashOf<[u8; 32]>,
        root: &HashOf<MerkleTree<[u8; 32]>>,
        max_height: usize,
    ) -> bool {
        let height = self.audit_path.len();
        if max_height < height {
            return false;
        }
        if height < u32::BITS as usize {
            let max_leaves = 1usize << height;
            if self.leaf_index as usize >= max_leaves {
                return false;
            }
        }
        let mut index =
            MerkleTree::<[u8; 32]>::index_in_tree_unchecked(self.leaf_index as usize, height);
        let mut acc_bytes: [u8; 32] = *leaf.as_ref();
        for sibling in self.audit_path {
            let (l_opt, r_opt) = match index % 2 {
                0 => (
                    sibling,
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(acc_bytes))),
                ),
                1 => (
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(acc_bytes))),
                    sibling,
                ),
                _ => unreachable!(),
            };
            index = index.saturating_sub(1) >> 1;
            let combined = match (l_opt, r_opt) {
                (Some(lh), Some(rh)) => {
                    let mut buf = [0u8; 64];
                    buf[..32].copy_from_slice(lh.as_ref());
                    buf[32..].copy_from_slice(rh.as_ref());
                    let digest = Sha256::digest(buf);
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&digest);
                    arr
                }
                (Some(lh), None) => *lh.as_ref(),
                (None, Some(_) | None) => return false,
            };
            acc_bytes = combined;
        }
        let computed =
            HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(acc_bytes));
        *root == computed
    }

    /// Construct a SHA-256 proof from raw sibling bytes.
    ///
    /// Conventions:
    /// - The path is ordered from leaf → root.
    /// - A sibling of all-zero bytes represents a missing node (`None`).
    pub fn from_audit_path_bytes(leaf_index: u32, path: Vec<[u8; 32]>) -> Self {
        let audit_path = path
            .into_iter()
            .map(|b| {
                if b == [0u8; 32] {
                    None
                } else {
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(b)))
                }
            })
            .collect();
        MerkleProof {
            leaf_index,
            audit_path,
        }
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

    fn find_deeper_even_step<T>(leaf_index: u32, audit_len: usize) -> Option<usize> {
        let mut index = MerkleTree::<T>::index_in_tree_unchecked(leaf_index as usize, audit_len);
        let mut chosen: Option<usize> = None;
        for step in 0..audit_len {
            if step > 0 && index % 2 == 0 {
                chosen = Some(step);
                break;
            }
            index = index.saturating_sub(1) >> 1;
        }
        chosen
    }

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

    #[test]
    fn merkle_tree_roundtrip() {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
            eprintln!(
                "Skipping: Merkle Norito roundtrip pending Norito opt handling. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        let tree: MerkleTree<_> = test_hashes(3).into_iter().collect();
        let bytes = norito::to_bytes(&tree).expect("encode");
        let archived = norito::from_bytes::<MerkleTree<()>>(&bytes).expect("failed to decode");
        let decoded = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(tree, decoded);
    }

    #[test]
    fn merkle_proof_roundtrip() {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
            eprintln!(
                "Skipping: MerkleProof Norito roundtrip pending Norito opt handling. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        let leaves = test_hashes(3);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let proof = tree.get_proof(1).unwrap();
        let bytes = norito::to_bytes(&proof).expect("encode");
        let archived = norito::from_bytes::<MerkleProof<()>>(&bytes).expect("failed to decode");
        let decoded = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(proof, decoded);
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn generic_parallel_equals_sequential() {
        let leaves = test_hashes(17);
        let seq: MerkleTree<()> = leaves.clone().into_iter().collect();
        let par: MerkleTree<()> = MerkleTree::from_typed_leaves_parallel(leaves);
        assert_eq!(seq.root(), par.root());

        // Compare a few proofs for equality as well
        for &idx in &[0u32, 4, 9, 16] {
            let p1 = seq.get_proof(idx).expect("proof");
            let p2 = par.get_proof(idx).expect("proof");
            assert_eq!(p1.audit_path(), p2.audit_path());
        }
    }

    #[test]
    fn into_audit_path_returns_path() {
        let tree: MerkleTree<_> = test_hashes(3).into_iter().collect();
        let proof = tree.get_proof(1).unwrap();
        let expected = proof.audit_path.clone();
        assert_eq!(proof.into_audit_path(), expected);
    }

    #[test]
    fn byte_tree_from_chunks_matches_manual() {
        // 35 bytes -> 2 leaves for chunk=32 (padded final chunk)
        let data: Vec<u8> = (0..35u8).collect();
        let chunk = 32;

        // Manual leaves: hash first 32 bytes, then hash zero-padded remainder
        let mut l0 = [0u8; 32];
        l0.copy_from_slice(&sha2::Sha256::digest(&data[..chunk]));
        let mut buf = [0u8; 32];
        buf[..(data.len() - chunk)].copy_from_slice(&data[chunk..]);
        let mut l1 = [0u8; 32];
        l1.copy_from_slice(&sha2::Sha256::digest(&buf[..chunk]));
        let manual: MerkleTree<[u8; 32]> = MerkleTree::from_hashed_leaves_sha256([l0, l1]);

        // Helper-based build
        let via_helper =
            MerkleTree::<[u8; 32]>::from_byte_chunks(&data, chunk).expect("valid chunk");

        assert_eq!(manual.root(), via_helper.root());
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn byte_tree_parallel_equals_sequential() {
        let data: Vec<u8> = (0..150u32).map(|i| (i % 251) as u8).collect();
        let a = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let b =
            MerkleTree::<[u8; 32]>::from_chunked_bytes_parallel(&data, 32).expect("valid chunk");
        assert_eq!(a.root(), b.root());
    }

    #[test]
    fn hashed_leaves_sha256_empty_tree_has_no_root() {
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(Vec::new());
        assert!(tree.root().is_none());
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn hashed_leaves_sha256_parallel_empty_tree_has_no_root() {
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256_parallel(Vec::new());
        assert!(tree.root().is_none());
    }

    #[test]
    fn byte_proof_verify_sha256() {
        let data: Vec<u8> = (0..90u32).map(|i| (i % 200) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 1u32;
        let proof = tree.get_proof(idx).expect("proof");
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        assert!(proof.clone().verify_sha256(&leaf, &root, 9));

        // Corrupt the leaf index to invalidate
        let mut bad = proof;
        bad.leaf_index = idx + 1;
        assert!(!bad.verify_sha256(&leaf, &root, 9));
    }

    #[test]
    fn byte_proof_rejects_out_of_range_leaf_index_sha256() {
        let data: Vec<u8> = (0..90u32).map(|i| (i % 200) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 1u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        let height = proof.audit_path.len();
        let height_u32 = u32::try_from(height).expect("height fits in u32");
        proof.leaf_index = 1u32 << height_u32;
        assert!(proof.compute_root_sha256(&leaf, height).is_none());
        assert!(!proof.verify_sha256(&leaf, &root, height));
    }

    #[test]
    fn byte_proof_compute_root_sha256_matches_root() {
        let data: Vec<u8> = (0..64u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 1u32;
        let proof = tree.get_proof(idx).expect("proof");
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let computed = proof
            .compute_root_sha256(&leaf, proof.audit_path().len())
            .expect("computed root");
        assert_eq!(root, computed);
    }

    #[test]
    fn compact_proof_roundtrip_and_verify() {
        let leaves = test_hashes(10);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 7u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let full = tree.get_proof(idx).expect("proof");

        // Compact conversion + verify
        let compact = CompactMerkleProof::from_full(full.clone());
        assert!(compact.clone().verify(&leaf, &root));

        // Expand back to full using the same index and compare audit path
        let expanded = compact.into_full_with_index(idx);
        assert_eq!(full.audit_path(), expanded.audit_path());
    }

    #[test]
    fn compact_proof_dirs_match_leaf_index_bits() {
        let leaves = test_hashes(8);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");

        for idx in 0..8u32 {
            let leaf = tree.leaves().nth(idx as usize).unwrap();
            let full = tree.get_proof(idx).expect("proof");
            let compact = CompactMerkleProof::from_full(full);
            let depth = compact.depth() as usize;
            let mask = (1u64 << depth) - 1;
            assert_eq!(
                u64::from(compact.dirs()),
                u64::from(idx) & mask,
                "dirs mismatch at idx={idx}"
            );
            assert!(compact.clone().verify(&leaf, &root));
        }
    }

    #[test]
    fn compact_proof_sha256_dirs_match_leaf_index_bits() {
        let data: Vec<u8> = (0..128u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 2u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let full = tree.get_proof(idx).expect("proof");
        let compact = CompactMerkleProof::from_full(full);
        let depth = compact.depth() as usize;
        let mask = (1u64 << depth) - 1;
        assert_eq!(u64::from(compact.dirs()), u64::from(idx) & mask);
        assert!(compact.verify_sha256(&leaf, &root));
    }

    #[test]
    fn compact_proof_rejects_sibling_length_mismatch() {
        let leaves = test_hashes(8);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 5u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut compact = CompactMerkleProof::from_full(tree.get_proof(idx).expect("proof"));
        compact.siblings.pop();
        assert!(!compact.verify(&leaf, &root));
    }

    #[test]
    fn compact_proof_truncates_audit_path_to_depth_limit() {
        let sibling: HashOf<()> =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x11; Hash::LENGTH]));
        let full = MerkleProof::from_audit_path(0, vec![Some(sibling); 40]);
        let compact = CompactMerkleProof::from_full(full);
        assert_eq!(compact.depth() as usize, 32);
        assert_eq!(compact.siblings().len(), 32);
    }

    #[test]
    fn compact_proof_rejects_depth_over_32() {
        let leaves = test_hashes(6);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 2u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut compact = CompactMerkleProof::from_full(tree.get_proof(idx).expect("proof"));
        compact.depth = 33;
        while compact.siblings.len() < 33 {
            compact.siblings.push(None);
        }
        assert!(!compact.verify(&leaf, &root));
    }

    #[test]
    fn compact_proof_sha256_rejects_sibling_length_mismatch() {
        let data: Vec<u8> = (0..96u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 1u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut compact = CompactMerkleProof::from_full(tree.get_proof(idx).expect("proof"));
        compact.siblings.pop();
        assert!(!compact.verify_sha256(&leaf, &root));
    }

    #[test]
    fn compact_proof_sha256_rejects_depth_over_32() {
        let data: Vec<u8> = (0..96u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 1u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut compact = CompactMerkleProof::from_full(tree.get_proof(idx).expect("proof"));
        compact.depth = 33;
        while compact.siblings.len() < 33 {
            compact.siblings.push(None);
        }
        assert!(!compact.verify_sha256(&leaf, &root));
    }

    #[test]
    fn byte_proof_tamper_audit_path_fails_sha256() {
        let data: Vec<u8> = (0..120u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 2u32;
        let mut proof = tree.get_proof(idx).expect("proof");
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        assert!(proof.clone().verify_sha256(&leaf, &root, 9));

        // Tamper with the first sibling (if any), otherwise set first path element.
        if let Some(slot) = proof.audit_path.iter_mut().find(|s| s.is_some()) {
            *slot = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0xAA; Hash::LENGTH],
            )));
        } else if !proof.audit_path.is_empty() {
            proof.audit_path[0] = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0x55; Hash::LENGTH],
            )));
        }
        assert!(!proof.verify_sha256(&leaf, &root, 9));
    }

    #[test]
    fn proof_truncated_path_fails() {
        let leaves = test_hashes(7);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 3u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        // Remove the last sibling in the path (toward the root)
        proof.audit_path.pop();
        assert!(!proof.verify(&leaf, &root, 9));
    }

    #[test]
    fn proof_rejects_out_of_range_leaf_index() {
        let leaves = test_hashes(6);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 1u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        let height = proof.audit_path.len();
        let height_u32 = u32::try_from(height).expect("height fits in u32");
        proof.leaf_index = 1u32 << height_u32;
        assert!(!proof.verify(&leaf, &root, height));
    }

    #[test]
    fn proof_extended_path_fails() {
        let leaves = test_hashes(6);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 2u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        // Append an extra bogus sibling at the end of the path
        proof
            .audit_path
            .push(Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0x42; Hash::LENGTH],
            ))));
        assert!(!proof.verify(&leaf, &root, 9));
    }

    #[test]
    fn byte_proof_reversed_order_fails_sha256() {
        let data: Vec<u8> = (0..120u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 2u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        // Reverse the audit path orientation (root->leaf instead of leaf->root)
        proof.audit_path.reverse();
        assert!(!proof.verify_sha256(&leaf, &root, 9));
    }

    #[test]
    fn proof_right_only_child_fails() {
        let leaves = test_hashes(5);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        // Choose an index with even initial tree index so that (l=None, r=Some) occurs when sibling is None.
        let idx = 1u32; // ensures even parity at first step for typical heights
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        if !proof.audit_path.is_empty() {
            proof.audit_path[0] = None;
        }
        assert!(!proof.verify(&leaf, &root, 9));
    }

    #[test]
    fn byte_proof_right_only_child_fails_sha256() {
        let data: Vec<u8> = (0..96u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 1u32; // even parity at first step
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        if !proof.audit_path.is_empty() {
            proof.audit_path[0] = None;
        }
        assert!(!proof.verify_sha256(&leaf, &root, 9));
    }

    #[test]
    fn proof_right_only_child_deeper_fails() {
        // Build a tree with enough height to have deeper steps
        let leaves = test_hashes(9);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");

        // Pick an index that has an even step at depth > 0; try candidates
        let mut picked = None;
        for idx in 0..u32::try_from(leaves.len()).expect("leaves length fits in u32") {
            if let Some(step) = find_deeper_even_step::<()>(idx, tree.height() as usize) {
                picked = Some((idx, step));
                break;
            }
        }
        let (idx, step) = picked.expect("found a deeper even step");
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        // Force a right-only child at the chosen deeper step
        proof.audit_path[step] = None;
        assert!(!proof.verify(&leaf, &root, 9));
    }

    #[test]
    fn typed_incremental_update_matches_rebuild() {
        // Build initial tree from typed leaves
        let mut leaves: Vec<HashOf<()>> = (0u8..16)
            .map(|i| Hash::prehashed([i; Hash::LENGTH]))
            .map(HashOf::from_untyped_unchecked)
            .collect();
        let mut tree: MerkleTree<()> = leaves.clone().into_iter().collect();
        let baseline_root = tree.root().expect("root");

        // Apply two updates using incremental typed API
        let new_a = HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; Hash::LENGTH]));
        let new_b = HashOf::from_untyped_unchecked(Hash::prehashed([0xBB; Hash::LENGTH]));
        tree.update_typed_leaf(3, new_a);
        tree.update_typed_leaf(11, new_b);

        // Build a fresh tree from updated leaves for conformance
        leaves[3] = new_a;
        leaves[11] = new_b;
        let rebuilt: MerkleTree<()> = leaves.clone().into_iter().collect();

        assert_ne!(baseline_root, rebuilt.root().expect("root"));
        assert_eq!(tree.root(), rebuilt.root());

        // Compare proofs at a few indices
        for &idx in &[0u32, 3, 7, 11, 15] {
            let p1 = tree.get_proof(idx).expect("proof");
            let p2 = rebuilt.get_proof(idx).expect("proof");
            assert_eq!(p1.audit_path(), p2.audit_path());
        }
    }

    #[test]
    fn byte_proof_right_only_child_deeper_fails_sha256() {
        let data: Vec<u8> = (0..320u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");

        let n = data.len().div_ceil(32);
        let mut picked = None;
        for idx in 0..u32::try_from(n).expect("n fits in u32") {
            if let Some(step) = find_deeper_even_step::<[u8; 32]>(idx, tree.height() as usize) {
                picked = Some((idx, step));
                break;
            }
        }
        let (idx, step) = picked.expect("found a deeper even step for bytes");
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let mut proof = tree.get_proof(idx).expect("proof");
        proof.audit_path[step] = None;
        assert!(!proof.verify_sha256(&leaf, &root, 9));
    }

    #[test]
    fn proof_max_height_too_small_fails() {
        let leaves = test_hashes(5);
        let tree: MerkleTree<_> = leaves.clone().into_iter().collect();
        let root = tree.root().expect("root");
        let idx = 2u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let proof = tree.get_proof(idx).expect("proof");
        let too_small = proof.audit_path.len().saturating_sub(1);
        assert!(!proof.verify(&leaf, &root, too_small));
    }

    #[test]
    fn byte_proof_max_height_too_small_fails_sha256() {
        let data: Vec<u8> = (0..96u32).map(|i| (i % 251) as u8).collect();
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).expect("valid chunk");
        let root = tree.root().expect("root");
        let idx = 2u32;
        let leaf = tree.leaves().nth(idx as usize).unwrap();
        let proof = tree.get_proof(idx).expect("proof");
        let too_small = proof.audit_path.len().saturating_sub(1);
        assert!(!proof.verify_sha256(&leaf, &root, too_small));
    }

    #[test]
    fn from_byte_chunks_rejects_invalid_chunk_size() {
        let data = [0u8; 4];
        assert!(matches!(
            MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 0),
            Err(MerkleError::InvalidChunkSize { .. })
        ));
        assert!(matches!(
            MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 64),
            Err(MerkleError::InvalidChunkSize { .. })
        ));
    }

    #[cfg(feature = "rayon")]
    #[test]
    fn from_chunked_bytes_parallel_rejects_invalid_chunk_size() {
        let data = [0u8; 4];
        assert!(matches!(
            MerkleTree::<[u8; 32]>::from_chunked_bytes_parallel(&data, 0),
            Err(MerkleError::InvalidChunkSize { .. })
        ));
    }

    #[test]
    fn merkle_proof_verify_rejects_oversized_height() {
        let leaf = HashOf::from_untyped_unchecked(Hash::prehashed([0x11; Hash::LENGTH]));
        let root =
            HashOf::<MerkleTree<()>>::from_untyped_unchecked(Hash::prehashed([0x22; Hash::LENGTH]));
        let proof = MerkleProof::from_audit_path(0, vec![None; usize::BITS as usize]);
        assert!(!proof.verify(&leaf, &root, usize::BITS as usize));
    }

    #[test]
    fn merkle_tree_decode_rejects_invalid_layout() {
        let bad_leaf = HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; Hash::LENGTH]));
        let bad_tree = MerkleTree::<()>(vec![Some(bad_leaf), Some(bad_leaf)]);
        let bytes = norito::to_bytes(&bad_tree).expect("encode merkle tree");
        let err = norito::decode_from_bytes::<MerkleTree<()>>(&bytes)
            .expect_err("invalid merkle layout should fail");
        assert!(matches!(err, norito::Error::Message(_)));
    }

    #[test]
    fn merkle_tree_decode_rejects_missing_nodes() {
        let bad_tree = MerkleTree::<()>(vec![None]);
        let bytes = norito::to_bytes(&bad_tree).expect("encode merkle tree");
        let err = norito::decode_from_bytes::<MerkleTree<()>>(&bytes)
            .expect_err("missing nodes should fail");
        assert!(matches!(err, norito::Error::Message(_)));
    }

    #[test]
    fn merkle_tree_decode_rejects_missing_parent() {
        let leaf = HashOf::from_untyped_unchecked(Hash::prehashed([0xAB; Hash::LENGTH]));
        let bad_tree = MerkleTree::<()>(vec![
            Some(leaf),
            None,
            Some(leaf),
            Some(leaf),
            Some(leaf),
            Some(leaf),
            Some(leaf),
        ]);
        let bytes = norito::to_bytes(&bad_tree).expect("encode merkle tree");
        let err = norito::decode_from_bytes::<MerkleTree<()>>(&bytes)
            .expect_err("missing parent should fail");
        assert!(matches!(err, norito::Error::Message(_)));
    }
}
