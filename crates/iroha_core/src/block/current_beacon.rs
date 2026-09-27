/// A current pulse checked against one committed predecessor before acquiring writers.
struct PreparedCurrentBeaconPulse {
    header: BlockHeader,
    pulse: iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
    requirement: crate::sumeragi::beacon::BeaconRequirement,
}

impl PreparedCurrentBeaconPulse {
    fn apply(self, overlay: &mut StateBlock<'_>) -> Result<(), BlockValidationError> {
        overlay
            .apply_pristine_global_beacon_pulse(
                self.header,
                &self.requirement.session,
                self.pulse,
                self.requirement.parent,
            )
            .map_err(|error| {
                BlockValidationError::from_npos_application_error(
                    error,
                    "current global beacon pulse cannot be applied to its committed predecessor",
                )
            })
    }
}

impl ValidBlock {
    fn prepare_current_beacon_pulse(
        block: &SignedBlock,
        state: &State,
        mode: iroha_data_model::parameter::system::ConsensusMode,
    ) -> Result<Option<PreparedCurrentBeaconPulse>, BlockValidationError> {
        if block.header().global_beacon_pulse_hash() != block.global_beacon_pulse().map(HashOf::new)
        {
            return Err(Self::npos_effects_error(
                "current global beacon pulse commitment differs from payload",
            ));
        }
        if block.header().is_genesis() {
            return if block.global_beacon_pulse().is_none() {
                Ok(None)
            } else {
                Err(Self::npos_effects_error(
                    "genesis cannot carry a finalized global beacon pulse",
                ))
            };
        }
        let requirement = crate::sumeragi::beacon::current_requirement(
            state,
            block.header().height().get(),
            mode,
        )
        .map_err(|error| Self::npos_effects_error(error.to_string()))?;
        let (requirement, pulse) = match (requirement, block.global_beacon_pulse()) {
            (None, None) => return Ok(None),
            (Some(_), None) => {
                return Err(Self::npos_effects_error(
                    "block is missing a current global beacon pulse required by committed state",
                ));
            }
            (None, Some(_)) => {
                return Err(Self::npos_effects_error(
                    "current global beacon pulse was not requested by committed state",
                ));
            }
            (Some(requirement), Some(pulse)) => (requirement, *pulse),
        };
        if block.network_entrypoint_count() == 0 {
            return Err(Self::npos_effects_error(
                "a global beacon pulse cannot substitute transaction work",
            ));
        }
        if block.header().prev_block_hash() != Some(requirement.parent.block_hash) {
            return Err(Self::npos_effects_error(
                "current global beacon pulse has another committed predecessor",
            ));
        }
        // Native/merge carriers require a separate composed execution capability. They may not
        // smuggle an independently applied pulse past their certified write-set boundary.
        if block.execution_context().is_some_and(|context| {
            context.native_lane_decisions.is_some() || context.merge_entry.is_some()
        }) {
            return Err(Self::npos_effects_error(
                "current pulse requires ordinary transaction execution",
            ));
        }
        requirement
            .verify(&pulse)
            .map_err(|error| Self::npos_effects_error(error.to_string()))?;
        Ok(Some(PreparedCurrentBeaconPulse {
            header: block.header(),
            pulse,
            requirement,
        }))
    }
}

