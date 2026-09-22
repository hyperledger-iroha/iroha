//! Authenticated lookup and updates through externally owned immutable nodes.
//!
//! The root envelope retains no node graph. Callers own storage, allocation
//! admission and root authority; this module never infers absence from a missing
//! node. These are logical descriptors, not a disk or wire encoding.

use super::{
    Hash, MerkleMap, Node, NodeKind, branch_hash, common_prefix, leaf_hash, prefix, root_hash,
};

#[path = "update.rs"]
mod update;
pub use update::{
    MerkleMapEdit, MerkleMapNodeStore, MerkleMapUpdateError, MerkleMapUpdateWorkspace,
};

const MAX_PATH_NODES: usize = Hash::LENGTH * 8 + 1;

/// Expected logical node identity and an explicit, untrusted physical location.
/// The caller retains the backing generation; a location never establishes
/// content authority and is excluded from every logical commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MerkleMapNodeRef<N> {
    /// Node hash authenticated by its parent or the original root owner.
    pub hash: Hash,
    /// Opaque store-owned location, including any required generation identity.
    pub location: N,
}

/// Authenticated logical value identity with its explicit physical location.
/// The value owner must load this exact location and verify the preimage hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MerkleMapValueRef<V> {
    /// Domain-separated logical value commitment.
    pub hash: Hash,
    /// Opaque location retained by the original store/root lease.
    pub location: V,
}

/// A fixed-size envelope for one immutable canonical map version.
///
/// It contains no resident nodes. Its hash must be authenticated by the caller's
/// original State/publication owner before it can authorize a membership answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MerkleMapRoot<N> {
    len: u64,
    node: Option<MerkleMapNodeRef<N>>,
}

/// One immutable node of the existing compressed binary radix tree.
///
/// Raw prefixes are byte arrays: applying the `Hash` low-bit marker to a prefix
/// would change its meaning. This descriptor defines no serialization format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MerkleMapNode<N, V> {
    /// A complete public key/value binding.
    Leaf {
        /// Canonical, domain-separated key hash.
        key: Hash,
        /// Canonical value hash and its explicit physical location.
        value: MerkleMapValueRef<V>,
    },
    /// Two ordered subtrees split at one MSB-first key bit.
    Branch {
        /// The split bit, in `0..256`; later bits belong to descendants.
        bit: u16,
        /// Shared key prefix; every bit at or after `bit` is zero.
        prefix: [u8; Hash::LENGTH],
        /// Root of the subtree whose split bit is zero.
        left: MerkleMapNodeRef<N>,
        /// Root of the subtree whose split bit is one.
        right: MerkleMapNodeRef<N>,
    },
}

/// A local lookup failure is distinct from authenticated key absence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MerkleMapReadError<E> {
    /// The envelope does not match the independently authenticated root.
    #[error("Merkle map root does not match its authenticated owner")]
    RootMismatch,
    /// Empty-node and entry-count metadata are inconsistent.
    #[error("Merkle map root has inconsistent entry-count metadata")]
    InvalidRoot,
    /// The external owner could not read a node.
    #[error("Merkle map node read failed: {0}")]
    Source(E),
    /// A referenced node was absent from external storage.
    #[error("Merkle map is missing node {0}")]
    MissingNode(Hash),
    /// Loaded content does not match its authenticated parent reference.
    #[error("Merkle map node content does not match reference {0}")]
    NodeHashMismatch(Hash),
    /// Loaded content violates canonical prefix, depth or child-path constraints.
    #[error("Merkle map node violates its canonical search path")]
    InvalidPath,
}

impl<N: Copy> MerkleMapRoot<N> {
    /// Construct untrusted root metadata; `lookup` still checks an independent root.
    pub fn from_parts(len: u64, node: Option<MerkleMapNodeRef<N>>) -> Self {
        Self { len, node }
    }

    /// Return the exact entry count and optional top-node reference.
    pub fn parts(&self) -> (u64, Option<MerkleMapNodeRef<N>>) {
        (self.len, self.node)
    }

    /// Compute the existing map commitment; this does not authenticate its origin.
    pub fn hash(&self) -> Hash {
        root_hash(self.len, self.node.map(|node| node.hash))
    }

