//! Selective native lifecycle runtime evidence under the existing ordinary-write SMT.
//!
//! These JSON transport types are not part of the covered block wire graph. The
//! selected service owns one fixed raw key/value write; a certified root proves
//! that exact native anchor without publishing any unrelated World entries.
//! Missing SMT children are the existing constant `H("")` at every level.

use crate::{
    NetworkId, block::consensus::ExecWitness,
    execution_witness::TRIGGER_LIFECYCLE_SELECTION_WITNESS_KEY_V1 as KEY,
    runtime::definition::SelectionId,
};
use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedBuffer};
use iroha_crypto::Hash;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Fixed native value size: network, height, execution selection and final selection.
pub const LIFECYCLE_RUNTIME_ANCHOR_BYTES: usize = 104;

/// Exact service-owned raw value; no mutable World state or decoder chooses its layout.
#[must_use]
pub fn lifecycle_runtime_anchor(
    network: NetworkId,
    height: u64,
    execution_selection: SelectionId,
    post_selection: SelectionId,
) -> [u8; LIFECYCLE_RUNTIME_ANCHOR_BYTES] {
    let mut bytes = [0; LIFECYCLE_RUNTIME_ANCHOR_BYTES];
    bytes[..32].copy_from_slice(network.as_bytes());
    bytes[32..40].copy_from_slice(&height.to_le_bytes());
    bytes[40..72].copy_from_slice(&execution_selection.0);
    bytes[72..].copy_from_slice(&post_selection.0);
    bytes
}

/// One nonempty sibling, counted upward from the leaf at level zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct LifecycleRuntimeProofStepV1 {
    /// Leaf-up level in the existing fixed-depth 256-bit SMT.
    pub level: u8,
    /// Exact nonempty sibling hash; explicit empty siblings are noncanonical.
    pub hash: Hash,
}

/// Untrusted bounded JSON transport; only a matching certified root grants authority.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct LifecycleRuntimeProofV1 {
    /// Certified block height bound by the native anchor value.
    pub height: u64,
    /// Selection frozen before execution begins.
    pub execution_selection: SelectionId,
    /// Actual final selection after all roots; restrictions may have changed.
    pub post_selection: SelectionId,
    /// Strictly increasing nonempty sibling levels, at most 256 entries.
    pub siblings: Vec<LifecycleRuntimeProofStepV1>,
}

/// Original physical branch backing retained until its JSON projection is released.
pub struct AllocatedLifecycleRuntimeProofV1 {
    proof: LifecycleRuntimeProofV1,
    _charge: AllocationCharge,
}
impl AllocatedLifecycleRuntimeProofV1 {
    /// Borrow the proof while its exact allocation remains retained.
    #[must_use]
    pub fn proof(&self) -> &LifecycleRuntimeProofV1 {
        &self.proof
    }
}

#[derive(Clone, Copy)]
struct Node {
    path: [u8; 32],
    hash: Hash,
    source: usize,
}

