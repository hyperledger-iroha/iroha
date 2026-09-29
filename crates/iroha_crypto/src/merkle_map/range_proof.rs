//! Complete, bounded half-open range witnesses for one authenticated map root.
//!
//! Expanded branches cover every key-space partition intersecting the requested
//! range. A sibling may remain hash-only only when its entire partition lies
//! outside that range. The caller must independently authenticate the root;
//! this proof alone does not establish State finality or table completeness.
//! Bounds use the 32-byte hashed-key order, which is not Norito-key order.

use super::{
    Hash, MerkleMap, MerkleMapNode, MerkleMapNodeRef, MerkleMapValueRef, Node, NodeKind,
    common_prefix, prefix, root_hash,
};

const PRUNED: u32 = u32::MAX;
/// Maximum number of returned rows in one complete range witness.
pub const MAX_RANGE_PROOF_ROWS: usize = 8_192;
const BOUNDARY_PATH_NODES: usize = 2 * Hash::LENGTH * 8 + 1;

/// Failure to construct or verify a complete map range witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MerkleMapRangeError {
    /// The half-open interval is empty or inverted.
    #[error("Merkle map range bounds must be strictly increasing")]
    InvalidBounds,
    /// The requested row ceiling exceeds the fixed witness limit.
    #[error("Merkle map range row limit exceeds the V1 ceiling")]
    InvalidLimit,
    /// The range contains more rows or nodes than the admitted bound.
    #[error("Merkle map range witness exceeds its admitted bound")]
    Capacity,
    /// A local allocation could not be reserved before proof construction.
    #[error("Merkle map range witness allocation failed")]
    Allocation,
    /// The claimed map root differs from the independently supplied root.
    #[error("Merkle map range root differs from its authenticated owner")]
    RootMismatch,
    /// A node, path, pruned child or result set is incomplete or malformed.
    #[error("Merkle map range witness is incomplete or malformed")]
    InvalidProof,
}

/// An authenticated complete interval over a canonical compressed Merkle map.
///
/// Node locations are local indices, not commitments or durable store offsets.
/// The proof is bounded by `2 * max_rows + 513` nodes; a pruned child carries
/// its parent-authenticated hash and the reserved `u32::MAX` location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerkleMapRangeProof {
    entries: u64,
    top: Option<Hash>,
    nodes: Vec<Option<MerkleMapNode<u32, ()>>>,
}

fn node_limit(max_rows: usize) -> Result<usize, MerkleMapRangeError> {
    if max_rows > MAX_RANGE_PROOF_ROWS {
        return Err(MerkleMapRangeError::InvalidLimit);
    }
    Ok(max_rows * 2 + BOUNDARY_PATH_NODES)
}

fn bit(bytes: &[u8; Hash::LENGTH], index: u16) -> bool {
    bytes[usize::from(index / 8)] & (0x80 >> (index % 8)) != 0
}

fn partition_intersects(
    branch_prefix: &[u8; Hash::LENGTH],
    split_bit: u16,
    right: bool,
    start: &Hash,
    end: &Hash,
) -> bool {
    let mut lower = *branch_prefix;
    let byte = usize::from(split_bit / 8);
    let mask = 0x80 >> (split_bit % 8);
    if right {
        lower[byte] |= mask;
    }
    let mut upper = lower;
    upper[byte] |= mask - 1;
    upper[byte + 1..].fill(0xff);
    upper >= *start.as_ref() && lower < *end.as_ref()
}

fn canonical_prefix(bytes: &[u8; Hash::LENGTH], depth: u16) -> bool {
    let byte = usize::from(depth / 8);
    let mut canonical = *bytes;
    canonical[byte] = if depth.is_multiple_of(8) {
        0
    } else {
        canonical[byte] & (0xff << (8 - depth % 8))
    };
    canonical[byte + 1..].fill(0);
    canonical == *bytes
}

fn reserve_node(
    nodes: &mut Vec<Option<MerkleMapNode<u32, ()>>>,
    maximum: usize,
) -> Result<usize, MerkleMapRangeError> {
    if nodes.len() >= maximum {
        return Err(MerkleMapRangeError::Capacity);
    }
    nodes
        .try_reserve(1)
        .map_err(|_| MerkleMapRangeError::Allocation)?;
    let index = nodes.len();
    nodes.push(None);
    Ok(index)
}

