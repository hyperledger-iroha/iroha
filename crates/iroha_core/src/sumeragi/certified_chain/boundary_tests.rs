//! Historical certification transcripts with genuine BLS votes and threshold pulses.
//! These fixtures do not execute NPoS State transitions. Application attestations below use a
//! test-only BLS verifier, not the production paired-Pasta signing/qualification path.

use super::*;
use crate::state::WorldReadOnly as _;
use crate::sumeragi::{
    commitment::execution_commitment,
    payload::{self, Assembly},
    schedule::{ChainParamsRecord, ScheduleOutcome, ScheduledConfig, ScheduledSlot},
};
use iroha_data_model::{
    NetworkId,
    block::{
        consensus::ExecWitness,
        consensus_v2::ValidatorPower,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
    },
    consensus::{GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconPulseContextV1},
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, KagemushaMintFinalityEpochDecisionV1,
    },
    parameter::system::{ConsensusMode, SumeragiNposParameters},
    sumeragi::epoch::{ValidatorCommitteeMemberV1, ValidatorEpochBoundaryV1},
    transaction::signed::TransactionResult,
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    crypto::{Signer as _, form_qc},
    message::Vote,
    types::{ChainParams, PublicKey},
};
use std::{num::NonZeroU64, sync::OnceLock, time::Duration};

fn keys(rotated: bool) -> Vec<KeyPair> {
    let start = if rotated { 0xD1u8 } else { 0xC1u8 };
    let mut keys = (start..=start + 3)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| core_key(key.public_key()).unwrap());
    keys
}
fn members(keys: &[KeyPair]) -> Vec<ValidatorCommitteeMemberV1> {
    keys.iter()
        .map(|key| ValidatorCommitteeMemberV1 {
            validator: PeerId::new(key.public_key().clone()),
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
        })
        .collect()
}

