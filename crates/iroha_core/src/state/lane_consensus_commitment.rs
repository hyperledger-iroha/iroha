//! Bounded execution-witness commitment to the complete frozen lane context set.
//!
//! The full values remain in transactional State and snapshots. This commitment
//! is included in the carrier's execution witness; hashing it alone does not
//! prove carrier finality, membership or absence to a remote consumer.

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::NetworkId;
use norito::codec::{Decode, Encode};

use super::lane_consensus_context::{FrozenLaneConsensusContextV1, LaneConsensusContextsV1};

const EMPTY_ROOT_DOMAIN: &[u8] = b"iroha:lane-consensus:empty-context-tree:v1\0";

/// Exact root and count for one carrier's post-execution open-instance set.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::state::LaneConsensusContextsCommitmentV1")]
pub(crate) struct LaneConsensusContextsCommitmentV1 {
    version: u16,
    network_id: NetworkId,
    carrier_height: u64,
    root: Hash,
    count: u32,
}

impl LaneConsensusContextsCommitmentV1 {
    /// Commit canonical route order, exact authority bytes and explicit emptiness.
    pub(crate) fn from_contexts(
        network_id: NetworkId,
        carrier_height: u64,
        contexts: &LaneConsensusContextsV1,
    ) -> Result<Self, String> {
        if carrier_height == 0 {
            return Err("lane context commitment carrier height is zero".to_owned());
        }
        contexts.validate().map_err(|error| error.to_string())?;
        if contexts.contexts.iter().any(|context| {
            context.network_id != network_id || context.opening_global_height > carrier_height
        }) {
            return Err("lane context commitment has a foreign or future opening".to_owned());
        }
        let leaves = contexts
            .contexts
            .iter()
            .map(context_leaf)
            .collect::<Result<Vec<_>, _>>()?;
        let root = MerkleTree::<FrozenLaneConsensusContextV1>::root_from_typed_leaves(leaves)
            .map_or_else(|| Hash::new(EMPTY_ROOT_DOMAIN), Into::into);
        Ok(Self {
            version: 1,
            network_id,
            carrier_height,
            root,
            count: u32::try_from(contexts.contexts.len())
                .map_err(|_| "lane context commitment count overflow".to_owned())?,
        })
    }

    /// Check a decoded commitment without treating its bytes as finality.
    pub(super) fn validate(&self) -> Result<(), String> {
        let empty = Hash::new(EMPTY_ROOT_DOMAIN);
        if self.version != 1
            || self.carrier_height == 0
            || usize::try_from(self.count).map_or(true, |count| {
                count > iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES
            })
            || (self.count == 0) != (self.root == empty)
            || self.root == Hash::prehashed([0; Hash::LENGTH])
        {
            return Err("invalid lane context root, count, revision or carrier".to_owned());
        }
        Ok(())
    }

    /// Bind a proof to the independently authenticated carrier and network.
    pub(super) fn matches_carrier(&self, network_id: NetworkId, height: u64) -> bool {
        self.network_id == network_id && self.carrier_height == height
    }

    /// Return the carrier whose execution witness contains this commitment.
    pub(super) fn carrier_height(&self) -> u64 {
        self.carrier_height
    }
}