    /// Authenticate one lookup using at most 257 fixed-size node reads.
    ///
    /// The kernel is iterative, retains one parent descriptor, and allocates no
    /// history or search-path container. The loader owns and bounds its I/O and
    /// allocations. It returns `Ok(None)` only for a missing stored node; that
    /// is reported as `MissingNode`, never as an absent key. An absent key is
    /// established only by an authenticated empty root, divergent compressed
    /// prefix, or different leaf. No loader call occurs for an invalid envelope.
    ///
    /// # Errors
    /// Returns a distinct root, source, missing-node, content or path failure.
    /// The caller must not translate these local failures into non-membership.
    pub fn lookup<V: Copy, E>(
        &self,
        expected_root: &Hash,
        key: &Hash,
        mut load: impl FnMut(&MerkleMapNodeRef<N>) -> Result<Option<MerkleMapNode<N, V>>, E>,
    ) -> Result<Option<MerkleMapValueRef<V>>, MerkleMapReadError<E>> {
        if self.hash() != *expected_root {
            return Err(MerkleMapReadError::RootMismatch);
        }
        if (self.len == 0) != self.node.is_none() {
            return Err(MerkleMapReadError::InvalidRoot);
        }
        let Some(mut reference) = self.node else {
            return Ok(None);
        };
        let mut parent: Option<(u16, [u8; Hash::LENGTH], bool)> = None;
        for _ in 0..MAX_PATH_NODES {
            let node = load(&reference)
                .map_err(MerkleMapReadError::Source)?
                .ok_or(MerkleMapReadError::MissingNode(reference.hash))?;
            if node.hash() != reference.hash {
                return Err(MerkleMapReadError::NodeHashMismatch(reference.hash));
            }
            if parent.is_none() && matches!(node, MerkleMapNode::Leaf { .. }) && self.len != 1 {
                return Err(MerkleMapReadError::InvalidPath);
            }
            let (first, depth) = match &node {
                MerkleMapNode::Leaf { key, .. } => (key.as_ref(), 256),
                MerkleMapNode::Branch {
                    bit,
                    prefix,
                    left,
                    right,
                } => {
                    if *bit >= 256
                        || !canonical_prefix(prefix, *bit)
                        || self.len < 2
                        || left.hash == right.hash
                    {
                        return Err(MerkleMapReadError::InvalidPath);
                    }
                    (prefix, *bit)
                }
            };
            if let Some((bit, prefix, right)) = parent {
                if depth <= bit
                    || common_prefix(first, &prefix) < bit
                    || raw_bit(first, bit) != right
                {
                    return Err(MerkleMapReadError::InvalidPath);
                }
            }
            match node {
                MerkleMapNode::Leaf { key: found, value } => {
                    return Ok((found == *key).then_some(value));
                }
                MerkleMapNode::Branch {
                    bit,
                    prefix,
                    left,
                    right,
                } => {
                    if common_prefix(key.as_ref(), &prefix) < bit {
                        return Ok(None);
                    }
                    let take_right = raw_bit(key.as_ref(), bit);
                    parent = Some((bit, prefix, take_right));
                    reference = if take_right { right } else { left };
                }
            }
        }
        Err(MerkleMapReadError::InvalidPath)
    }
}

impl<N, V> MerkleMapNode<N, V> {
    /// Hash these exact logical fields using the existing tree's domains.
    ///
    /// Hash equality alone is not canonical-shape validation. `lookup` also
    /// validates the node's prefix and its relationship to the original path.
    pub fn hash(&self) -> Hash {
        match self {
            Self::Leaf { key, value } => leaf_hash(*key, value.hash),
            Self::Branch {
                bit,
                prefix,
                left,
                right,
            } => branch_hash(*bit, prefix, left.hash, right.hash),
        }
    }
}

impl MerkleMap {
    /// Export this exact version in child-before-parent order with real locations.
    ///
    /// The original store owns persistence/admission and stops on its own error.
    /// The value resolver supplies an explicit location for each logical value;
    /// it borrows that same store rather than an implicit last-read cache. Its
    /// preimage still needs authentication when consumed by the value reader.
    /// Traversal borrows the immutable graph and allocates no export collection;
    /// recursive depth is bounded by the 256-bit key space. The original child
    /// references stay in bounded traversal frames, never a history-sized address
    /// map. The caller must admit these frames for its concrete location types.
    /// The result retains locations but does not establish durability or authority.
    ///
    /// # Errors
    /// Returns the original resolver/write error without changing this version.
    pub fn export_nodes<S: MerkleMapNodeStore>(
        &self,
        store: &mut S,
        mut locate_value: impl FnMut(&mut S, Hash) -> Result<S::ValueLocation, S::Error>,
    ) -> Result<MerkleMapRoot<S::NodeLocation>, S::Error> {
        fn walk<S: MerkleMapNodeStore>(
            node: &Node,
            store: &mut S,
            locate_value: &mut impl FnMut(&mut S, Hash) -> Result<S::ValueLocation, S::Error>,
        ) -> Result<MerkleMapNodeRef<S::NodeLocation>, S::Error> {
            let descriptor = match &node.kind {
                NodeKind::Leaf(value) => MerkleMapNode::Leaf {
                    key: node.first,
                    value: MerkleMapValueRef {
                        hash: *value,
                        location: locate_value(store, *value)?,
                    },
                },
                NodeKind::Branch { bit, left, right } => {
                    let left = walk(left, store, locate_value)?;
                    let right = walk(right, store, locate_value)?;
                    MerkleMapNode::Branch {
                        bit: *bit,
                        prefix: prefix(&node.first, *bit),
                        left,
                        right,
                    }
                }
            };
            Ok(MerkleMapNodeRef {
                hash: node.hash,
                location: store.write(descriptor)?,
            })
        }
        let node = self
            .node
            .as_ref()
            .map(|node| walk(node, store, &mut locate_value))
            .transpose()?;
        Ok(MerkleMapRoot::from_parts(self.len, node))
    }
}

fn raw_bit(bytes: &[u8; Hash::LENGTH], bit: u16) -> bool {
    bytes[usize::from(bit / 8)] & (0x80 >> (bit % 8)) != 0
}

fn canonical_prefix(prefix: &[u8; Hash::LENGTH], bit: u16) -> bool {
    let byte = usize::from(bit / 8);
    let mask = 0xff_u8 >> (bit % 8);
    prefix[byte] & mask == 0 && prefix[byte + 1..].iter().all(|byte| *byte == 0)
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