/// Explicit test attestation authority. It checks actual signatures over the entire epoch-bound
/// core statement; accepting these signatures does not qualify production Pasta attestations.
struct TestAttestations;
impl AttestationVerifier for TestAttestations {
    fn verify(
        &self,
        _height: u64,
        _signer: u32,
        key: &PublicKey,
        statement: &[u8],
        witness: &iroha_sumeragi::message::ResultWitness,
        attestation: &[u8],
    ) -> bool {
        let Ok(key) = crate::sumeragi::crypto::iroha_key(key) else {
            return false;
        };
        witness.as_slice() == statement
            && iroha_crypto::Signature::from_bytes(attestation)
                .verify(&key, statement)
                .is_ok()
    }
}
fn certificate(keys: &[KeyPair], header: &BlockHeader, result: Hash32, last: bool) -> Qc {
    let crypto = BlsCrypto::new();
    crypto
        .admit_committee(
            keys.iter()
                .map(|key| {
                    (
                        key.public_key(),
                        iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
                .iter()
                .map(|(key, pop)| (*key, pop.as_slice())),
        )
        .unwrap();
    let indices = if last { [1usize, 2, 3] } else { [0usize, 1, 2] };
    let votes = indices
        .into_iter()
        .map(|index| {
            let mut vote = Vote {
                kind: VoteKind::Commit,
                instance: header.instance,
                epoch: header.epoch,
                height: header.height,
                view: 0,
                block_hash: header.hash(&crypto),
                result,
                attest: header.attest,
                signer: index as u32,
                sig: iroha_sumeragi::types::Signature([0; iroha_sumeragi::types::SIGNATURE_LEN]),
                attestation: None,
            };
            if header.attest {
                vote.attestation = Some(iroha_sumeragi::message::CommitAttestation {
                    witness: iroha_sumeragi::message::ResultWitness::from_untrusted(
                        vote.statement(),
                    )
                    .unwrap(),
                    signature: iroha_sumeragi::message::AttestationSignature::try_from_slice(
                        iroha_crypto::Signature::new(keys[index].private_key(), &vote.statement())
                            .payload(),
                    )
                    .unwrap(),
                });
            }
            vote.sig = crate::sumeragi::crypto::KeyPairSigner::new(&keys[index])
                .unwrap()
                .sign(&vote.preimage());
            vote
        })
        .collect::<Vec<_>>();
    form_qc(&crypto, keys.len(), &votes.iter().collect::<Vec<_>>()).unwrap()
}
fn install_structural_outputs(block: &mut SignedBlock) {
    let outputs = (0..block.external_transactions().count())
        .map(|index| {
            ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: index as u32,
                result: TransactionResult::new(Ok(Vec::new())),
                completions: Vec::new(),
            })
        })
        .collect();
    block
        .set_execution_outputs(
            outputs,
            0,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .unwrap();
}
fn params() -> ChainParamsRecord {
    ChainParamsRecord::from_core(&ChainParams {
        epoch_length: 6,
        ..ChainParams::default()
    })
}
fn slot(height: u64, current: &ValidatorEpochContextV1) -> ScheduledSlot {
    if height <= current.authorization.last_height {
        ScheduledSlot::Ready(ScheduledConfig {
            height,
            epoch: current.clone(),
            params: params(),
        })
    } else {
        ScheduledSlot::PendingBoundary {
            height,
            boundary_height: current.authorization.last_height,
            predecessor_context_id: current.context_id().unwrap(),
            params: params(),
        }
    }
}
fn outcome(
    height: u64,
    current: &ValidatorEpochContextV1,
    boundary: Option<ValidatorEpochBoundaryV1>,
) -> ScheduleOutcome {
    let authorized = boundary.as_ref().map_or(current, |boundary| &boundary.next);
    let next = slot(height + 1, authorized);
    let after_next = slot(height + 2, authorized);
    ScheduleOutcome {
        height,
        current: current.clone(),
        boundary,
        next,
        after_next,
    }
}
fn build_history(retain: bool) -> Vec<Arc<SignedBlock>> {
    let original_keys = keys(false);
    let genesis_key = KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519);
    let mut policy = SumeragiNposParameters::default();
    policy.epoch_seed = [0x35; 32];
    policy.epoch_length_blocks = NonZeroU64::new(6).unwrap();
    policy.evidence_horizon_blocks = 6;
    policy.slashing_delay_blocks = 6;
    policy.max_validators = 4;
    let validators = members(&original_keys)
        .into_iter()
        .map(|member| (member.validator, member.proof_of_possession))
        .collect::<Vec<_>>();
    let mut genesis = crate::sumeragi::test_chain::signed_genesis_fixture(
        &"sumeragi-certified-test-chain".parse().unwrap(),
        &genesis_key,
        &validators,
        Vec::new(),
        1000,
        ConsensusMode::Npos,
        Some(policy),
    )
    .unwrap();
    let mut current = crate::sumeragi::epoch::genesis_epoch(&genesis).unwrap();
    assert_eq!(current.authorization.last_height, 6);
    let network = NetworkId::from_genesis_hash(genesis.hash());
    let instance = global_instance(&genesis, "sumeragi-certified-test-chain");
    let world = State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        network,
    );
    let witness = complete_lane_state_witness(network, 1);
    install_structural_outputs(&mut genesis);
    let result = ExecutionResultCommitment::new(
        1,
        execution_commitment(&witness, &genesis).unwrap(),
        outcome(1, &current, None),
        None,
        iroha_data_model::sumeragi_finality::NativeLaneStateProof::from_witness(
            &witness,
            &mv::allocation::AllocationBudget::new(64 * 1024),
        )
        .unwrap(),
    )
    .unwrap();
    genesis = genesis.with_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        Vec::new(),
        Vec::new(),
        result.preimage().unwrap(),
    )));
    let mut history = vec![Arc::new(genesis)];
    let mut active_keys = original_keys;
    let mut active_beacon = crate::beacon::tests::HistoricalBeaconFixture::new(
        network,
        [0x61; 32],
        current.authority.generation,
        &active_keys,
    );
    for height in 2..=14 {
        let parent = read_frame(history.last().unwrap().clone(), height - 1).unwrap();
        let block_time = parent.block().header().creation_time() + Duration::from_millis(1);
        let (_, time_source) = iroha_primitives::time::TimeSource::new_mock(block_time);
        let mut transaction = iroha_data_model::transaction::TransactionBuilder::new(
            network,
            iroha_data_model::account::AccountId::new(genesis_key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, format!("certified history {height}"))]);
        transaction.set_creation_time(parent.block().header().creation_time());
        let transaction = crate::tx::AcceptedTransaction::accept_with_time_source(
            transaction.sign(genesis_key.private_key()),
            &network,
            Duration::from_secs(1),
            world.view().world().parameters().transaction(),
            &iroha_config::parameters::actual::Crypto::default(),
            &time_source,
        )
        .unwrap();
        let mut block = payload::assemble(
            &world,
            Assembly {
                parent: parent.block(),
                view: 0,
                cadence: Duration::from_millis(1),
            },
            &[transaction],
        )
        .unwrap();
        let beacon = if height + 1 == current.authorization.last_height {
            Some(active_beacon.pulse(
                GlobalThresholdBeaconChainAnchorV1 {
                    height: height - 1,
                    block_hash: parent.block_hash(),
                },
                GlobalThresholdBeaconPulseContextV1 {
                    instance: instance.0,
                    epoch: current.authorization.epoch,
                    epoch_context_id: current.context_id().unwrap(),
                    parent_consensus_hash: parent.core_hash().0,
                    parent_result: parent.result().0,
                },
            ))
        } else {
            None
        };
        let mut successor = None;
        let boundary = if height == current.authorization.last_height {
            let next_keys = if retain {
                keys(false)
            } else {
                keys(height == 6)
            };
            let authority = if retain {
                current.authority.clone()
            } else {
                let roster = members(&next_keys)
                    .into_iter()
                    .map(|member| ValidatorPower {
                        validator: member.validator,
                        power: 1,
                    })
                    .collect::<Vec<_>>();
                crate::kagemusha_v1_test_fixtures::mint_finality_authority(
                    network,
                    current.authority.generation + 1,
                    &roster,
                )
            };
            let next_beacon = (!retain).then(|| {
                crate::beacon::tests::HistoricalBeaconFixture::new(
                    network,
                    [0x62 + (height / 12) as u8; 32],
                    authority.generation,
                    &next_keys,
                )
            });
            let record = next_beacon.as_ref().unwrap_or(&active_beacon).record();
            assert_eq!(
                record.adaptive_dkg.session.authority_generation,
                authority.generation
            );
            let authorization =
                crate::kagemusha_v1_test_fixtures::mint_finality_successor_authorization(
                    &current.authorization,
                    &authority,
                    height + 6,
                    BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                        session_id: record.session_id,
                        transcript_hash: record.transcript_hash,
                    }),
                    if retain {
                        KagemushaMintFinalityEpochDecisionV1::Retain
                    } else {
                        KagemushaMintFinalityEpochDecisionV1::Activate
                    },
                    if retain { [0; 32] } else { [height as u8; 32] },
                );
            let next = ValidatorEpochContextV1 {
                version: 1,
                network_id: network,
                mode: ConsensusMode::Npos,
                authority,
                authorization,
                committee: members(&next_keys),
                leader_seed: crate::beacon::global_threshold_beacon_npos_successor_seed_v1(
                    parent.commitment().beacon.as_ref().unwrap(),
                    height,
                    authorization.epoch,
                ),
            };
            next.validate_successor(&current).unwrap();
            successor = Some((next.clone(), next_keys, next_beacon));
            Some(ValidatorEpochBoundaryV1 {
                version: 1,
                height,
                predecessor_context_id: current.context_id().unwrap(),
                selection_anchor: parent.block_hash(),
                next,
                preparation: None,
            })
        } else {
            None
        };
        assert!(
            block.global_beacon_pulse().is_none(),
            "native header is the sole control owner"
        );
        install_structural_outputs(&mut block);
        let payload = payload::encode(&block).unwrap();
        let header = BlockHeader {
            control_witness: crate::sumeragi::epoch_beacon::control::encode(beacon).unwrap(),
            instance,
            epoch: schedule::core_epoch(&current).unwrap().id,
            height,
            origin_view: 0,
            parent_hash: parent.core_hash(),
            parent_result: parent.result(),
            payload_hash: payload_hash(&BlsCrypto::new(), &payload),
            payload_len: u32::try_from(payload.len()).unwrap(),
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: boundary.is_some(),
        };
        let witness = complete_lane_state_witness(network, height);
        let commitment = ExecutionResultCommitment::new(
            height,
            execution_commitment(&witness, &block).unwrap(),
            outcome(height, &current, boundary),
            beacon,
            iroha_data_model::sumeragi_finality::NativeLaneStateProof::from_witness(
                &witness,
                &mv::allocation::AllocationBudget::new(64 * 1024),
            )
            .unwrap(),
        )
        .unwrap();
        let preimage = commitment.preimage().unwrap();
        let qc = certificate(&active_keys, &header, result_of_preimage(&preimage), false);
        block = block
            .with_commit_certificate(Some(commit_certificate(&header, &qc, preimage).unwrap()));
        history.push(Arc::new(block));
        if let Some((next, keys, beacon)) = successor {
            current = next;
            active_keys = keys;
            if let Some(beacon) = beacon {
                active_beacon = beacon;
            }
        }
    }
    history
}
fn history() -> Vec<Arc<SignedBlock>> {
    static HISTORY: OnceLock<Vec<Arc<SignedBlock>>> = OnceLock::new();
    HISTORY.get_or_init(|| build_history(false)).clone()
}
fn retained_history() -> Vec<Arc<SignedBlock>> {
    static HISTORY: OnceLock<Vec<Arc<SignedBlock>>> = OnceLock::new();
    HISTORY.get_or_init(|| build_history(true)).clone()
}