/// Exact source, scratch or allocation refusal while building the original runtime branch.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleRuntimeProofBuildErrorV1 {
    /// The original witness cannot establish one canonical runtime branch.
    #[error("{0}")]
    Invalid(&'static str),
    /// A finite backing allocation was not admitted by its original pool.
    #[error(transparent)]
    Buffer(#[from] iroha_allocation::ChargedBufferError),
    /// The original pool refused the exact retained sibling array.
    #[error(transparent)]
    Admission(#[from] iroha_allocation::AllocationRefusal),
    /// The admitted array layout could not be split from its original reservation.
    #[error(transparent)]
    Reservation(#[from] iroha_allocation::InsufficientReservation),
    /// A concrete backing demand could not be represented.
    #[error(transparent)]
    Layout(#[from] std::alloc::LayoutError),
    /// The global allocator refused the already admitted sibling array.
    #[error(transparent)]
    Allocator(#[from] std::collections::TryReserveError),
}
impl From<&'static str> for LifecycleRuntimeProofBuildErrorV1 {
    fn from(value: &'static str) -> Self {
        Self::Invalid(value)
    }
}
impl LifecycleRuntimeProofBuildErrorV1 {
    /// Whether the same source may succeed with its original local resource owner available.
    pub fn is_local_refusal(&self) -> bool {
        matches!(
            self,
            Self::Buffer(
                iroha_allocation::ChargedBufferError::Admission(
                    iroha_allocation::AllocationRefusal::Capacity { .. }
                ) | iroha_allocation::ChargedBufferError::Allocator { .. }
            ) | Self::Admission(iroha_allocation::AllocationRefusal::Capacity { .. })
                | Self::Allocator(_)
        )
    }
}

impl LifecycleRuntimeProofV1 {
    /// Build one branch from the complete original block witness, without World enumeration.
    ///
    /// Scratch and retained branch backing are charged before allocation. Repeated ordinary
    /// keys use their last value; the native fixed key must occur exactly once.
    /// # Errors
    /// Invalid carrier/value, duplicate native key, key-hash collision, or resource refusal.
    pub fn from_witness(
        witness: &ExecWitness,
        network: NetworkId,
        height: u64,
        budget: &AllocationBudget,
    ) -> Result<AllocatedLifecycleRuntimeProofV1, String> {
        Self::from_witness_owned(witness, network, height, budget)
            .map_err(|error| error.to_string())
    }

    /// Build the identical branch while retaining typed original resource refusals.
    /// # Errors
    /// Preserves malformed source, exact pool, reserved-layout and allocator failures.
    pub fn from_witness_owned(
        witness: &ExecWitness,
        network: NetworkId,
        height: u64,
        budget: &AllocationBudget,
    ) -> Result<AllocatedLifecycleRuntimeProofV1, LifecycleRuntimeProofBuildErrorV1> {
        let mut targets = witness.writes.iter().filter(|write| write.key == KEY);
        let target = targets
            .next()
            .ok_or("missing native lifecycle runtime anchor")?;
        if targets.next().is_some()
            || target.value.len() != LIFECYCLE_RUNTIME_ANCHOR_BYTES
            || target.value[..32] != *network.as_bytes()
            || target.value[32..40] != height.to_le_bytes()
            || height < 2
        {
            return Err("invalid native lifecycle runtime anchor".into());
        }
        let execution_selection = SelectionId(
            target.value[40..72]
                .try_into()
                .map_err(|_| "invalid frozen selection bytes")?,
        );
        let post_selection = SelectionId(
            target.value[72..104]
                .try_into()
                .map_err(|_| "invalid final selection bytes")?,
        );
        let capacity = witness
            .writes
            .len()
            .checked_mul(2)
            .ok_or("runtime proof scratch overflow")?;
        let mut storage = ChargedBuffer::new(capacity, budget)?;
        let empty = Hash::new([]);
        for _ in 0..capacity {
            storage.push_reserved(Node {
                path: [0; 32],
                hash: empty,
                source: 0,
            });
        }
        let layout = std::alloc::Layout::array::<LifecycleRuntimeProofStepV1>(256)?;
        let mut reservation = budget.try_reserve_layouts([layout])?;
        let charge = reservation.try_split(layout)?;
        let mut siblings = Vec::new();
        siblings.try_reserve_exact(256)?;
        let (mut current, mut next) = storage.as_mut_slice().split_at_mut(witness.writes.len());
        for (source, write) in witness.writes.iter().enumerate() {
            let path = Hash::new(&write.key);
            let value = Hash::new(&write.value);
            current[source] = Node {
                path: path.into(),
                hash: leaf(path, value),
                source,
            };
        }
        current.sort_unstable_by_key(|node| (node.path, node.source));
        let mut length = 0;
        for source in 0..current.len() {
            let node = current[source];
            if length > 0 && current[length - 1].path == node.path {
                if witness.writes[current[length - 1].source].key != witness.writes[node.source].key
                {
                    return Err("ordinary-write key-path collision".into());
                }
                current[length - 1] = node;
            } else {
                current[length] = node;
                length += 1;
            }
        }
        let mut target_path: [u8; 32] = Hash::new(KEY).into();
        for level in 0..256 {
            let bit = 255 - level;
            let byte = bit / 8;
            let mask = 1 << (bit % 8);
            let mut sibling = target_path;
            sibling[byte] ^= mask;
            let other = lookup(&current[..length], &sibling).map_or(empty, |node| node.hash);
            if other != empty {
                siblings.push(LifecycleRuntimeProofStepV1 {
                    level: level as u8,
                    hash: other,
                });
            }
            let mut out = 0;
            for node in &current[..length] {
                let mut sibling = node.path;
                sibling[byte] ^= mask;
                let other = lookup(&current[..length], &sibling);
                let is_right = node.path[byte] & mask != 0;
                if is_right && other.is_some() {
                    continue;
                }
                let other = other.map_or(empty, |node| node.hash);
                let hash = if is_right {
                    branch(other, node.hash)
                } else {
                    branch(node.hash, other)
                };
                let mut parent = node.path;
                parent[byte] &= !mask;
                next[out] = Node {
                    path: parent,
                    hash,
                    source: 0,
                };
                out += 1;
            }
            next[..out].sort_unstable_by_key(|node| node.path);
            target_path[byte] &= !mask;
            std::mem::swap(&mut current, &mut next);
            length = out;
        }
        let proof = Self {
            height,
            execution_selection,
            post_selection,
            siblings,
        };
        if length != 1 || proof.computed_root(network) != Some(current[0].hash) {
            return Err("native lifecycle proof differs from its original write root".into());
        }
        Ok(AllocatedLifecycleRuntimeProofV1 {
            proof,
            _charge: charge,
        })
    }

    /// Compute the untrusted branch root using only its bounded path and fixed value.
    #[must_use]
    pub fn computed_root(&self, network: NetworkId) -> Option<Hash> {
        let empty = Hash::new([]);
        if self.height < 2
            || self.siblings.len() > 256
            || self.siblings.iter().any(|step| step.hash == empty)
            || self
                .siblings
                .windows(2)
                .any(|pair| pair[0].level >= pair[1].level)
        {
            return None;
        }
        let value = lifecycle_runtime_anchor(
            network,
            self.height,
            self.execution_selection,
            self.post_selection,
        );
        let path = Hash::new(KEY);
        let mut current = leaf(path, Hash::new(value));
        let mut siblings = self.siblings.iter().peekable();
        for level in 0..256 {
            let sibling = if siblings
                .peek()
                .is_some_and(|step| step.level as usize == level)
            {
                siblings.next()?.hash
            } else {
                empty
            };
            let bit = 255 - level;
            current = if path.as_ref()[bit / 8] & (1 << (bit % 8)) != 0 {
                branch(sibling, current)
            } else {
                branch(current, sibling)
            };
        }
        Some(current)
    }

    /// Verify both exact selected cuts and the existing certified ordinary-write root.
    #[must_use]
    pub fn verify(
        &self,
        network: NetworkId,
        height: u64,
        selection: SelectionId,
        root: Hash,
    ) -> bool {
        self.height == height
            && self.execution_selection == selection
            && self.post_selection == selection
            && self.computed_root(network) == Some(root)
    }
}

fn leaf(path: Hash, value: Hash) -> Hash {
    Hash::new_from_chunks(&[&[0], path.as_ref(), value.as_ref()])
}
fn branch(left: Hash, right: Hash) -> Hash {
    Hash::new_from_chunks(&[&[1], left.as_ref(), right.as_ref()])
}
fn lookup<'a>(nodes: &'a [Node], path: &[u8; 32]) -> Option<&'a Node> {
    nodes
        .binary_search_by_key(path, |node| node.path)
        .ok()
        .map(|index| &nodes[index])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::consensus::ExecKv;
    use iroha_crypto::HashOf;

    fn fixture() -> (NetworkId, SelectionId, ExecWitness) {
        let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle network",
        )));
        let selection = SelectionId(Hash::new(b"selected successor").into());
        let mut witness = ExecWitness::default();
        witness.writes.push(ExecKv {
            key: KEY.to_vec(),
            value: lifecycle_runtime_anchor(network, 7, selection, selection).to_vec(),
        });
        for index in 0..32u64 {
            witness.writes.push(ExecKv {
                key: index.to_le_bytes().to_vec(),
                value: vec![index as u8; 16],
            });
        }
        (network, selection, witness)
    }

    #[test]
    fn lifecycle_runtime_selective_branch_binds_exact_native_value_and_root() {
        let (network, selection, witness) = fixture();
        let pool = AllocationBudget::new(1_000_000);
        let retained = LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).unwrap();
        let proof = retained.proof();
        assert!(proof.siblings.len() < 16);
        let root = proof.computed_root(network).unwrap();
        assert!(proof.verify(network, 7, selection, root));
        assert!(!proof.verify(network, 8, selection, root));
        assert!(!proof.verify(network, 7, SelectionId([4; 32]), root));
        assert!(!proof.verify(network, 7, selection, Hash::new(b"foreign root")));
        let other = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign network",
        )));
        assert!(!proof.verify(other, 7, selection, root));
        let encoded = norito::json::to_json(proof).unwrap();
        let decoded: LifecycleRuntimeProofV1 = norito::json::from_json(&encoded).unwrap();
        assert_eq!(&decoded, proof);
        assert!(decoded.verify(network, 7, selection, root));
        assert!(pool.reserved_bytes() > 0);
        drop(retained);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn lifecycle_runtime_branch_rejects_noncanonical_paths_and_unowned_anchor() {
        let (network, _, mut witness) = fixture();
        let pool = AllocationBudget::new(1_000_000);
        let retained = LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).unwrap();
        let mut proof = retained.proof().clone();
        proof.siblings.push(proof.siblings[0].clone());
        assert!(proof.computed_root(network).is_none());
        let mut proof = retained.proof().clone();
        proof.siblings[0].hash = Hash::new([]);
        assert!(proof.computed_root(network).is_none());
        let mut proof = retained.proof().clone();
        proof.siblings.reverse();
        assert!(proof.computed_root(network).is_none());
        witness.writes.push(witness.writes[0].clone());
        assert!(LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).is_err());
        witness.writes.retain(|write| write.key != KEY);
        assert!(LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).is_err());
    }

    #[test]
    fn lifecycle_runtime_proof_retains_last_writes_and_refuses_unpaid_backing() {
        let (network, selection, mut witness) = fixture();
        let pool = AllocationBudget::new(1_000_000);
        let original = LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).unwrap();
        witness.writes.push(witness.writes[1].clone());
        let repeated = LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).unwrap();
        assert_eq!(original.proof(), repeated.proof());
        witness.writes.last_mut().unwrap().value.push(2);
        let changed = LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).unwrap();
        assert!(!changed.proof().verify(
            network,
            7,
            selection,
            original.proof().computed_root(network).unwrap()
        ));
        let empty_pool = AllocationBudget::new(0);
        assert!(LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &empty_pool).is_err());
        assert_eq!(empty_pool.reserved_bytes(), 0);
    }

    #[test]
    fn lifecycle_runtime_thousands_of_cuts_have_no_world_size_term() {
        // This transport takes only the block's ordinary writes. A million
        // untouched World entries are neither an input nor a retained output.
        let (network, selection, witness) = fixture();
        let pool = AllocationBudget::new(1_000_000);
        let retained = LifecycleRuntimeProofV1::from_witness(&witness, network, 7, &pool).unwrap();
        let mut cuts = Vec::new();
        for height in 7..4103 {
            let mut proof = retained.proof().clone();
            proof.height = height;
            let root = proof.computed_root(network).unwrap();
            assert!(proof.verify(network, height, selection, root));
            cuts.push(proof);
        }
        let json = norito::json::to_json(&cuts).unwrap();
        assert_eq!(cuts.len(), 4096);
        assert!(json.len() < 8 * 1024 * 1024, "{} bytes", json.len());
    }
}
