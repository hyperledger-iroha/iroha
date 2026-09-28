//! Fixed ordinary-write commitment to the global chain's complete lane lifecycle state.
//!
//! This is the actual `World.sumeragi_lanes` value. A commitment grants no authority until its
//! ordinary-write path is checked against the result of an independently certified global block.
use crate::{NetworkId, sumeragi_lanes::SumeragiLaneState};
use iroha_crypto::Hash;
use norito::{Decode, Encode};

const DOMAIN: &[u8] = b"iroha:sumeragi:lane-state-value:v1\0";

/// Complete canonical lane state at one exact global execution height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::SumeragiLaneStateCommitment")]
pub struct SumeragiLaneStateCommitment {
    network_id: NetworkId,
    carrier_height: u64,
    state_hash: Hash,
}

impl SumeragiLaneStateCommitment {
    /// Bind every lane field, including empty state, samples and the incarnation counter.
    /// No dynamic hashing scratch or intermediate canonical byte vector is allocated.
    /// # Errors
    /// Rejects a zero carrier height, unordered lanes/samples or a canonical encoding failure.
    pub fn from_state(
        network: NetworkId,
        height: u64,
        state: &SumeragiLaneState,
    ) -> Result<Self, String> {
        Self::from_state_encoding(network, height, state).map_err(|error| error.to_string())
    }

    /// Equality-only streaming projection of the original state. This does not verify finality.
    /// # Errors
    /// Rejects noncanonical state ordering or a canonical encoding failure.
    pub fn from_state_encoding(
        network_id: NetworkId,
        carrier_height: u64,
        state: &SumeragiLaneState,
    ) -> Result<Self, norito::Error> {
        if carrier_height == 0
            || state
                .lanes
                .windows(2)
                .any(|pair| pair[0].lane >= pair[1].lane)
            || state
                .lanes
                .iter()
                .any(|lane| lane.lane.as_u32() == 0 || lane.created_at > carrier_height)
            || state
                .samples
                .windows(2)
                .any(|pair| pair[0].height >= pair[1].height)
            || state
                .samples
                .iter()
                .any(|sample| sample.height > carrier_height)
            || state.last_transition > carrier_height
        {
            return Err(norito::Error::NonCanonicalEncoding);
        }
        let state_hash = Hash::new_from_writer(|writer| {
            writer.write_all(DOMAIN)?;
            norito::core::write_canonical_to_writer(state, writer).map_err(std::io::Error::other)
        })
        .map_err(|_| norito::Error::NonCanonicalEncoding)?;
        Ok(Self {
            network_id,
            carrier_height,
            state_hash,
        })
    }

    /// Validate only the fixed commitment shape; external finality supplies its authority.
    /// # Errors
    /// Rejects a zero carrier height.
    pub fn validate(&self) -> Result<(), String> {
        if self.carrier_height == 0 {
            return Err("zero lane state carrier height".into());
        }
        Ok(())
    }

    /// Bind a proof to the independently authenticated network and global carrier.
    pub fn matches_carrier(&self, network: NetworkId, height: u64) -> bool {
        self.network_id == network && self.carrier_height == height
    }

    /// Exact carrier height committed by this value.
    pub fn carrier_height(&self) -> u64 {
        self.carrier_height
    }