#[test]
fn rotated_away_committee_verifies_from_authenticated_boundaries_with_bounded_authority() {
    let history = history();
    let state = state_with_history(&history);
    let view = state.view();
    let reader = CertifiedChain::new(&view)
        .unwrap()
        .with_attestation_verifier(&TestAttestations);
    assert_eq!(
        reader
            .walk(1, 14)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        14
    );
    {
        let cursor = reader.prefix.lock();
        let prefix = cursor.as_ref().unwrap();
        assert_eq!(prefix.tip.height(), 14);
        assert_eq!(prefix.authority.material.authority.generation, 2);
        assert!(prefix.schedule.entries().len() <= 3);
    }
    let original = reader.certified(7).unwrap();
    let other = with_parts(&history[6], |header, qc, _| {
        *qc = certificate(&keys(true), header, qc.result, true)
    });
    let other = reader
        .check_certificate(read_frame(other, 7).unwrap())
        .unwrap();
    assert_ne!(
        original.commit_qc().unwrap().signers,
        other.commit_qc().unwrap().signers
    );
    assert_eq!(original.id(), other.id());
    assert_eq!(original.commitment(), other.commitment());
    assert_eq!(
        reader.certified(14).unwrap().verification(),
        QcVerification::Verified
    );
}

#[test]
fn retained_generation_still_binds_new_epoch_and_fresh_leader_randomness() {
    let history = retained_history();
    let state = state_with_history(&history);
    let view = state.view();
    let reader = CertifiedChain::new(&view)
        .unwrap()
        .with_attestation_verifier(&TestAttestations);
    let before = reader.certified(6).unwrap();
    let after = reader.certified(7).unwrap();
    assert_eq!(
        before.commitment().schedule.current.authority,
        after.commitment().schedule.current.authority
    );
    assert_eq!(
        before.commitment().schedule.current.committee,
        after.commitment().schedule.current.committee
    );
    assert_ne!(
        before.header().unwrap().epoch,
        after.header().unwrap().epoch
    );
    assert_ne!(
        before.commitment().schedule.current.leader_seed,
        after.commitment().schedule.current.leader_seed
    );
    let old_epoch = with_parts(&history[6], |header, qc, _| {
        header.epoch = before.header().unwrap().epoch;
        *qc = certificate(&keys(false), header, qc.result, false);
    });
    assert!(matches!(
        read_frame(old_epoch, 7).and_then(|block| reader.check_certificate(block)),
        Err(ChainReadError::HeaderMismatch { height: 7 })
    ));
}

