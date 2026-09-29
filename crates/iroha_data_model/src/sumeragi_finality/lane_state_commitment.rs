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

    /// Equality-only hash of a borrowed original canonical lane payload. The native result
    /// path must independently authenticate this commitment before equality authorizes use.
    pub(super) fn matches_state_payload(
        &self,
        network: NetworkId,
        height: u64,
        payload: &[u8],
    ) -> Result<bool, norito::Error> {
        if !self.matches_carrier(network, height) || height == 0 {
            return Ok(false);
        }
        let frame = StatePayload {
            bytes: payload,
            _alignment: [],
        };
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(DOMAIN)?;
            norito::core::write_canonical_to_writer(&frame, writer).map_err(std::io::Error::other)
        })
        .map_err(|_| norito::Error::NonCanonicalEncoding)?;
        Ok(hash == self.state_hash)
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

// A view of the existing payload, with the original wire type's exact alignment. This is
// only a streaming equality projection: it implements no decoder or alternate wire format.
struct StatePayload<'a> {
    bytes: &'a [u8],
    _alignment: [SumeragiLaneState; 0],
}
const _: () =
    assert!(std::mem::align_of::<StatePayload<'_>>() == std::mem::align_of::<SumeragiLaneState>());
impl norito::core::SerializePayload for StatePayload<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        // The sole canonical SumeragiLaneState payload always emits its four compact field
        // lengths. Propagate that known layout usage while forwarding the original bytes.
        norito::core::note_compact_len_emitted();
        std::io::Write::write_all(writer, self.bytes)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(self.bytes.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(self.bytes.len())
    }
}
impl norito::NoritoSchema for StatePayload<'_> {
    fn nominal_name() -> String {
        <SumeragiLaneState as norito::NoritoSchema>::nominal_name()
    }
    fn static_frame_name() -> Option<&'static str> {
        <SumeragiLaneState as norito::NoritoSchema>::static_frame_name()
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
                    da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
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
    fn borrowed_payload_equality_preserves_the_complete_original_canonical_frame() {
        for count in [0, 1, 4, 7, 31] {
            let original = state(count);
            let commitment =
                SumeragiLaneStateCommitment::from_state(network(), 4, &original).unwrap();
            let frame = norito::encode_canonical(&original).unwrap();
            let view = norito::core::from_bytes_view(&frame).unwrap();
            let borrowed = StatePayload {
                bytes: view.as_bytes(),
                _alignment: [],
            };
            assert_eq!(norito::encode_canonical(&borrowed).unwrap(), frame);
            assert!(
                commitment
                    .matches_state_payload(network(), 4, view.as_bytes())
                    .unwrap()
            );
            assert!(
                !commitment
                    .matches_state_payload(network(), 5, view.as_bytes())
                    .unwrap()
            );
            let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"foreign lane source",
            )));
            assert!(
                !commitment
                    .matches_state_payload(foreign, 4, view.as_bytes())
                    .unwrap()
            );
            for malformed in [
                vec![],
                view.as_bytes()[..view.as_bytes().len() - 1].to_vec(),
                {
                    let mut bytes = view.as_bytes().to_vec();
                    bytes.push(0);
                    bytes
                },
            ] {
                assert!(
                    !commitment
                        .matches_state_payload(network(), 4, &malformed)
                        .unwrap()
                );
            }
            // Alter every lane field family, including live BLS keys/PoPs and parameter vectors.
            if count > 0 {
                for field in 0..16 {
                    let mut changed = original.clone();
                    let lane = &mut changed.lanes[0];
                    match field {
                        0 => lane.lane = LaneId::new(99),
                        1 => lane.dataspace = DataSpaceId::new(2),
                        2 => lane.incarnation[0] ^= 1,
                        3 => lane.params.key_allowed_algorithms.clear(),
                        4 => {
                            lane.committee[0].peer = PeerId::new(
                                KeyPair::from_seed(vec![77; 32], Algorithm::BlsNormal)
                                    .public_key()
                                    .clone(),
                            )
                        }
                        5 => lane.committee[0].pop[0] ^= 1,
                        6 => lane.created_at += 1,
                        7 => lane.active_from += 1,
                        8 => lane.closing = Some(4),
                        9 => lane.anchor_freshness += 1,
                        10 => lane.merged.height += 1,
                        11 => lane.merged.block_hash[0] ^= 1,
                        12 => lane.merged.result[0] ^= 1,
                        13 => lane.merged_at += 1,
                        14 => lane.rescued += 1,
                        _ => lane.params.max_clock_drift_ms += 1,
                    }
                    let frame = norito::encode_canonical(&changed).unwrap();
                    let view = norito::core::from_bytes_view(&frame).unwrap();
                    assert!(
                        !commitment
                            .matches_state_payload(network(), 4, view.as_bytes())
                            .unwrap(),
                        "field {field}"
                    );
                }
            }
            for field in 0..6 {
                let mut changed = original.clone();
                match field {
                    0 => changed.samples[0].height += 1,
                    1 => changed.samples[0].time_ms += 1,
                    2 => changed.samples[0].transactions += 1,
                    3 => changed.samples[0].lanes += 1,
                    4 => changed.last_transition += 1,
                    _ => changed.incarnations += 1,
                }
                let frame = norito::encode_canonical(&changed).unwrap();
                assert!(
                    !commitment
                        .matches_state_payload(
                            network(),
                            4,
                            norito::core::from_bytes_view(&frame).unwrap().as_bytes()
                        )
                        .unwrap()
                );
            }
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
