//! Persistent canonical commitments to maps of public key/value hashes.
//!
//! A binary radix tree compresses every unary path. Each branch commits its
//! split bit, the shared key prefix, and its ordered children. Its shape depends
//! only on the current keys, never insertion order. This is a separate commitment
//! format from the indexed `MerkleTree` and the full-depth sparse Merkle tree.
//! It has no wire encoding or externally supplied nodes/proofs.
//!
//! Clones share immutable nodes. A mutation copies at most one 256-bit search
//! path; it never scans unrelated entries. Allocator failure is not recoverable
//! here, so callers must still provide their own aggregate resource admission.
//! Keys and tree shape are public: lookup is intentionally not constant-time.

use std::sync::Arc;

use crate::Hash;

const EMPTY: &[u8] = b"iroha:merkle-map:empty:v1\0";
const LEAF: &[u8] = b"iroha:merkle-map:leaf:v1\0";
const BRANCH: &[u8] = b"iroha:merkle-map:branch:v1\0";
const ROOT: &[u8] = b"iroha:merkle-map:root:v1\0";

/// An immutable-node, history-independent map commitment with cheap snapshots.
///
/// The owner supplies canonical, domain-separated key and value hashes. Updates
/// compare the expected old value before changing anything; snapshots remain
/// valid after updates to their descendants. This is not persistence to disk.
#[derive(Clone, Default)]
pub struct MerkleMap {
    node: Option<Arc<Node>>,
    len: u64,
}

/// A rejected update leaves the entire map and its root unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MerkleMapError {
    /// The caller's preimage does not match this map version.
    #[error("Merkle map preimage mismatch: expected {expected:?}, found {actual:?}")]
    PreimageMismatch {
        /// Value asserted by the caller, or absence.
        expected: Option<Hash>,
        /// Value present in this version, or absence.
        actual: Option<Hash>,
    },
    /// The number of entries cannot be represented by the commitment format.
    #[error("Merkle map entry count overflow")]
    Capacity,
}

struct Node {
    hash: Hash,
    /// Smallest key in this subtree; its prefix authenticates compressed paths.
    first: Hash,
    kind: NodeKind,
}

enum NodeKind {
    Leaf(Hash),
    Branch {
        bit: u16,
        left: Arc<Node>,
        right: Arc<Node>,
    },
}

impl MerkleMap {
    /// Construct an empty commitment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of present keys, including keys whose values hash empty payloads.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether the map contains no keys.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bind the exact current key/value set and its entry count in constant time.
    pub fn root(&self) -> Hash {
        let node = self
            .node
            .as_ref()
            .map_or_else(|| Hash::new(EMPTY), |n| n.hash);
        Hash::new_from_chunks(&[ROOT, &self.len.to_le_bytes(), node.as_ref()])
    }

    /// Look up a public key without copying any nodes or encoded values.
    pub fn get(&self, key: &Hash) -> Option<Hash> {
        let mut node = self.node.as_deref()?;
        loop {
            match &node.kind {
                NodeKind::Leaf(value) => return (node.first == *key).then_some(*value),
                NodeKind::Branch { bit, left, right } => {
                    if common_bits(key, &node.first) < *bit {
                        return None;
                    }
                    node = if key_bit(key, *bit) { right } else { left };
                }
            }
        }
    }