#[test]
fn historical_authority_missing_reordered_or_forged_proofs_fail_closed() {
    let history = history();
    for mutation in 0..5 {
        let mut corrupt = history.clone();
        corrupt[5] = with_parts(&corrupt[5], |_, _, bytes| {
            let mut result = ExecutionResultCommitment::decode(bytes).unwrap();
            let next = &mut result.schedule.boundary.as_mut().unwrap().next;
            match mutation {
                0 => {
                    next.committee.pop();
                }
                1 => next.committee.swap(0, 1),
                2 => next.committee[0].proof_of_possession.clear(),
                3 => next.committee[0].proof_of_possession[0] ^= 1,
                _ => next.authority.validators[0].ep_proof_public_key = [0xff; 32],
            }
            *bytes = norito::encode_canonical(&result).unwrap();
        });
        let state = state_with_history(&corrupt);
        let view = state.view();
        assert!(matches!(
            CertifiedChain::new(&view)
                .unwrap()
                .with_attestation_verifier(&TestAttestations)
                .certified(14),
            Err(ChainReadError::Malformed { height: 6, .. })
        ));
    }
    let mut missing = history;
    missing[5] = Arc::new(missing[5].as_ref().clone().with_commit_certificate(None));
    let state = state_with_history(&missing);
    let view = state.view();
    assert!(matches!(
        CertifiedChain::new(&view)
            .unwrap()
            .with_attestation_verifier(&TestAttestations)
            .certified(14),
        Err(ChainReadError::MissingCertificate { height: 6 })
    ));
}