fn export_node(
    node: &Node,
    start: &Hash,
    end: &Hash,
    max_rows: usize,
    max_nodes: usize,
    rows: &mut usize,
    nodes: &mut Vec<Option<MerkleMapNode<u32, ()>>>,
) -> Result<u32, MerkleMapRangeError> {
    let index = reserve_node(nodes, max_nodes)?;
    let descriptor = match &node.kind {
        NodeKind::Leaf(value) => {
            if *start <= node.first && node.first < *end {
                *rows += 1;
                if *rows > max_rows {
                    return Err(MerkleMapRangeError::Capacity);
                }
            }
            MerkleMapNode::Leaf {
                key: node.first,
                value: MerkleMapValueRef {
                    hash: *value,
                    location: (),
                },
            }
        }
        NodeKind::Branch { bit, left, right } => {
            let branch_prefix = prefix(&node.first, *bit);
            let mut child = |side: bool, subtree: &Node| {
                let location = if partition_intersects(&branch_prefix, *bit, side, start, end) {
                    export_node(subtree, start, end, max_rows, max_nodes, rows, nodes)?
                } else {
                    PRUNED
                };
                Ok(MerkleMapNodeRef {
                    hash: subtree.hash,
                    location,
                })
            };
            let left = child(false, left)?;
            let right = child(true, right)?;
            MerkleMapNode::Branch {
                bit: *bit,
                prefix: branch_prefix,
                left,
                right,
            }
        }
    };
    nodes[index] = Some(descriptor);
    u32::try_from(index).map_err(|_| MerkleMapRangeError::Capacity)
}

impl MerkleMap {
    /// Export every key/value hash in `[start, end)` with authenticated exclusion.
    ///
    /// The result is an untrusted proof until verified against an independently
    /// owned root. Growth is fallibly reserved before every node allocation.
    ///
    /// # Errors
    /// Rejects invalid bounds or limits, excessive rows/nodes, or allocation
    /// refusal without changing the map.
    pub fn prove_range(
        &self,
        start: &Hash,
        end: &Hash,
        max_rows: usize,
    ) -> Result<MerkleMapRangeProof, MerkleMapRangeError> {
        if start >= end {
            return Err(MerkleMapRangeError::InvalidBounds);
        }
        let max_nodes = node_limit(max_rows)?;
        let mut proof = MerkleMapRangeProof {
            entries: self.len,
            top: self.node.as_ref().map(|node| node.hash),
            nodes: Vec::new(),
        };
        if let Some(root) = self.node.as_deref() {
            let mut rows = 0;
            let index = export_node(
                root,
                start,
                end,
                max_rows,
                max_nodes,
                &mut rows,
                &mut proof.nodes,
            )?;
            debug_assert_eq!(index, 0);
        }
        Ok(proof)
    }
}

struct Verify<'a> {
    proof: &'a MerkleMapRangeProof,
    start: &'a Hash,
    end: &'a Hash,
    max_rows: usize,
    visited: Vec<bool>,
    rows: Vec<(Hash, Hash)>,
}

