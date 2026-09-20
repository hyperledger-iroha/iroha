//! State-local test projection for model-owned frozen lane values.

#[cfg(test)]
use iroha_crypto::Hash;
pub(crate) use iroha_data_model::block::lane_consensus::{
    FrozenLaneConsensusContextV1, LaneConsensusContextsV1,
};
#[cfg(test)]
use iroha_data_model::{
    NetworkId,
    block::{consensus_v2 as wire, lane_consensus::LaneConsensusContextError},
    nexus::MAX_ACTIVE_EXECUTION_LANES,
};
#[cfg(test)]
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};

// Raw frozen structure is never a production voting capability. The production
// factory remains privately owned by lane_consensus_verified.
#[cfg(test)]
trait TestCoreContextProjection {
    fn core_context_for_test(
        &self,
        opening_subject: wire::BlockSubject,
    ) -> Result<crate::sumeragi::v2_core::HeightContext, String>;
}
#[cfg(test)]
impl TestCoreContextProjection for FrozenLaneConsensusContextV1 {
    fn core_context_for_test(
        &self,
        opening_subject: wire::BlockSubject,
    ) -> Result<crate::sumeragi::v2_core::HeightContext, String> {
        use crate::sumeragi::v2_core as core;
        let frozen_hash = self.canonical_hash().map_err(|error| error.to_string())?;
        let subject_bytes =
            norito::encode_canonical(&opening_subject).map_err(|error| error.to_string())?;
        let subject_hash = Hash::new(&subject_bytes);
        let instance_hash = Hash::new_from_chunks(&[
            b"iroha:lane-consensus:finalized-instance:v1\0",
            frozen_hash.as_ref(),
            &subject_bytes,
        ]);
        let layout =
            norito::encode_canonical(&self.da_layout).map_err(|error| error.to_string())?;
        let roster = self
            .committee
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let mut token = [0; Hash::LENGTH];
                // validate() bounds this index by the native maximum committee.
                token[28..].copy_from_slice(&(index as u32).to_be_bytes());
                core::Validator::new(core::ValidatorId::new(token), core::VotingPower::new(1))
            })
            .collect();
        core::HeightContext::new_from_finalized_state(
            core::ContextId::new(*instance_hash.as_ref()),
            core::NetworkId::new(*self.network_id.as_bytes()),
            self.next_lane_height,
            core::FinalizedStateAnchor {
                context_id: core::ContextId::new(*self.opening_global_context_id.0.as_ref()),
                height: self.opening_global_height,
                subject: core::Subject::new(*subject_hash.as_ref()),
                predecessor_height: self.predecessor_height,
                predecessor_subject: self
                    .predecessor_hash
                    .map(|hash| core::Subject::new(*hash.as_ref())),
            },
            self.epoch,
            roster,
            match self.mode {
                wire::ConsensusMode::Permissioned => core::VotingMode::Permissioned,
                wire::ConsensusMode::Npos => core::VotingMode::Npos,
            },
            core::Digest::new(*self.nexus_amx_context_hash.as_ref()),
            core::Digest::new(*self.execution_policy_hash.as_ref()),
            core::Digest::new(*Hash::new(layout).as_ref()),
            core::Digest::new(self.leader_seed),
        )
        .map_err(|error| error.to_string())
    }
}