#[test]
fn boundary_authority_and_parent_links_cannot_self_authorize() {
    let original = history();
    for kind in 0..3 {
        let mut history = original.clone();
        let index = if kind == 0 { 5 } else { 6 };
        history[index] = with_parts(&history[index], |header, qc, _| {
            if kind == 1 {
                header.parent_result = Hash32([0x77; 32]);
            }
            *qc = certificate(&keys(kind != 2), header, qc.result, false);
        });
        let state = state_with_history(&history);
        let view = state.view();
        let result = CertifiedChain::new(&view)
            .unwrap()
            .with_attestation_verifier(&TestAttestations)
            .certified(14);
        if kind == 1 {
            assert!(matches!(
                result,
                Err(ChainReadError::Discontinuous { height: 7 })
            ));
        } else {
            assert!(matches!(result, Err(ChainReadError::Certificate { .. })));
        }
    }
}

#[test]
fn nonempty_boundary_requires_flagged_attestation_and_exact_fresh_pulse() {
    let original = history();
    let state = state_with_history(&original);
    let view = state.view();
    let reader = CertifiedChain::new(&view)
        .unwrap()
        .with_attestation_verifier(&TestAttestations);
    let boundary = reader.certified(6).unwrap();
    assert!(boundary.header().unwrap().payload_len > 0);
    assert!(boundary.header().unwrap().attest);
    for mutation in 0..4 {
        let mut history = original.clone();
        history[5] = with_parts(&history[5], |header, qc, bytes| {
            if mutation == 0 {
                header.attest = false;
            } else if mutation < 3 {
                let mut result = ExecutionResultCommitment::decode(bytes).unwrap();
                let boundary = result.schedule.boundary.as_mut().unwrap();
                if mutation == 1 {
                    boundary.selection_anchor =
                        HashOf::from_untyped_unchecked(Hash::new(b"foreign cut"));
                } else {
                    boundary.next.leader_seed[0] ^= 1;
                    let changed = boundary.next.clone();
                    if let ScheduledSlot::Ready(next) = &mut result.schedule.next {
                        next.epoch = changed.clone();
                    }
                    if let ScheduledSlot::Ready(next) = &mut result.schedule.after_next {
                        next.epoch = changed;
                    }
                }
                *bytes = result.preimage().unwrap();
            }
            *qc = certificate(&keys(false), header, result_of_preimage(bytes), false);
            if mutation == 3 {
                let mut changed = qc.attestations[0].as_slice().to_vec();
                changed[0] ^= 1;
                qc.attestations[0] =
                    iroha_sumeragi::message::AttestationSignature::try_from_slice(&changed)
                        .unwrap();
            }
        });
        let state = state_with_history(&history);
        let view = state.view();
        let result = CertifiedChain::new(&view)
            .unwrap()
            .with_attestation_verifier(&TestAttestations)
            .certified(6);
        assert!(result.is_err(), "mutation {mutation}");
    }
}