impl Verify<'_> {
    fn visit(
        &mut self,
        index: u32,
        expected_hash: Hash,
        parent: Option<(u16, [u8; Hash::LENGTH], bool)>,
    ) -> Result<(), MerkleMapRangeError> {
        let index = usize::try_from(index).map_err(|_| MerkleMapRangeError::InvalidProof)?;
        let visited = self
            .visited
            .get_mut(index)
            .ok_or(MerkleMapRangeError::InvalidProof)?;
        if *visited {
            return Err(MerkleMapRangeError::InvalidProof);
        }
        *visited = true;
        let node = self
            .proof
            .nodes
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(MerkleMapRangeError::InvalidProof)?;
        if node.hash() != expected_hash {
            return Err(MerkleMapRangeError::InvalidProof);
        }
        let (first, depth) = match node {
            MerkleMapNode::Leaf { key, .. } => (key.as_ref(), 256),
            MerkleMapNode::Branch {
                bit,
                prefix,
                left,
                right,
            } => {
                if *bit >= 256
                    || !canonical_prefix(prefix, *bit)
                    || left.hash == right.hash
                    || self.proof.entries < 2
                {
                    return Err(MerkleMapRangeError::InvalidProof);
                }
                (prefix, *bit)
            }
        };
        if let Some((parent_bit, parent_prefix, right)) = parent {
            if depth <= parent_bit
                || common_prefix(first, &parent_prefix) < parent_bit
                || bit(first, parent_bit) != right
            {
                return Err(MerkleMapRangeError::InvalidProof);
            }
        } else if matches!(node, MerkleMapNode::Leaf { .. }) && self.proof.entries != 1 {
            return Err(MerkleMapRangeError::InvalidProof);
        }
        match node {
            MerkleMapNode::Leaf { key, value } => {
                if *self.start <= *key && *key < *self.end {
                    if self.rows.len() >= self.max_rows {
                        return Err(MerkleMapRangeError::Capacity);
                    }
                    self.rows
                        .try_reserve(1)
                        .map_err(|_| MerkleMapRangeError::Allocation)?;
                    self.rows.push((*key, value.hash));
                }
            }
            MerkleMapNode::Branch {
                bit,
                prefix,
                left,
                right,
            } => {
                for (side, child) in [(false, left), (true, right)] {
                    let intersects = partition_intersects(prefix, *bit, side, self.start, self.end);
                    if intersects {
                        if child.location == PRUNED {
                            return Err(MerkleMapRangeError::InvalidProof);
                        }
                        self.visit(child.location, child.hash, Some((*bit, *prefix, side)))?;
                    } else if child.location != PRUNED {
                        return Err(MerkleMapRangeError::InvalidProof);
                    }
                }
            }
        }
        Ok(())
    }
}

impl MerkleMapRangeProof {
    /// Return the untrusted entry count committed in this proof's claimed root.
    #[must_use]
    pub fn entry_count(&self) -> u64 {
        self.entries
    }

    /// Authenticate every row in `[start, end)` against an independently owned root.
    ///
    /// The result is in canonical key order. An omitted in-range subtree,
    /// malformed compressed path, duplicate node, extra node or wrong root is
    /// an error. The returned value hashes still require their own preimages
    /// when a caller needs decoded values.
    ///
    /// # Errors
    /// Rejects invalid bounds, row/node limits, root mismatch, incomplete
    /// proof structure or local allocation refusal.
    pub fn verify(
        &self,
        expected_root: &Hash,
        start: &Hash,
        end: &Hash,
        max_rows: usize,
    ) -> Result<Vec<(Hash, Hash)>, MerkleMapRangeError> {
        if start >= end {
            return Err(MerkleMapRangeError::InvalidBounds);
        }
        if self.nodes.len() > node_limit(max_rows)? {
            return Err(MerkleMapRangeError::Capacity);
        }
        if root_hash(self.entries, self.top) != *expected_root {
            return Err(MerkleMapRangeError::RootMismatch);
        }
        if (self.entries == 0) != self.top.is_none() {
            return Err(MerkleMapRangeError::InvalidProof);
        }
        let Some(top) = self.top else {
            return self
                .nodes
                .is_empty()
                .then(Vec::new)
                .ok_or(MerkleMapRangeError::InvalidProof);
        };
        let mut visited = Vec::new();
        visited
            .try_reserve_exact(self.nodes.len())
            .map_err(|_| MerkleMapRangeError::Allocation)?;
        visited.resize(self.nodes.len(), false);
        let mut verifier = Verify {
            proof: self,
            start,
            end,
            max_rows,
            visited,
            rows: Vec::new(),
        };
        verifier.visit(0, top, None)?;
        if verifier.visited.iter().any(|visited| !visited) {
            return Err(MerkleMapRangeError::InvalidProof);
        }
        Ok(verifier.rows)
    }
}

#[cfg(test)]
#[path = "range_proof/tests.rs"]
mod tests;