#[cfg(test)]
mod current_beacon_tests {
    use super::*;
    use crate::{
        beacon::{
            FinalizedGlobalThresholdBeaconKeySessionRecordV1,
            signed_pulses_fixture_for_roster_and_anchors,
        },
        state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, StateReadOnly, World, WorldReadOnly},
        sumeragi::{
            block_store::Staging,
            certified_chain::CertifiedChain,
            executor::{ExecutorContext, StateExecutor},
            network_topology::Topology,
            payload,
            test_chain::{CertifiedTestChain, Signers, TestChainConfig},
        },
    };
    use iroha_data_model::{
        consensus::FinalizedGlobalThresholdBeaconPulseV1,
        governance::types::{BeaconSessionId, GovernanceAttemptId},
        parameter::system::ConsensusMode,
    };
    use iroha_sumeragi::crypto::NoAttestation;
    use mv::storage::StorageReadOnly as _;
    use std::{collections::BTreeSet, sync::Arc};

    fn predecessor() -> CertifiedTestChain {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000)).unwrap();
        for height in 2..=4 {
            chain.commit_at(height * 10_000, Vec::new());
        }
        chain
    }

    fn install_request(state: &State, record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1) {
        let mut world = state.world.block();
        world
            .global_beacon_key_sessions
            .insert(record.session.session_id, record.clone());
        world.global_beacon_active_session.insert(
            GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            record.session.session_id,
        );
        // Seed the committed Parliament request index; pulse admission does not invent or
        // mutate governance authority. Production governance owns this index.
        world.parliament_required_beacon_pulse_slots.insert(
            (
                BeaconSessionId::for_network_v1(&record.session.network_id),
                5,
            ),
            BTreeSet::from([GovernanceAttemptId::new([0x71; 32])]),
        );
        world.commit();
    }

    fn fixture() -> (
        CertifiedTestChain,
        FinalizedGlobalThresholdBeaconKeySessionRecordV1,
        FinalizedGlobalThresholdBeaconPulseV1,
    ) {
        let chain = predecessor();
        let mut keys = [0xC1, 0xC2, 0xC3, 0xC4].map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal).unwrap()
        });
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let parent = iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1 {
            height: 4,
            block_hash: chain.committed(4).block_hash(),
        };
        let (record, pulses) =
            signed_pulses_fixture_for_roster_and_anchors(chain.network_id(), &keys, &[parent]);
        install_request(chain.state(), &record);
        (chain, record, pulses[0])
    }

    fn validate(
        chain: &CertifiedTestChain,
        block: SignedBlock,
    ) -> Result<(), BlockValidationError> {
        let view = chain.state().view();
        let scheduled = view.world().consensus_schedule().get(5).unwrap().clone();
        drop(view);
        ValidBlock::validate_sumeragi_block(
            block,
            &Topology::new(scheduled.committee),
            chain.genesis_account(),
            Duration::from_millis(scheduled.params.block_time_ms),
            ConsensusMode::Permissioned,
            chain.state(),
        )
        .unpack(|_| {})
        .map(|(_, overlay)| drop(overlay))
        .map_err(|(_, error)| *error)
    }

    #[test]
    fn current_threshold_pulse_commits_real_work_and_replays_exact_result() {
        let (mut chain, record, pulse) = fixture();
        let replay = predecessor();
        assert_eq!(
            chain.committed(4).block_hash(),
            replay.committed(4).block_hash()
        );
        install_request(replay.state(), &record);
        chain.commit_with_pulse(Some(50_000), Vec::new(), Signers::Quorum, Some(pulse));
        let stored = chain.committed(5);
        assert!(stored.block().network_entrypoint_count() > 0);
        assert_eq!(stored.block().global_beacon_pulse(), Some(&pulse));
        assert!(stored.block().npos_consensus_effects().is_none());
        let view = chain.state().view();
        assert_eq!(
            view.world().global_beacon_pulses().get(&pulse.pulse_id),
            Some(&pulse)
        );
        let certified = CertifiedChain::new(&view)
            .unwrap()
            .with_attestation_verifier(&NoAttestation)
            .certified(5)
            .unwrap();
        let qc = certified.commit_qc().unwrap().clone();
        drop(view);
        let proposal = stored.block().canonical_resultless_proposal();
        validate(&replay, proposal.clone())
            .expect("same real current block is admitted on its exact predecessor");
        assert!(
            replay
                .state()
                .view()
                .world()
                .global_beacon_pulses()
                .is_empty(),
            "discarded execution cannot publish a pulse"
        );
        let core_block = iroha_sumeragi::message::Block {
            header: stored.header().unwrap().clone(),
            payload: payload::encode(&proposal).unwrap(),
        };
        replay.kura().store_block(stored.block().clone()).unwrap();
        let mut restarted = StateExecutor::spawn(ExecutorContext {
            state: Arc::clone(replay.state()),
            queue: None,
            staging: Staging::new(),
            events: tokio::sync::broadcast::channel(16).0,
            genesis_account: replay.genesis_account().clone(),
            consensus_mode: ConsensusMode::Permissioned,
            applied: (4, replay.committed(4).core_hash()),
            crypto: None,
        })
        .unwrap();
        restarted
            .replay(&core_block, &qc)
            .expect("cold executor replays exact certified pulse writes");
        assert_eq!(replay.state().view().height(), 5);
        assert_eq!(
            replay
                .state()
                .view()
                .world()
                .global_beacon_pulses()
                .get(&pulse.pulse_id),
            Some(&pulse)
        );
        assert_eq!(
            replay
                .state()
                .view()
                .world()
                .global_beacon_pulse_slots()
                .len(),
            1
        );
        assert!(
            ValidBlock::prepare_current_beacon_pulse(
                &proposal,
                replay.state(),
                ConsensusMode::Permissioned
            )
            .is_err(),
            "committed pulse cannot be replayed at its old height"
        );
    }

    #[test]
    fn current_threshold_pulse_rejects_missing_unrequested_tampered_and_empty_carriers() {
        let (mut source, record, pulse) = fixture();
        source.commit_with_pulse(Some(50_000), Vec::new(), Signers::Quorum, Some(pulse));
        let proposal = source.committed(5).block().canonical_resultless_proposal();
        let predecessor = predecessor();
        install_request(predecessor.state(), &record);
        validate(&predecessor, proposal.clone()).unwrap();
        let mut missing = proposal.clone();
        missing.set_global_beacon_pulse(None);
        assert!(validate(&predecessor, missing).is_err());
        let mut empty = proposal.clone();
        empty.set_external_entrypoints(Vec::new());
        empty.set_execution_context(None);
        assert!(payload::encode(&empty).is_err());
        assert!(
            ValidBlock::prepare_current_beacon_pulse(
                &empty,
                predecessor.state(),
                ConsensusMode::Permissioned
            )
            .is_err()
        );
        for field in 0..10 {
            let mut wrong = pulse;
            match field {
                0 => wrong.signature[0] ^= 1,
                1 => wrong.session_id[0] ^= 1,
                2 => wrong.roster_hash[0] ^= 1,
                3 => wrong.transcript_hash[0] ^= 1,
                4 => wrong.height += 1,
                5 => wrong.round += 1,
                6 => {
                    wrong.finalized_chain_anchor.block_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"foreign parent"))
                }
                7 => wrong.seed[0] ^= 1,
                8 => wrong.pulse_id[0] ^= 1,
                9 => {
                    wrong.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                        Hash::new(b"foreign genesis"),
                    ))
                }
                _ => unreachable!(),
            }
            let mut candidate = proposal.clone();
            candidate.set_global_beacon_pulse(Some(wrong));
            assert!(
                validate(&predecessor, candidate).is_err(),
                "tampered field {field}"
            );
        }
        let mut world = predecessor.state().world.block();
        world
            .parliament_required_beacon_pulse_slots
            .remove(&(BeaconSessionId::for_network_v1(&pulse.network_id), 5));
        world.commit();
        assert!(
            validate(&predecessor, proposal).is_err(),
            "a valid signature alone does not request a pulse"
        );
        assert_eq!(predecessor.state().view().height(), 4);
        assert!(
            predecessor
                .state()
                .view()
                .world()
                .global_beacon_pulses()
                .is_empty()
        );
    }
}