/// Shared native-key fixture for State storage, witness and snapshot tests.
#[cfg(test)]
pub(super) fn frozen_lane_context_fixture_for_test() -> FrozenLaneConsensusContextV1 {
    tests::fixture()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use std::sync::OnceLock;

    pub(super) fn fixture() -> FrozenLaneConsensusContextV1 {
        static FIXTURE: OnceLock<FrozenLaneConsensusContextV1> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let mut validators = (1..=4)
                    .map(|seed| {
                        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                            .expect("deterministic native lane BLS key");
                        let pop = iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("native BLS proof");
                        (PeerId::new(key.public_key().clone()), pop)
                    })
                    .collect::<Vec<_>>();
                validators.sort_by(|left, right| left.0.cmp(&right.0));
                FrozenLaneConsensusContextV1 {
                    network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                        Hash::new(b"lane-context-network"),
                    )),
                    protocol_version: wire::PROTOCOL_VERSION,
                    opening_global_height: 41,
                    opening_global_context_id: wire::HeightContextId(
                        HashOf::from_untyped_unchecked(Hash::new(b"opening-global-context")),
                    ),
                    admitted_binding_hash: Hash::new(b"oldest admitted binding"),
                    admission_priority: super::super::QueuePlanAdmissionPriorityV1::new(40, 0)
                        .unwrap(),
                    epoch: 7,
                    mode: wire::ConsensusMode::Permissioned,
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(2),
                    lane_incarnation: Hash::new(b"lane-incarnation"),
                    next_lane_height: 3,
                    predecessor_height: 2,
                    predecessor_hash: Some(Hash::new(b"predecessor-descriptor")),
                    predecessor_applied_global_height: 40,
                    committee: validators.iter().map(|(peer, _)| peer.clone()).collect(),
                    validator_set_pops: validators.into_iter().map(|(_, pop)| pop).collect(),
                    nexus_amx_context_hash: Hash::new(b"frozen-nexus-policy"),
                    execution_policy_hash: Hash::new(b"frozen-execution-policy"),
                    da_layout: wire::recommended_data_availability_layout(),
                    leader_seed: [0; Hash::LENGTH],
                }
            })
            .clone()
    }

    fn subject() -> wire::BlockSubject {
        wire::BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(b"opening-parent"))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"opening-block")),
            payload_hash: Hash::new(b"opening-payload"),
        }
    }

    #[test]
    fn frozen_lane_context_roundtrips_canonical_norito_and_json() {
        let context = fixture();
        context.validate().expect("native four-validator context");
        assert_eq!(context.minimum_signer_count(), Ok(3));
        assert_eq!(
            context.route_key(),
            (
                context.lane_id,
                context.dataspace_id,
                context.lane_incarnation
            )
        );
        let bytes = norito::encode_canonical(&context).expect("canonical context");
        let decoded: FrozenLaneConsensusContextV1 =
            norito::decode_canonical(&bytes).expect("decode context");
        assert_eq!(decoded, context);
        assert_eq!(norito::encode_canonical(&decoded).expect("reencode"), bytes);
        let json = norito::json::to_json(&context).expect("context JSON");
        assert_eq!(
            norito::json::from_str::<FrozenLaneConsensusContextV1>(&json).expect("decode JSON"),
            context
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(norito::decode_canonical::<FrozenLaneConsensusContextV1>(&trailing).is_err());
        assert!(norito::json::from_str::<FrozenLaneConsensusContextV1>("{}").is_err());
    }

    #[test]
    fn frozen_lane_context_rejects_opening_and_predecessor_drift() {
        let valid = fixture();
        let mut cases = Vec::new();
        let mut changed = valid.clone();
        changed.protocol_version += 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.opening_global_height = 0;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.next_lane_height += 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_height = u64::MAX;
        changed.next_lane_height = 0;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_hash = None;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_hash = Some(Hash::prehashed([0; Hash::LENGTH]));
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_applied_global_height = 0;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_applied_global_height = 42;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_height = 0;
        changed.next_lane_height = 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_height = 0;
        changed.next_lane_height = 1;
        changed.predecessor_hash = None;
        cases.push(changed);
        for changed in cases {
            assert!(
                changed.validate().is_err(),
                "malformed context: {changed:?}"
            );
            assert!(
                changed.canonical_hash().is_err(),
                "hashing must not accept malformed context"
            );
        }
        let mut empty = valid.clone();
        empty.predecessor_height = 0;
        empty.next_lane_height = 1;
        empty.predecessor_hash = None;
        empty.predecessor_applied_global_height = 0;
        assert!(empty.validate().is_ok());
        let mut same_carrier = valid;
        same_carrier.predecessor_applied_global_height = same_carrier.opening_global_height;
        assert!(
            same_carrier.validate().is_ok(),
            "one carrier may apply the predecessor and open its successor"
        );
    }

    #[test]
    fn frozen_lane_context_rejects_zero_identities_and_invalid_signed_layout() {
        let valid = fixture();
        let zero = Hash::prehashed([0; Hash::LENGTH]);
        let mut cases = Vec::new();
        let mut changed = valid.clone();
        changed.admitted_binding_hash = zero;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.admission_priority.carrier_height = valid.opening_global_height + 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.admission_priority.carrier_height = 0;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(zero));
        cases.push(changed);
        let mut changed = valid.clone();
        changed.opening_global_context_id =
            wire::HeightContextId(HashOf::from_untyped_unchecked(zero));
        cases.push(changed);
        let mut changed = valid.clone();
        changed.lane_incarnation = zero;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.nexus_amx_context_hash = zero;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.execution_policy_hash = zero;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.da_layout.parity_shards = 0;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.da_layout.chunk_size_bytes = 3;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.da_layout.max_payload_size_bytes = wire::MAX_DA_PAYLOAD_SIZE_BYTES + 1;
        cases.push(changed);
        let mut changed = valid;
        changed.da_layout.max_chunk_count = 1;
        cases.push(changed);
        for changed in cases {
            assert!(changed.validate().is_err());
        }
    }

    #[test]
    fn frozen_lane_context_rejects_committee_geometry_order_and_native_pop_corruption() {
        let valid = fixture();
        let mut cases = Vec::new();
        let mut changed = valid.clone();
        changed.committee.pop();
        changed.validator_set_pops.pop();
        cases.push(changed);
        let mut changed = valid.clone();
        changed.committee.push(changed.committee[0].clone());
        changed
            .validator_set_pops
            .push(changed.validator_set_pops[0].clone());
        cases.push(changed);
        let mut changed = valid.clone();
        changed.committee = vec![changed.committee[0].clone(); wire::MAX_VALIDATORS_PER_HEIGHT + 1];
        cases.push(changed);
        let mut changed = valid.clone();
        changed.committee.swap(0, 1);
        changed.validator_set_pops.swap(0, 1);
        cases.push(changed);
        let mut changed = valid.clone();
        changed.committee[1] = changed.committee[0].clone();
        cases.push(changed);
        let mut changed = valid.clone();
        changed.validator_set_pops.pop();
        cases.push(changed);
        let mut changed = valid.clone();
        changed.validator_set_pops[0].clear();
        cases.push(changed);
        let mut changed = valid.clone();
        changed.validator_set_pops[0] = vec![1; wire::finality::MAX_VALIDATOR_POP_BYTES + 1];
        cases.push(changed);
        let mut changed = valid.clone();
        changed.validator_set_pops[0][0] ^= 1;
        cases.push(changed);
        let mut changed = valid;
        changed.validator_set_pops.swap(0, 1);
        cases.push(changed);
        for changed in cases {
            assert!(changed.validate().is_err());
        }
    }

    #[test]
    fn frozen_lane_context_accepts_native_committee_boundaries_and_binds_the_roster() {
        let original = fixture();
        let original_hash = original.canonical_hash().expect("original roster");
        for count in [7, wire::MAX_VALIDATORS_PER_HEIGHT] {
            let mut validators = (1..=count)
                .map(|seed| {
                    let key = KeyPair::try_from_seed(
                        vec![u8::try_from(seed).expect("bounded committee"); 32],
                        Algorithm::BlsNormal,
                    )
                    .expect("deterministic native BLS key");
                    let pop = iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("native BLS proof");
                    (PeerId::new(key.public_key().clone()), pop)
                })
                .collect::<Vec<_>>();
            validators.sort_by(|left, right| left.0.cmp(&right.0));
            let mut context = original.clone();
            context.committee = validators.iter().map(|(peer, _)| peer.clone()).collect();
            context.validator_set_pops = validators.into_iter().map(|(_, pop)| pop).collect();
            assert_eq!(
                context.minimum_signer_count(),
                Ok(2 * ((count - 1) / 3) + 1)
            );
            assert_ne!(
                context.canonical_hash().expect("changed valid roster"),
                original_hash
            );
        }
    }

    #[test]
    fn frozen_lane_context_hash_binds_every_mutable_semantic_field() {
        let valid = fixture();
        let expected = valid.canonical_hash().expect("original hash");
        let mut cases = Vec::new();
        let mut changed = valid.clone();
        changed.admitted_binding_hash = Hash::new(b"other admitted binding");
        cases.push(changed);
        let mut changed = valid.clone();
        changed.admission_priority.carrier_height -= 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.admission_priority.admission_index += 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::new(b"other-network"),
        ));
        cases.push(changed);
        let mut changed = valid.clone();
        changed.opening_global_height += 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.opening_global_context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
            Hash::new(b"other-opening-context"),
        ));
        cases.push(changed);
        let mut changed = valid.clone();
        changed.epoch += 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.mode = wire::ConsensusMode::Npos;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.lane_id = LaneId::new(2);
        cases.push(changed);
        let mut changed = valid.clone();
        changed.dataspace_id = DataSpaceId::new(3);
        cases.push(changed);
        let mut changed = valid.clone();
        changed.lane_incarnation = Hash::new(b"new-incarnation");
        cases.push(changed);
        let mut changed = valid.clone();
        changed.next_lane_height += 1;
        changed.predecessor_height += 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_hash = Some(Hash::new(b"other-predecessor"));
        cases.push(changed);
        let mut changed = valid.clone();
        changed.predecessor_applied_global_height -= 1;
        cases.push(changed);
        let mut changed = valid.clone();
        changed.nexus_amx_context_hash = Hash::new(b"other-nexus");
        cases.push(changed);
        let mut changed = valid.clone();
        changed.execution_policy_hash = Hash::new(b"other-policy");
        cases.push(changed);
        let mut changed = valid.clone();
        changed.da_layout.chunk_size_bytes /= 2;
        cases.push(changed);
        let mut changed = valid;
        changed.leader_seed[0] = 1;
        cases.push(changed);
        for changed in cases {
            assert_ne!(
                changed.canonical_hash().expect("valid altered context"),
                expected
            );
        }
    }

    #[test]
    fn frozen_lane_context_set_is_exact_bounded_and_never_silently_sorted() {
        let first = fixture();
        let mut second = first.clone();
        second.lane_id = LaneId::new(2);
        let set =
            LaneConsensusContextsV1::new(vec![first.clone(), second.clone()]).expect("ordered set");
        let bytes = norito::encode_canonical(&set).expect("set wire");
        assert_eq!(
            norito::decode_canonical::<LaneConsensusContextsV1>(&bytes).expect("decode set"),
            set
        );
        let json = norito::json::to_json(&set).expect("set JSON");
        assert_eq!(
            norito::json::from_str::<LaneConsensusContextsV1>(&json).expect("decode set JSON"),
            set
        );
        assert_eq!(
            LaneConsensusContextsV1::new(vec![second, first.clone()]),
            Err(LaneConsensusContextError::ContextOrder)
        );
        assert_eq!(
            LaneConsensusContextsV1::new(vec![first.clone(), first.clone()]),
            Err(LaneConsensusContextError::ContextOrder)
        );
        let mut other_incarnation = first.clone();
        other_incarnation.lane_incarnation = Hash::new(b"another-incarnation");
        let mut repeated_lane = vec![first.clone(), other_incarnation];
        repeated_lane.sort_by_key(FrozenLaneConsensusContextV1::route_key);
        assert_eq!(
            LaneConsensusContextsV1::new(repeated_lane),
            Err(LaneConsensusContextError::ContextOrder)
        );
        let mut other_network = first.clone();
        other_network.lane_id = LaneId::new(2);
        other_network.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::new(b"foreign-network"),
        ));
        assert_eq!(
            LaneConsensusContextsV1::new(vec![first.clone(), other_network]),
            Err(LaneConsensusContextError::MixedNetworks)
        );
        assert!(matches!(
            LaneConsensusContextsV1::new(vec![first.clone(); MAX_ACTIVE_EXECUTION_LANES + 1]),
            Err(LaneConsensusContextError::TooManyContexts(_))
        ));
        let empty = LaneConsensusContextsV1::default();
        assert_eq!(LaneConsensusContextsV1::new(Vec::new()), Ok(empty.clone()));
        assert_ne!(
            empty.canonical_hash().expect("empty commitment"),
            set.canonical_hash().expect("present commitment")
        );
        assert_ne!(
            LaneConsensusContextsV1::new(vec![first])
                .expect("one context")
                .canonical_hash()
                .expect("one commitment"),
            set.canonical_hash().expect("two contexts")
        );
        assert!(
            norito::json::from_str::<LaneConsensusContextsV1>("{}").is_err(),
            "restore must not default a missing context list"
        );
    }

    #[test]
    fn frozen_lane_test_projection_binds_the_exact_opening_subject_and_native_index_order() {
        use crate::sumeragi::v2_core as core;
        let frozen = fixture();
        let opening = subject();
        let context = frozen
            .core_context_for_test(opening)
            .expect("structural test projection");
        assert_eq!(context.height(), frozen.next_lane_height);
        assert_eq!(context.epoch(), frozen.epoch);
        assert_eq!(context.minimum_signer_count(), 3);
        assert_eq!(context.parent_commit(), None);
        assert!(!context.is_snapshot_bootstrap());
        assert_eq!(
            context
                .finalized_state_anchor()
                .expect("explicit external anchor")
                .height,
            frozen.opening_global_height
        );
        assert_eq!(
            context.roster()[0].id(),
            core::ValidatorId::new([0; Hash::LENGTH])
        );
        assert_eq!(context.leader(0), context.roster()[0].id());
        assert_eq!(context.leader(1), context.roster()[1].id());
        let mut changed = opening;
        changed.payload_hash = Hash::new(b"other-opening-payload");
        assert_ne!(
            context.id(),
            frozen
                .core_context_for_test(changed)
                .expect("changed subject projection")
                .id()
        );
        let mut changed = opening;
        changed.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"other-opening-block"));
        assert_ne!(
            context.id(),
            frozen
                .core_context_for_test(changed)
                .expect("changed block projection")
                .id()
        );
        let mut changed = frozen.clone();
        changed.predecessor_applied_global_height -= 1;
        assert_ne!(
            context.id(),
            changed
                .core_context_for_test(opening)
                .expect("changed predecessor carrier")
                .id()
        );
        let mut empty = frozen;
        empty.next_lane_height = 1;
        empty.predecessor_height = 0;
        empty.predecessor_hash = None;
        empty.predecessor_applied_global_height = 0;
        let empty_context = empty
            .core_context_for_test(opening)
            .expect("empty lane frontier still needs external finality");
        assert_eq!(
            empty_context
                .finalized_state_anchor()
                .expect("external anchor")
                .predecessor_subject,
            None
        );
    }
}
