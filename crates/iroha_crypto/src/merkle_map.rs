//! Persistent canonical commitments to maps of public key/value hashes.
//!
//! A binary radix tree compresses every unary path. Each branch commits its
//! split bit, the shared key prefix, and its ordered children. Its shape depends
//! only on the current keys, never insertion order. This is a separate commitment
//! format from the indexed `MerkleTree` and the full-depth sparse Merkle tree.
//! Fixed-size node descriptors support authenticated external lookup and updates;
//! bounded complete-range witnesses cover every matching key. The map defines
//! no wire encoding or durable-storage owner.
//!
//! Clones share immutable nodes. A mutation copies at most one 256-bit search
//! path; it never scans unrelated entries. Each new node is admitted against
//! the retained original pool before allocation. Refusal preserves the old version.
//! Keys and tree shape are public: lookup is intentionally not constant-time.

use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedShared, PrepaidSharedError};

use crate::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[path = "merkle_map/external.rs"]
mod external;
#[path = "merkle_map/resident.rs"]
mod resident;
pub use external::{
    MerkleMapEdit, MerkleMapNode, MerkleMapNodeRef, MerkleMapNodeStore, MerkleMapReadError,
    MerkleMapRoot, MerkleMapStoreNode, MerkleMapUpdateError, MerkleMapUpdateWorkspace,
    MerkleMapValueRef,
};
#[path = "merkle_map/proof.rs"]
mod proof;
pub use proof::MerkleMapLookupProof;
#[path = "merkle_map/ordered_range.rs"]
mod ordered_range;
pub use ordered_range::{
    MAX_NORITO_DOMAIN_BYTES, MAX_NORITO_KEY_BYTES, MAX_NORITO_RANGE_PROOF_BYTES,
    MAX_NORITO_RANGE_ROWS, MAX_NORITO_TREE_ENTRIES, MAX_NORITO_TREE_PAYLOAD_BYTES,
    MAX_NORITO_VALUE_BYTES, NoritoKeyDigestRangeProofV1, NoritoKeyDigestRangeTreeV1,
    NoritoKeyRangeError, NoritoKeyRangeExternalProofV1, NoritoKeyRangeExternalV1,
    NoritoKeyRangeNodeStoreV1, NoritoKeyRangeProofV1, NoritoKeyRangeTreeV1,
    NoritoKeyRangeVerifyRequestV1, VerifiedNoritoKeyDigestRangeV1,
    VerifiedNoritoKeyRangeExternalV1, VerifiedNoritoKeyRangeV1, digest_norito_value_frame_v1,
};
#[path = "merkle_map/range_proof.rs"]
mod range_proof;
pub use range_proof::{MerkleMapRangeError, MerkleMapRangeProof};

const EMPTY: &[u8] = b"iroha:merkle-map:empty:v1\0";
const LEAF: &[u8] = b"iroha:merkle-map:leaf:v1\0";
const BRANCH: &[u8] = b"iroha:merkle-map:branch:v1\0";
const ROOT: &[u8] = b"iroha:merkle-map:root:v1\0";

/// An immutable-node, history-independent map commitment with cheap snapshots.
///
/// The owner supplies canonical, domain-separated key and value hashes. Updates
/// compare the expected old value before changing anything; snapshots remain
/// valid after updates to their descendants. This is not persistence to disk.
#[derive(Clone)]
pub struct MerkleMap {
    node: Option<ChargedShared<Node>>,
    len: u64,
    budget: AllocationBudget,
}

/// One sibling on an exact-key membership path through the compressed tree.
///
/// Steps are ordered from root to leaf. `prefix` is the raw, masked key prefix
/// at `bit`; it is not a `Hash` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_crypto::merkle_map::MerkleMapProofStep")]
pub struct MerkleMapProofStep {
    /// MSB-first split bit in `0..256`.
    pub bit: u16,
    /// Shared key prefix with every bit at or after `bit` cleared.
    pub prefix: [u8; Hash::LENGTH],
    /// Hash of the other child at this split.
    pub sibling: Hash,
}

/// Inclusion of one exact key/value hash in a canonical accumulated map root.
///
/// This proves membership against a caller-supplied root. It does not establish
/// that the root was committed by consensus or that the raw value matches the
/// owner-specific value-hash convention.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_crypto::merkle_map::MerkleMapProof")]
pub struct MerkleMapProof {
    /// Exact hashed key.
    pub key: Hash,
    /// Exact hashed value.
    pub value: Hash,
    /// Number of leaves in the authenticated map version.
    pub len: u64,
    /// Canonical compressed path from root to leaf.
    pub steps: Vec<MerkleMapProofStep>,
}