    /// Hash of the complete canonical state, used to seal the original overlay.
    pub fn state_hash(&self) -> Hash {
        self.state_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi_lanes::{
        SumeragiLaneFrontier, SumeragiLaneMember, SumeragiLaneRecord, SumeragiLaneSample,
    };
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_model_base::{
        peer::PeerId,
        topology::{DataSpaceId, LaneId},
    };

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"lane state fixture",
        )))
    }
    fn state(count: u32) -> SumeragiLaneState {
        let mut committee = (1..=4)
            .map(|seed| {
                let keys = KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal);
                SumeragiLaneMember {
                    peer: PeerId::new(keys.public_key().clone()),
                    pop: iroha_crypto::bls_normal_pop_prove(keys.private_key()).unwrap(),
                }
            })
            .collect::<Vec<_>>();
        committee.sort();
        SumeragiLaneState {
            lanes: (1..=count)
                .map(|lane| SumeragiLaneRecord {
                    lane: LaneId::new(lane),
                    dataspace: DataSpaceId::new(1),
                    incarnation: [lane as u8; 32],
                    params: Default::default(),
                    committee: committee.clone(),
                    created_at: 1,
                    active_from: 3,
                    closing: None,
                    anchor_freshness: 16,
                    merged: SumeragiLaneFrontier::default(),
                    merged_at: 3,
                    rescued: 0,
                })
                .collect(),
            samples: vec![SumeragiLaneSample {
                height: 1,
                time_ms: 1000,
                transactions: 0,
                lanes: count + 1,
            }],
            last_transition: 0,
            incarnations: u64::from(count),
        }
    }
    #[test]
    fn streaming_commitment_binds_every_state_component_and_exact_carrier() {
        for count in [0, 1, 3, 4, 7, 31] {
            let value = state(count);
            let exact = SumeragiLaneStateCommitment::from_state(network(), 4, &value).unwrap();
            let bytes = norito::encode_canonical(&value).unwrap();
            assert_eq!(exact.state_hash, Hash::new_from_chunks(&[DOMAIN, &bytes]));
            assert_eq!(
                exact,
                SumeragiLaneStateCommitment::from_state_encoding(network(), 4, &value).unwrap()
            );
            for field in 0..4 {
                let mut changed = value.clone();
                match field {
                    0 => changed.incarnations += 1,
                    1 => changed.last_transition = 1,
                    2 => changed.samples[0].transactions += 1,
                    _ => changed.samples[0].time_ms += 1,
                }
                assert_ne!(
                    exact,
                    SumeragiLaneStateCommitment::from_state(network(), 4, &changed).unwrap()
                );
            }
            if count > 0 {
                for field in 0..10 {
                    let mut changed = value.clone();
                    let lane = &mut changed.lanes[0];
                    match field {
                        0 => lane.incarnation[0] ^= 1,
                        1 => lane.committee[0].pop[0] ^= 1,
                        2 => lane.anchor_freshness += 1,
                        3 => lane.active_from += 1,
                        4 => lane.closing = Some(4),
                        5 => lane.merged.height += 1,
                        6 => lane.merged.block_hash[0] ^= 1,
                        7 => lane.merged.result[0] ^= 1,
                        8 => lane.merged_at += 1,
                        _ => lane.rescued += 1,
                    }
                    assert_ne!(
                        exact,
                        SumeragiLaneStateCommitment::from_state(network(), 4, &changed).unwrap()
                    );
                }
            }
            if count > 1 {
                let mut changed = value.clone();
                changed.lanes.reverse();
                assert!(SumeragiLaneStateCommitment::from_state(network(), 4, &changed).is_err());
            }
            assert!(!exact.matches_carrier(network(), 5));
            let encoded = norito::encode_canonical(&exact).unwrap();
            assert!(encoded.len() < 512);
            assert_eq!(
                norito::decode_canonical::<SumeragiLaneStateCommitment>(&encoded).unwrap(),
                exact
            );
        }
    }
    #[test]
    fn empty_history_is_distinct_from_absent_lanes_with_retained_autoscale_history() {
        let empty = SumeragiLaneState::default();
        let exact = SumeragiLaneStateCommitment::from_state(network(), 1, &empty).unwrap();
        assert_ne!(
            exact,
            SumeragiLaneStateCommitment::from_state(network(), 2, &empty).unwrap()
        );
        assert_ne!(
            exact,
            SumeragiLaneStateCommitment::from_state(network(), 1, &state(0)).unwrap()
        );
        assert!(SumeragiLaneStateCommitment::from_state(network(), 0, &empty).is_err());
        let mut future = state(0);
        future.samples[0].height = 2;
        assert!(SumeragiLaneStateCommitment::from_state(network(), 1, &future).is_err());
        let mut duplicate = state(0);
        duplicate.samples.push(duplicate.samples[0].clone());
        assert!(SumeragiLaneStateCommitment::from_state(network(), 1, &duplicate).is_err());
    }
}
