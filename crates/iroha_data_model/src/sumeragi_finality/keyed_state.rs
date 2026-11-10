//! Ordered, history-independent State commitments and exact-key witnesses.
//!
//! The State owner supplies the complete registry schema digest and authoritative
//! canonical row bytes. This primitive has no means of establishing finality.
//! The certified execution and its selected commitment definition must authenticate
//! the expected root independently.
//!
//! A compressed binary radix tree orders leaves by table and bare Norito key.
//! Each branch binds the split bit, ordered subtree digests and subtree counts.
//! Counts bind ranks, enabling complete interval checks under the same root.
//! A point absence witness exposes digests of one terminal leaf without disclosing
//! its key or value bytes. The root binds the schema, cardinality and tree digest.

//!
//! Each digest uses the 32 raw output bytes of BLAKE3 over the common domain,
//! one tag byte and the ordered inputs below. Integers are little endian.
//!
//! | Tag | Inputs |
//! | --- | --- |
//! | 1 | table length (u64), table bytes, bare key |
//! | 2 | key digest, value length (u64), bare value |
//! | 3 | key digest, value digest |
//! | 4 | split bit (u32), left count (u64), left digest, right count (u64), right digest |
//! | 5 | schema digest, row count (u64), top digest (all zeros for an empty tree) |
//!
//! The search key is the ASCII table, a zero delimiter, and the bare key. A
//! byte contributes a one continuation bit followed by its eight big-endian
//! bits. End-of-key contributes zero, with subsequent bits zero. A branch
//! splits at the first differing bit among its keys. The bit encoding preserves
//! lexicographic byte order even when one key is a prefix of another.
//!
//! Proofs expose path shape, counts and deterministic digests. A reader who
//! guesses an adjacent key and low-entropy value may test that guess. Complete
//! range proofs also disclose the two outside boundary keys.

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, ChargedBuffer, ChargedBufferError,
    shared::Shared,
};
use iroha_schema::IntoSchema;
use norito::{
    Decode, Encode,
    derive::{JsonDeserialize, JsonSerialize},
};

mod build;
pub use build::KeyedStateBuilderV1;
mod range;
pub use range::StateRangeProofV1;

type Digest = [u8; 32];
const DOMAIN: &[u8] = b"iroha:keyed-state:radix:blake3:v1\0";

/// Canonical table identity and its exact bare Norito key (empty for a cell).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateKeyV1")]
pub struct StateKeyV1 {
    /// Registered ASCII identity of the canonical table or cell.
    pub table: String,
    /// Complete key bytes as encoded under the independently qualified schema.
    pub key: Vec<u8>,
}

impl StateKeyV1 {
    /// Whether the identifier uses the canonical registry ASCII grammar.
    pub fn valid_table_id(id: &str) -> bool {
        !id.is_empty()
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
    }

    fn validate(&self) -> Result<(), KeyedStateError> {
        if !Self::valid_table_id(&self.table) {
            return Err(KeyedStateError::InvalidKey);
        }
        let bytes = self
            .table
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_add(self.key.len()))
            .ok_or(KeyedStateError::InvalidKey)?;
        if bytes > u32::MAX as usize / 9 {
            return Err(KeyedStateError::InvalidKey);
        }
        Ok(())
    }

    fn bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.table
            .bytes()
            .chain(std::iter::once(0))
            .chain(self.key.iter().copied())
    }

    fn byte(&self, i: usize) -> Option<u8> {
        if i < self.table.len() {
            self.table.as_bytes().get(i).copied()
        } else if i == self.table.len() {
            Some(0)
        } else {
            self.key.get(i - self.table.len() - 1).copied()
        }
    }

    fn right(&self, bit: u32) -> bool {
        let offset = bit % 9;
        self.byte((bit / 9) as usize)
            .is_some_and(|b| offset == 0 || (b >> (8 - offset)) & 1 != 0)
    }

    fn first_diff(&self, other: &[u8]) -> Option<u32> {
        for (i, (a, b)) in self.bytes().zip(other.iter().copied()).enumerate() {
            if a != b {
                return Some(i as u32 * 9 + 1 + (a ^ b).leading_zeros());
            }
        }
        let len = self.table.len() + 1 + self.key.len();
        (len != other.len()).then_some(len.min(other.len()) as u32 * 9)
    }

    fn digest(&self) -> Digest {
        hash(
            1,
            &[
                &(self.table.len() as u64).to_le_bytes(),
                self.table.as_bytes(),
                &self.key,
            ],
        )
    }
}

fn hash(tag: u8, parts: &[&[u8]]) -> Digest {
    let mut h = blake3::Hasher::new();
    h.update(DOMAIN).update(&[tag]);
    for part in parts {
        h.update(part);
    }
    *h.finalize().as_bytes()
}