impl MerkleMapProof {
    /// Verify this exact membership against an independently authenticated map root.
    #[must_use]
    pub fn verify(&self, expected_root: Hash) -> bool {
        if self.len == 0
            || self.steps.len() > Hash::LENGTH * 8
            || (self.len == 1 && !self.steps.is_empty())
            || (self.len > 1 && self.steps.is_empty())
        {
            return false;
        }
        let mut previous_bit = None;
        for step in &self.steps {
            if step.bit >= 256
                || previous_bit.is_some_and(|previous| step.bit <= previous)
                || step.prefix != prefix(&self.key, step.bit)
            {
                return false;
            }
            previous_bit = Some(step.bit);
        }
        let mut current = leaf_hash(self.key, self.value);
        for step in self.steps.iter().rev() {
            current = if key_bit(&self.key, step.bit) {
                branch_hash(step.bit, &step.prefix, step.sibling, current)
            } else {
                branch_hash(step.bit, &step.prefix, current, step.sibling)
            };
        }
        root_hash(self.len, Some(current)) == expected_root
    }
}

/// A rejected update leaves the entire map and its root unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
    /// The original local pool refused the complete replacement path before allocation.
    #[error("Merkle map node admission failed: {0}")]
    Admission(#[source] AllocationRefusal),
    /// An admitted physical allocation or exact prepaid partition failed locally.
    #[error("Merkle map node allocation failed: {0}")]
    Allocation(#[source] PrepaidSharedError),
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
        left: ChargedShared<Node>,
        right: ChargedShared<Node>,
    },
}

impl MerkleMap {
    /// Construct an empty commitment retaining its caller's original local pool.
    /// The empty map allocates no nodes; every later mutation uses this same pool.
    pub fn new(budget: &AllocationBudget) -> Self {
        Self {
            node: None,
            len: 0,
            budget: budget.clone(),
        }
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
        root_hash(self.len, self.node.as_ref().map(|node| node.hash))
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

    /// Construct the exact-key membership path for this immutable map version.
    ///
    /// An absent key returns `None`; callers must never interpret that result as
    /// a non-membership proof.
    #[must_use]
    pub fn proof(&self, key: &Hash) -> Option<MerkleMapProof> {
        let mut node = self.node.as_deref()?;
        let mut steps = Vec::new();
        loop {
            match &node.kind {
                NodeKind::Leaf(value) => {
                    return (node.first == *key).then_some(MerkleMapProof {
                        key: *key,
                        value: *value,
                        len: self.len,
                        steps,
                    });
                }
                NodeKind::Branch { bit, left, right } => {
                    if common_bits(key, &node.first) < *bit {
                        return None;
                    }
                    let is_right = key_bit(key, *bit);
                    steps.push(MerkleMapProofStep {
                        bit: *bit,
                        prefix: prefix(key, *bit),
                        sibling: if is_right { left.hash } else { right.hash },
                    });
                    node = if is_right { right } else { left };
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
    /// Rejects a mismatched expected value, unrepresentable count, or local
    /// allocation refusal. Local capacity never changes the committed result.
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
        let node = resident::prepare(self.node.as_ref(), key, after, &self.budget)?;
        let previous = std::mem::replace(&mut self.node, node);
        self.len = len;
        // A release callback must see the complete new version, never a new root
        // with the previous count. Older snapshots keep their own node charges.
        drop(previous);
        Ok(true)
    }
}

/// MSB-first shared prefix length, including all 256 bits for equal keys.
fn common_bits(a: &Hash, b: &Hash) -> u16 {
    common_prefix(a.as_ref(), b.as_ref())
}

fn common_prefix(a: &[u8; Hash::LENGTH], b: &[u8; Hash::LENGTH]) -> u16 {
    for (index, (&a, &b)) in (0_u16..).zip(a.iter().zip(b)) {
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

fn root_hash(len: u64, node: Option<Hash>) -> Hash {
    let node = node.unwrap_or_else(|| Hash::new(EMPTY));
    Hash::new_from_chunks(&[ROOT, &len.to_le_bytes(), node.as_ref()])
}

fn leaf_hash(key: Hash, value: Hash) -> Hash {
    Hash::new_from_chunks(&[LEAF, key.as_ref(), value.as_ref()])
}

fn branch_hash(bit: u16, prefix: &[u8; Hash::LENGTH], left: Hash, right: Hash) -> Hash {
    Hash::new_from_chunks(&[
        BRANCH,
        &bit.to_le_bytes(),
        prefix,
        left.as_ref(),
        right.as_ref(),
    ])
}

#[cfg(test)]
#[path = "merkle_map_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "merkle_map/resident_tests.rs"]
mod resident_tests;