    /// Replace one value only if its expected preimage matches this version.
    ///
    /// `None` denotes absence, so insertion, removal and empty values remain
    /// distinct. Returns `false` for an exact no-op. Errors leave the map intact.
    /// A successful update copies at most 256 branches and one leaf, with no
    /// whole-map reconstruction. Hashing uses the existing portable `Hash` path.
    ///
    /// # Errors
    /// Rejects a mismatched expected value or an update whose entry count cannot
    /// be represented by the commitment format.
    pub fn replace(
        &mut self,
        key: Hash,
        expected: Option<Hash>,
        after: Option<Hash>,
    ) -> Result<bool, MerkleMapError> {
        let actual = self.get(&key);
        if actual != expected {
            return Err(MerkleMapError::PreimageMismatch { expected, actual });
        }
        if expected == after {
            return Ok(false);
        }
        let len = match (expected, after) {
            (None, Some(_)) => self.len.checked_add(1).ok_or(MerkleMapError::Capacity)?,
            (Some(_), None) => self.len.checked_sub(1).ok_or(MerkleMapError::Capacity)?,
            _ => self.len,
        };
        let node = replace_node(self.node.as_ref(), key, after);
        self.node = node;
        self.len = len;
        Ok(true)
    }
}

/// MSB-first shared prefix length, including all 256 bits for equal keys.
fn common_bits(a: &Hash, b: &Hash) -> u16 {
    for (index, (&a, &b)) in (0_u16..).zip(a.as_ref().iter().zip(b.as_ref())) {
        let differing = a ^ b;
        if differing != 0 {
            return index * 8
                + u16::try_from(differing.leading_zeros())
                    .expect("an eight-bit prefix length fits u16");
        }
    }
    256
}

fn key_bit(key: &Hash, bit: u16) -> bool {
    key.as_ref()[usize::from(bit / 8)] & (0x80 >> (bit % 8)) != 0
}

/// Raw prefix bytes must not receive `Hash`'s mandatory low-bit marker.
fn prefix(key: &Hash, bit: u16) -> [u8; Hash::LENGTH] {
    let mut prefix = *key.as_ref();
    let byte = usize::from(bit / 8);
    prefix[byte] = if bit.is_multiple_of(8) {
        0
    } else {
        prefix[byte] & (0xff << (8 - bit % 8))
    };
    prefix[byte + 1..].fill(0);
    prefix
}

fn leaf(key: Hash, value: Hash) -> Arc<Node> {
    Arc::new(Node {
        hash: Hash::new_from_chunks(&[LEAF, key.as_ref(), value.as_ref()]),
        first: key,
        kind: NodeKind::Leaf(value),
    })
}

fn branch(bit: u16, left: Arc<Node>, right: Arc<Node>) -> Arc<Node> {
    Arc::new(Node {
        hash: Hash::new_from_chunks(&[
            BRANCH,
            &bit.to_le_bytes(),
            &prefix(&left.first, bit),
            left.hash.as_ref(),
            right.hash.as_ref(),
        ]),
        first: left.first,
        kind: NodeKind::Branch { bit, left, right },
    })
}

fn replace_node(node: Option<&Arc<Node>>, key: Hash, after: Option<Hash>) -> Option<Arc<Node>> {
    let Some(node) = node else {
        return after.map(|value| leaf(key, value));
    };
    let shared = common_bits(&key, &node.first);
    let depth = match &node.kind {
        NodeKind::Leaf(_) => 256,
        NodeKind::Branch { bit, .. } => *bit,
    };
    if shared < depth {
        // The validated operation is an insertion outside this subtree's prefix.
        return after.map(|value| {
            let new = leaf(key, value);
            if key_bit(&key, shared) {
                branch(shared, Arc::clone(node), new)
            } else {
                branch(shared, new, Arc::clone(node))
            }
        });
    }
    match &node.kind {
        NodeKind::Leaf(_) => after.map(|value| leaf(key, value)),
        NodeKind::Branch { bit, left, right } => {
            // A removal collapses its unary branch; no tombstone/history remains.
            Some(if key_bit(&key, *bit) {
                replace_node(Some(right), key, after).map_or_else(
                    || Arc::clone(left),
                    |new| branch(*bit, Arc::clone(left), new),
                )
            } else {
                replace_node(Some(left), key, after).map_or_else(
                    || Arc::clone(right),
                    |new| branch(*bit, new, Arc::clone(right)),
                )
            })
        }
    }
}

#[cfg(test)]
#[path = "merkle_map_tests.rs"]
mod tests;