/// Use the frozen context's explicit domain-separated canonical hash as the
/// application-Merkle prehash, consistently for roots and membership proofs.
fn context_leaf(
    context: &FrozenLaneConsensusContextV1,
) -> Result<HashOf<FrozenLaneConsensusContextV1>, String> {
    context
        .canonical_hash()
        .map(HashOf::from_untyped_unchecked)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{MerkleProof, MerkleTreeCommitment};
    use iroha_model_base::topology::LaneId;
    use std::num::NonZeroU64;

    fn contexts(count: u32) -> LaneConsensusContextsV1 {
        let template = super::super::lane_consensus_context::frozen_lane_context_fixture_for_test();
        LaneConsensusContextsV1::new(
            (0..count)
                .map(|index| {
                    let mut context = template.clone();
                    context.lane_id = LaneId::new(index);
                    context
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn lane_context_commitment_rejects_incoherent_decoded_root_and_count() {
        let contexts = contexts(1);
        let exact = LaneConsensusContextsCommitmentV1::from_contexts(
            contexts.contexts[0].network_id,
            contexts.contexts[0].opening_global_height,
            &contexts,
        )
        .unwrap();
        assert!(exact.validate().is_ok());
        for change in 0..5 {
            let mut invalid = exact.clone();
            match change {
                0 => invalid.version = 2,
                1 => invalid.count = 0,
                2 => invalid.count = u32::MAX,
                3 => invalid.root = Hash::prehashed([0; Hash::LENGTH]),
                4 => invalid.root = Hash::new(EMPTY_ROOT_DOMAIN),
                _ => unreachable!(),
            }
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn lane_context_commitment_bounds_witness_size_and_binds_complete_set() {
        let one = contexts(1);
        let three = contexts(3);
        let network = one.contexts[0].network_id;
        let height = one.contexts[0].opening_global_height;
        let first =
            LaneConsensusContextsCommitmentV1::from_contexts(network, height, &one).unwrap();
        let all =
            LaneConsensusContextsCommitmentV1::from_contexts(network, height, &three).unwrap();
        assert_ne!(first.root, all.root);
        let encoded = norito::to_bytes(&all).unwrap();
        let single_encoded = norito::to_bytes(&first).unwrap();
        assert!(encoded.len().abs_diff(single_encoded.len()) <= 4);
        assert!(encoded.len() < norito::to_bytes(&one).unwrap().len());
        assert_eq!(
            norito::decode_from_bytes::<LaneConsensusContextsCommitmentV1>(&encoded).unwrap(),
            all
        );
        let mut changed = three.clone();
        changed.contexts[1].leader_seed[0] ^= 1;
        assert_ne!(
            LaneConsensusContextsCommitmentV1::from_contexts(network, height, &changed)
                .unwrap()
                .root,
            all.root
        );
        changed.contexts.swap(0, 1);
        assert!(
            LaneConsensusContextsCommitmentV1::from_contexts(network, height, &changed).is_err()
        );
        let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign network",
        )));
        assert!(LaneConsensusContextsCommitmentV1::from_contexts(foreign, height, &one).is_err());
        assert!(
            LaneConsensusContextsCommitmentV1::from_contexts(network, height - 1, &one).is_err()
        );
    }

    #[test]
    fn lane_context_commitment_authenticates_explicit_empty_state_at_each_carrier() {
        let template = contexts(1);
        let network = template.contexts[0].network_id;
        let empty = LaneConsensusContextsV1::default();
        let first = LaneConsensusContextsCommitmentV1::from_contexts(network, 1, &empty).unwrap();
        let next = LaneConsensusContextsCommitmentV1::from_contexts(network, 2, &empty).unwrap();
        assert_eq!(first.count, 0);
        assert_eq!(first.root, Hash::new(EMPTY_ROOT_DOMAIN));
        assert_ne!(
            norito::to_bytes(&first).unwrap(),
            norito::to_bytes(&next).unwrap()
        );
        assert!(LaneConsensusContextsCommitmentV1::from_contexts(network, 0, &empty).is_err());
    }

    #[test]
    fn lane_context_commitment_proof_requires_exact_count_and_ragged_edge() {
        let contexts = contexts(3);
        let first = &contexts.contexts[0];
        let summary = LaneConsensusContextsCommitmentV1::from_contexts(
            first.network_id,
            first.opening_global_height,
            &contexts,
        )
        .unwrap();
        let leaves = contexts
            .contexts
            .iter()
            .map(context_leaf)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let tree: MerkleTree<FrozenLaneConsensusContextV1> = leaves.iter().copied().collect();
        let commitment = MerkleTreeCommitment::new(
            HashOf::from_untyped_unchecked(summary.root),
            NonZeroU64::new(u64::from(summary.count)).unwrap(),
        );
        for (index, leaf) in leaves.iter().enumerate() {
            assert!(
                tree.get_proof(index as u32)
                    .unwrap()
                    .verify(leaf, &commitment)
            );
        }
        let edge = tree.get_proof(2).unwrap();
        assert!(!edge.verify(&leaves[1], &commitment));
        let wrong_count =
            MerkleTreeCommitment::new(*commitment.root(), NonZeroU64::new(4).unwrap());
        assert!(!edge.verify(&leaves[2], &wrong_count));
        let mut path = edge.audit_path().to_vec();
        assert!(path[0].is_none());
        path[0] = Some(leaves[2]);
        assert!(!MerkleProof::from_audit_path(2, path).verify(&leaves[2], &commitment));
    }
}