fn value_digest(key: &Digest, value: &[u8]) -> Digest {
    hash(2, &[key, &(value.len() as u64).to_le_bytes(), value])
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateRadixSummaryV1")]
struct Summary {
    count: u64,
    digest: Digest,
}
const EMPTY: Summary = Summary {
    count: 0,
    digest: [0; 32],
};

impl Summary {
    fn branch(bit: u32, left: Self, right: Self) -> Option<Self> {
        let count = left.count.checked_add(right.count)?;
        let digest = hash(
            4,
            &[
                &bit.to_le_bytes(),
                &left.count.to_le_bytes(),
                &left.digest,
                &right.count.to_le_bytes(),
                &right.digest,
            ],
        );
        Some(Self { count, digest })
    }
    fn root(self, schema: &Digest) -> Digest {
        hash(5, &[schema, &self.count.to_le_bytes(), &self.digest])
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateRadixLeafV1")]
struct Opening {
    key: Digest,
    value: Digest,
}
impl Opening {
    fn summary(self) -> Summary {
        Summary {
            count: 1,
            digest: hash(3, &[&self.key, &self.value]),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateRadixStepV1")]
struct Step {
    bit: u32,
    sibling: Summary,
}

/// Compressed path for a single raw State key. The expected value and certified
/// root are supplied by the verifier, never selected by this witness.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::StateLookupProofV1")]
pub struct StateLookupProofV1 {
    #[norito(required)]
    terminal: Option<Opening>,
    steps: Vec<Step>,
}

impl StateLookupProofV1 {
    fn rank(&self, schema: &Digest, root: &Digest, key: &StateKeyV1) -> Option<(u64, u64)> {
        key.validate().ok()?;
        let mut summary = self.terminal.map_or(EMPTY, Opening::summary);
        let mut rank = 0u64;
        let mut last = None;
        for step in self.steps.iter().rev() {
            if summary.count == 0
                || step.sibling.count == 0
                || last.is_some_and(|bit| step.bit >= bit)
            {
                return None;
            }
            last = Some(step.bit);
            summary = if key.right(step.bit) {
                rank = rank.checked_add(step.sibling.count)?;
                Summary::branch(step.bit, step.sibling, summary)?
            } else {
                Summary::branch(step.bit, summary, step.sibling)?
            };
        }
        (summary.root(schema) == *root).then_some((rank, summary.count))
    }

    /// Authenticate presence of the exact bytes or absence of this key under an
    /// independently selected schema and root. An empty present value is distinct
    /// from a missing row.
    #[must_use]
    pub fn verify(
        &self,
        schema: &Digest,
        root: &Digest,
        key: &StateKeyV1,
        expected: Option<&[u8]>,
    ) -> bool {
        if self.rank(schema, root, key).is_none() {
            return false;
        }
        let searched = key.digest();
        match (self.terminal, expected) {
            (Some(open), Some(value)) => {
                open.key == searched && open.value == value_digest(&searched, value)
            }
            (Some(open), None) => open.key != searched,
            (None, value) => value.is_none(),
        }
    }
}

/// Construction or local funding refusal. Refusal leaves the previous version intact.
#[derive(Debug, thiserror::Error)]
pub enum KeyedStateError {
    /// Unknown identity grammar or length outside the bit-index representation.
    #[error("invalid canonical State key")]
    InvalidKey,
    /// The cold stream repeated or reordered a canonical identity.
    #[error("State rows must be strictly ordered")]
    Order,
    /// Entry count cannot be represented.
    #[error("State radix count overflow")]
    Count,
    /// Invalid half-open range of keys.
    #[error("invalid State key range")]
    Range,
    /// The specified proof row or contributor bound was exceeded.
    #[error("State witness exceeds its local capacity")]
    Capacity,
    /// Preserve original persistent-node memory pool refusal.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// Original row bytes could not be retained.
    #[error(transparent)]
    Buffer(#[from] ChargedBufferError),
    /// Physical allocator refused the locally admitted node.
    #[error(transparent)]
    Allocator(#[from] iroha_allocation::shared::ReservationError),
}

type Owner = Shared<Node, AllocationCharge>;

struct Leaf {
    bytes: ChargedBuffer<u8>,
    key_end: usize,
    opening: Opening,
}

impl Leaf {
    fn encoded_key(&self) -> &[u8] {
        &self.bytes.as_slice()[..self.key_end]
    }
    fn value(&self) -> &[u8] {
        &self.bytes.as_slice()[self.key_end..]
    }
}

enum Kind {
    Leaf(Leaf),
    Branch { bit: u32, children: [Owner; 2] },
    Released,
}

struct Node {
    summary: Summary,
    kind: Kind,
}

// A pathologically long unary-prefix family must not recursively destroy the stack.
// Peel children while the original charged owners remain live; shared branches are
// left alone when another State version still retains them.
impl Drop for Node {
    fn drop(&mut self) {
        let Kind::Branch { children, .. } = std::mem::replace(&mut self.kind, Kind::Released)
        else {
            return;
        };
        // Descend into the smaller subtree first. Each pending larger sibling
        // halves the current leaf bound, so 64 slots suffice for u64 counts.
        // Reclamation cannot fail or acquire memory while a replica is dropping
        // a failed update under an exhausted State budget.
        fn split([left, right]: [Owner; 2]) -> (Owner, Owner) {
            if left.summary.count <= right.summary.count {
                (left, right)
            } else {
                (right, left)
            }
        }
        let mut pending: [Option<Owner>; 64] = std::array::from_fn(|_| None);
        let (mut next, right) = split(children);
        pending[0] = Some(right);
        let mut used = 1;
        loop {
            if let Some(mut node) = next.into_inner()
                && let Kind::Branch { children, .. } =
                    std::mem::replace(&mut node.kind, Kind::Released)
            {
                let (small, large) = split(children);
                pending[used] = Some(large);
                used += 1;
                next = small;
                continue;
            }
            if used == 0 {
                break;
            }
            used -= 1;
            next = pending[used].take().expect("pending owned sibling");
        }
    }
}

impl Node {
    fn new(
        kind: Kind,
        summary: Summary,
        budget: &AllocationBudget,
    ) -> Result<Owner, KeyedStateError> {
        let layout = Owner::layout();
        let charge = budget
            .try_reserve(layout)?
            .try_split(layout)
            .expect("exact charge");
        Owner::try_new(Node { summary, kind }, charge).map_err(|(_, _, error)| error.into())
    }
    fn branch(
        bit: u32,
        left: Owner,
        right: Owner,
        budget: &AllocationBudget,
    ) -> Result<Owner, KeyedStateError> {
        let sum =
            Summary::branch(bit, left.summary, right.summary).ok_or(KeyedStateError::Count)?;
        Self::new(
            Kind::Branch {
                bit,
                children: [left, right],
            },
            sum,
            budget,
        )
    }
    fn leaf(
        key: &StateKeyV1,
        value: &[u8],
        budget: &AllocationBudget,
    ) -> Result<Owner, KeyedStateError> {
        let key_end = key.table.len() + 1 + key.key.len();
        let size = key_end
            .checked_add(value.len())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let mut bytes = ChargedBuffer::new(size, budget)?;
        for b in key.bytes().chain(value.iter().copied()) {
            bytes
                .try_push(b)
                .expect("exact canonical key and value layout");
        }
        let digest = key.digest();
        let opening = Opening {
            key: digest,
            value: value_digest(&digest, value),
        };
        Self::new(
            Kind::Leaf(Leaf {
                bytes,
                key_end,
                opening,
            }),
            opening.summary(),
            budget,
        )
    }
}

/// A persistent ordered map of authoritative State entries. Cloning shares its
/// immutable root and original allocation budget. Each mutation copies only
/// ancestors of the changed key, and a refusal preserves the entire prior map.
#[derive(Clone)]
pub struct KeyedStateTreeV1 {
    schema: Digest,
    top: Option<Owner>,
    budget: AllocationBudget,
}

impl KeyedStateTreeV1 {
    /// Empty map under the qualified registry schema.
    pub fn new(schema: Digest, budget: &AllocationBudget) -> Self {
        Self {
            schema,
            top: None,
            budget: budget.clone(),
        }
    }

    /// Digest of the registry schema and commitment profile of this version.
    pub fn schema_digest(&self) -> Digest {
        self.schema
    }

    /// Schema-bound 32-byte digest. Its authority comes from the State publication owner.
    pub fn root(&self) -> Digest {
        self.top
            .as_ref()
            .map_or(EMPTY, |node| node.summary)
            .root(&self.schema)
    }

    /// Number of committed rows.
    pub fn len(&self) -> u64 {
        self.top.as_ref().map_or(0, |n| n.summary.count)
    }

    /// Whether this version has zero rows.
    pub fn is_empty(&self) -> bool {
        self.top.is_none()
    }

    fn seek<'a>(
        &'a self,
        key: &StateKeyV1,
        mut step: impl FnMut(u32, bool, &'a Owner),
    ) -> Option<&'a Leaf> {
        let mut node = self.top.as_ref()?;
        loop {
            match &node.kind {
                Kind::Leaf(leaf) => return Some(leaf),
                Kind::Branch { bit, children } => {
                    let right = key.right(*bit);
                    step(*bit, right, &children[usize::from(!right)]);
                    node = &children[usize::from(right)];
                }
                Kind::Released => unreachable!("released node retained"),
            }
        }
    }

    /// Read the exact present canonical bytes.
    pub fn get(&self, key: &StateKeyV1) -> Option<&[u8]> {
        key.validate().ok()?;
        let leaf = self.seek(key, |_, _, _| {})?;
        (key.first_diff(leaf.encoded_key()).is_none()).then_some(leaf.value())
    }

    fn proof_steps(&self, key: &StateKeyV1) -> Result<usize, KeyedStateError> {
        key.validate()?;
        let mut count = 0;
        self.seek(key, |_, _, _| count += 1);
        Ok(count)
    }

    /// Exact heap backing required by a point proof at this immutable version.
    /// The returned count excludes the inline proof object and its outer wire envelope.
    ///
    /// # Errors
    /// Invalid key or allocation layout overflow.
    pub fn proof_heap_bytes(&self, key: &StateKeyV1) -> Result<usize, KeyedStateError> {
        std::alloc::Layout::array::<Step>(self.proof_steps(key)?)
            .map(|l| l.size())
            .map_err(|_| KeyedStateError::Capacity)
    }

    /// Produce a point witness, including for a key absent in this version.
    /// Exactly the preflighted `proof_heap_bytes` are requested by the step vector.
    ///
    /// # Errors
    /// Invalid key or witness allocation refusal.
    pub fn prove(&self, key: &StateKeyV1) -> Result<StateLookupProofV1, KeyedStateError> {
        let count = self.proof_steps(key)?;
        let mut steps = Vec::new();
        steps
            .try_reserve_exact(count)
            .map_err(|_| KeyedStateError::Capacity)?;
        let terminal = self
            .seek(key, |bit, _, sib| {
                steps.push(Step {
                    bit,
                    sibling: sib.summary,
                })
            })
            .map(|leaf| leaf.opening);
        Ok(StateLookupProofV1 { terminal, steps })
    }

    /// Replace, insert or remove a canonical value. Applying a repeated change
    /// is a no-op. Clone this version and install it only after all changes of
    /// an atomic batch have succeeded.
    ///
    /// # Errors
    /// Invalid key, count overflow or unavailable original allocation credit.
    pub fn replace(
        &mut self,
        key: &StateKeyV1,
        value: Option<&[u8]>,
    ) -> Result<(), KeyedStateError> {
        key.validate()?;
        let Some(top) = self.top.as_ref() else {
            if let Some(value) = value {
                self.top = Some(Node::leaf(key, value, &self.budget)?);
            }
            return Ok(());
        };
        // Scattered keys have about log2(rows) branches. Retain common paths entirely
        // on the stack; a longer adversarial key is re-read into exactly funded storage.
        let mut inline = [(0u32, false, top); 32];
        let mut used = 0;
        let found = self
            .seek(key, |bit, right, sibling| {
                if let Some(slot) = inline.get_mut(used) {
                    *slot = (bit, right, sibling);
                }
                used += 1;
            })
            .expect("nonempty tree has a terminal leaf");
        let diff = key.first_diff(found.encoded_key());
        if diff.is_some() && value.is_none() || diff.is_none() && value == Some(found.value()) {
            return Ok(());
        }
        let mut backing;
        let mut path = if used <= inline.len() {
            &inline[..used]
        } else {
            backing = ChargedBuffer::new(used, &self.budget)?;
            self.seek(key, |bit, right, sibling| {
                assert!(
                    backing.try_push((bit, right, sibling)).is_ok(),
                    "path fits its original preflight"
                );
            });
            backing.as_slice()
        };
        let mut next = match (diff, value) {
            (Some(split), Some(value)) => {
                let leaf = Node::leaf(key, value, &self.budget)?;
                let index = path.partition_point(|(bit, _, _)| *bit < split);
                // Descend the original unchanged upper path to the subtree at the insertion point.
                let mut subtree = self.top.as_ref().expect("nonempty");
                for (_, right, _) in &path[..index] {
                    let Kind::Branch { children, .. } = &subtree.kind else {
                        unreachable!("path ends before terminal");
                    };
                    subtree = &children[usize::from(*right)];
                }
                path = &path[..index];
                Some(if key.right(split) {
                    Node::branch(split, subtree.clone(), leaf, &self.budget)?
                } else {
                    Node::branch(split, leaf, subtree.clone(), &self.budget)?
                })
            }
            (None, Some(value)) => Some(Node::leaf(key, value, &self.budget)?),
            (None, None) => path.split_last().map(|((_, _, sibling), parents)| {
                path = parents;
                (*sibling).clone()
            }),
            _ => unreachable!("absent removal returned"),
        };
        for &(bit, right, sibling) in path.iter().rev() {
            let child = next.expect("nonempty path has replacement child");
            next = Some(if right {
                Node::branch(bit, sibling.clone(), child, &self.budget)?
            } else {
                Node::branch(bit, child, sibling.clone(), &self.budget)?
            });
        }
        self.top = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
