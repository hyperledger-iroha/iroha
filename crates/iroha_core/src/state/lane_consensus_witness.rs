//! Retain the fixed context-set write against the global execution commitment.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::{NetworkId, block::consensus::ExecWitness};
use norito::codec::{Decode, Encode};

use super::{
    lane_consensus_commitment::LaneConsensusContextsCommitmentV1,
    lane_consensus_state::LANE_CONSENSUS_CONTEXTS_WITNESS_KEY,
};
use crate::sumeragi::smt::{KvPair, compute_post_state_root};

/// Canonical fixed-key sparse-Merkle proof retained with global finality.
///
/// The root must come from independently verified global finality. A valid
/// proof binds the complete set commitment, including its exact empty state;
/// it does not make an arbitrary context or an old carrier current.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::state::LaneConsensusContextsWitnessV1")]
pub(crate) struct LaneConsensusContextsWitnessV1 {
    commitment: LaneConsensusContextsCommitmentV1,
    siblings: Vec<Hash>,
}

impl LaneConsensusContextsWitnessV1 {
    /// Build one proof from the canonical validator-owned execution witness.
    pub(crate) fn from_witness(witness: &ExecWitness) -> Result<(Self, Hash), String> {
        let mut targets = witness
            .writes
            .iter()
            .filter(|entry| entry.key == LANE_CONSENSUS_CONTEXTS_WITNESS_KEY);
        let target = targets
            .next()
            .ok_or_else(|| "lane context witness write is missing".to_owned())?;
        if targets.next().is_some() {
            return Err("lane context witness write is duplicated".to_owned());
        }
        let commitment: LaneConsensusContextsCommitmentV1 = norito::decode_canonical(&target.value)
            .map_err(|error| format!("invalid canonical lane context commitment: {error}"))?;
        commitment.validate()?;
        if norito::to_bytes(&commitment).map_err(|error| error.to_string())? != target.value {
            return Err("lane context commitment has an alternate encoding".to_owned());
        }
        let canonical = witness
            .writes
            .iter()
            .map(|entry| (entry.key.clone(), entry.value.clone()))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(key, value)| KvPair::new(key, value))
            .collect::<Vec<_>>();
        let root = compute_post_state_root(&[], &canonical);
        let target = KvPair::new(target.key.clone(), target.value.clone());
        let siblings = crate::receiver_snapshot::sparse_smt_siblings(&canonical, &target)?;
        let result = Self {
            commitment,
            siblings,
        };
        if !result.verify_root(root) {
            return Err(
                "lane context witness path differs from the ordinary-write root".to_owned(),
            );
        }
        Ok((result, root))
    }

    /// Verify exact key, canonical value, fixed depth and externally supplied root.
    pub(crate) fn verify_root(&self, root: Hash) -> bool {
        if self.commitment.validate().is_err() || self.siblings.len() != 256 {
            return false;
        }
        let Ok(value) = norito::to_bytes(&self.commitment) else {
            return false;
        };
        let path = Hash::new(LANE_CONSENSUS_CONTEXTS_WITNESS_KEY);
        let value_hash = Hash::new(value);
        let mut current = Hash::new_from_chunks(&[&[0], path.as_ref(), value_hash.as_ref()]);
        for (level, sibling) in self.siblings.iter().enumerate() {
            let bit = 255 - level;
            let right = path.as_ref()[bit / 8] & (1 << (bit % 8)) != 0;
            let (left, right) = if right {
                (sibling, &current)
            } else {
                (&current, sibling)
            };
            current = Hash::new_from_chunks(&[&[1], left.as_ref(), right.as_ref()]);
        }
        current == root
    }

    /// Verify proof and identity against the exact finalized global carrier.
    pub(crate) fn verify(&self, network: NetworkId, height: u64, root: Hash) -> bool {
        self.commitment.matches_carrier(network, height) && self.verify_root(root)
    }

    /// Check staged execution height before a finality artifact exists.
    pub(crate) fn carrier_height(&self) -> u64 {
        self.commitment.carrier_height()
    }

    /// Match the complete native set, including authenticated absence.
    pub(super) fn matches_contexts(
        &self,
        network: NetworkId,
        height: u64,
        contexts: &super::LaneConsensusContextsV1,
    ) -> Result<bool, String> {
        Ok(self.commitment
            == LaneConsensusContextsCommitmentV1::from_contexts(network, height, contexts)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{FrozenLaneConsensusContextV1, LaneConsensusContextsV1};
    use iroha_data_model::block::consensus::ExecKv;

    fn fixture() -> (FrozenLaneConsensusContextV1, ExecWitness) {
        let context = super::super::lane_consensus_context::frozen_lane_context_fixture_for_test();
        let commitment = LaneConsensusContextsCommitmentV1::from_contexts(
            context.network_id,
            context.opening_global_height,
            &LaneConsensusContextsV1::new(vec![context.clone()]).unwrap(),
        )
        .unwrap();
        let witness = ExecWitness {
            writes: vec![
                ExecKv {
                    key: b"unrelated ordinary write".to_vec(),
                    value: vec![7],
                },
                ExecKv {
                    key: LANE_CONSENSUS_CONTEXTS_WITNESS_KEY.to_vec(),
                    value: norito::to_bytes(&commitment).unwrap(),
                },
            ],
            ..ExecWitness::default()
        };
        (context, witness)
    }

    #[test]
    fn lane_context_witness_binds_native_context_to_exact_carrier_write_root() {
        let (context, witness) = fixture();
        let (proof, root) = LaneConsensusContextsWitnessV1::from_witness(&witness).unwrap();
        assert!(proof.verify(context.network_id, context.opening_global_height, root));
        assert!(!proof.verify(context.network_id, context.opening_global_height + 1, root));
        let foreign = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"foreign witness network"),
        ));
        assert!(!proof.verify(foreign, context.opening_global_height, root));
        assert!(!proof.verify_root(Hash::new(b"another ordinary root")));
        assert_eq!(proof.carrier_height(), context.opening_global_height);
        assert!(
            proof
                .matches_contexts(
                    context.network_id,
                    context.opening_global_height,
                    &LaneConsensusContextsV1::new(vec![context.clone()]).unwrap()
                )
                .unwrap()
        );
        assert!(
            !proof
                .matches_contexts(
                    context.network_id,
                    context.opening_global_height,
                    &LaneConsensusContextsV1::default()
                )
                .unwrap()
        );
        let decoded: LaneConsensusContextsWitnessV1 =
            norito::decode_canonical(&norito::to_bytes(&proof).unwrap()).unwrap();
        assert_eq!(proof, decoded);
        let mut altered = proof.clone();
        altered.siblings[128] = Hash::new(b"substituted sibling");
        assert!(!altered.verify_root(root));
        let mut truncated = proof;
        truncated.siblings.pop();
        assert!(!truncated.verify_root(root));
    }

    #[test]
    fn lane_context_witness_rejects_missing_duplicate_and_noncanonical_values() {
        let (_, mut witness) = fixture();
        let exact = witness.clone();
        witness.writes.pop();
        assert!(LaneConsensusContextsWitnessV1::from_witness(&witness).is_err());
        witness = exact.clone();
        witness.writes.push(witness.writes[1].clone());
        assert!(LaneConsensusContextsWitnessV1::from_witness(&witness).is_err());
        witness = exact;
        witness.writes[1].value.push(0);
        assert!(LaneConsensusContextsWitnessV1::from_witness(&witness).is_err());
    }
}