#[test]
fn unsigned_genesis_result_cannot_substitute_the_signed_epoch_root() {
    let mut history = history();
    let original = history[0].as_ref();
    let certificate = original.commit_certificate().unwrap();
    let mut result = ExecutionResultCommitment::decode(certificate.result_preimage()).unwrap();
    let mut changed = result.schedule.current.clone();
    changed.leader_seed[0] ^= 1;
    result.schedule = outcome(1, &changed, None);
    let certificate = CommitCertificate::from_untrusted_parts(
        certificate.consensus_header().to_vec(),
        certificate.commit_qc().to_vec(),
        result.preimage().unwrap(),
    );
    history[0] = Arc::new(original.clone().with_commit_certificate(Some(certificate)));
    let state = state_with_history(&history);
    let view = state.view();
    assert!(
        committed_block(&view, 1).is_ok(),
        "local deterministic State trust is separate"
    );
    assert!(matches!(
        CertifiedChain::new(&view)
            .unwrap()
            .with_attestation_verifier(&TestAttestations)
            .certified(1),
        Err(ChainReadError::Committee { height: 1, .. })
    ));
}

#[test]
fn result_pulses_require_exact_height_network_session_and_parent_bindings() {
    let history = history();
    for mutation in 0..7 {
        let original = &history[if mutation == 6 { 10 } else { 4 }];
        let certificate = original.commit_certificate().unwrap();
        let mut result = ExecutionResultCommitment::decode(certificate.result_preimage()).unwrap();
        if mutation == 0 {
            result.beacon = None;
        } else {
            let pulse = result.beacon.as_mut().unwrap();
            match mutation {
                1 => pulse.height += 1,
                2 => {
                    pulse.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                        Hash::new(b"foreign pulse"),
                    ))
                }
                3 => pulse.finalized_chain_anchor.height -= 1,
                4 => pulse.seed[0] ^= 1,
                5 => pulse.round += 1,
                _ => {
                    pulse.session_id[0] ^= 1;
                    pulse.pulse_id =
                        crate::beacon::global_threshold_beacon_pulse_id_v1(pulse, pulse.seed);
                }
            }
        }
        assert!(
            ExecutionResultCommitment::decode(&norito::encode_canonical(&result).unwrap()).is_err(),
            "mutation {mutation}"
        );
    }
    // A well-shaped pulse for a different parent still cannot authenticate this stored frame.
    let bad = with_parts(&history[4], |header, qc, bytes| {
        let mut result = ExecutionResultCommitment::decode(bytes).unwrap();
        let pulse = result.beacon.as_mut().unwrap();
        pulse.finalized_chain_anchor.block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"other parent"));
        pulse.pulse_id = crate::beacon::global_threshold_beacon_pulse_id_v1(pulse, pulse.seed);
        *bytes = result.preimage().unwrap();
        *qc = certificate(&keys(false), header, result_of_preimage(bytes), false);
    });
    assert!(matches!(
        read_frame(bad, 5),
        Err(ChainReadError::Malformed { height: 5, .. })
    ));
}

#[test]
fn pinned_restoration_reuses_full_boundary_verification_and_one_authority_cursor() {
    let history = history();
    let kura = Kura::blank_kura_for_testing();
    for block in &history {
        kura.store_block(Arc::clone(block)).unwrap();
    }
    let chain_id = ChainId::from("sumeragi-certified-test-chain");
    let network = NetworkId::from_genesis_hash(history[0].hash());
    let hashes = history.iter().map(|block| block.hash()).collect::<Vec<_>>();
    let reader = CertifiedChain::from_pinned(&chain_id, &network, &hashes, &kura)
        .unwrap()
        .with_attestation_verifier(&TestAttestations);
    for (index, receipt) in reader.walk(1, 14).enumerate() {
        let receipt = receipt.unwrap();
        assert_eq!(receipt.block_hash(), hashes[index]);
        assert_eq!(
            receipt.verification(),
            if index == 0 {
                QcVerification::Genesis
            } else {
                QcVerification::Verified
            }
        );
    }
    {
        let cursor = reader.prefix.lock();
        let prefix = cursor.as_ref().unwrap();
        assert_eq!(prefix.tip.height(), 14);
        assert_eq!(prefix.authority.material.authority.generation, 2);
        assert!(prefix.schedule.entries().len() <= 3);
    }
    // Old random reads replay the authenticated prefix; no rotated-away roster cache exists.
    assert_eq!(
        reader
            .certified(7)
            .unwrap()
            .commitment()
            .schedule
            .current
            .authority
            .generation,
        1
    );
    assert_eq!(
        reader
            .certified(14)
            .unwrap()
            .commitment()
            .schedule
            .current
            .authority
            .generation,
        2
    );
}

#[test]
fn historical_pulse_requires_signed_header_bytes_and_complete_native_context() {
    let history = history();
    let original = &history[4]; // genuine threshold pulse immediately before the first boundary
    let valid = read_frame(original.clone(), 5).unwrap();
    assert!(!valid.header().unwrap().control_witness.is_empty());
    for mutation in 0..3 {
        let corrupt = with_parts(original, |header, qc, bytes| {
            let mut result = ExecutionResultCommitment::decode(bytes).unwrap();
            if mutation == 0 {
                header.control_witness = iroha_sumeragi::types::ControlWitness::empty();
            } else {
                let prior = result.beacon.unwrap();
                let mut context = prior.context;
                if mutation == 1 {
                    context.parent_result[0] ^= 1;
                } else {
                    context.parent_consensus_hash[0] ^= 1;
                }
                let (_, pulses) = crate::beacon::tests::finalized_pulses_fixture_for_context_v1(
                    prior.network_id,
                    prior.session_id,
                    &keys(false),
                    &[(prior.finalized_chain_anchor, context)],
                );
                result.beacon = Some(pulses[0]);
                header.control_witness =
                    crate::sumeragi::epoch_beacon::control::encode(result.beacon).unwrap();
                *bytes = result.preimage().unwrap();
            }
            *qc = certificate(&keys(false), header, result_of_preimage(bytes), false);
        });
        assert!(
            read_frame(corrupt, 5).is_err(),
            "genuine threshold and QC signatures cannot rebind a pulse to another source"
        );
    }
}

/// Boundary-verifier fixture: a canonical complete lane-state write, not an execution corpus.
fn complete_lane_state_witness(network: NetworkId, height: u64) -> ExecWitness {
    let lanes = iroha_data_model::sumeragi_lanes::SumeragiLaneState::default();
    let commitment = iroha_data_model::sumeragi_finality::SumeragiLaneStateCommitment::from_state(
        network, height, &lanes,
    )
    .unwrap();
    ExecWitness {
        writes: vec![iroha_data_model::block::consensus::ExecKv {
            key: iroha_data_model::sumeragi_finality::SUMERAGI_LANE_STATE_WITNESS_KEY.to_vec(),
            value: norito::encode_canonical(&commitment).unwrap(),
        }],
        ..ExecWitness::default()
    }
}
