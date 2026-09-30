//! Native committee evidence with exact BLS quorum, paired-Pasta boundary seals and real DKG.
//! The constructed transcript tests offline proof admission; it does not execute NPoS custody
//! transitions or qualify a network. Genesis is actually signed; no height-one QC is fabricated.
//! Every successor binds its real resultless payload and original proposer-signed RS16 availability.

use super::*;
use crate::{
    state::{WorldReadOnly as _, threshold_key_lifecycle_certificate_preimage_v1},
    sumeragi::{
        attestation::{NativePastaVerifier, encode_native_seal, native_seal_message},
        commitment::{ExecutionResultCommitment, execution_commitment, result_of_preimage},
        crypto::{BlsCrypto, KeyPairSigner, core_key},
        payload::{self, Assembly},
        schedule::{self, ChainParamsRecord, ScheduleOutcome, ScheduledConfig, ScheduledSlot},
    },
    zk::kagemusha_v1_recursion::KagemushaMintFinalitySignerV1,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    block::{
        CommitCertificate, SignedBlock,
        consensus::ExecWitness,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
    },
    consensus::{GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconPulseContextV1},
    isi::consensus_keys::ThresholdKeyLifecycleSignatureV1,
    nexus::ValidatorCommitteeSelectionStatusV1,
    parameter::system::{ConsensusMode, SumeragiNposParameters},
    sumeragi::{
        epoch::{ValidatorEpochBoundaryV1, ValidatorEpochContextV1},
        finality::NativeFinalityArtifact,
    },
    transaction::signed::TransactionResult,
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    availability::{PayloadAuthoring, PayloadBytes},
    crypto::{Signer as _, form_qc},
    message::{BlockHeader, CommitAttestation, ResultWitness, Vote, VoteKind},
    preimage::payload_hash,
    types::{ChainParams, Hash32},
};
use mv::storage::StorageReadOnly as _;
use std::{num::NonZeroU64, sync::Arc, time::Duration};

fn chain_id() -> ChainId {
    ChainId::from("native-committee-evidence-tests")
}
fn limits() -> NativeFinalityLimits {
    NativeFinalityLimits {
        block_bytes: 4 * 1024 * 1024,
        journal_bytes: 64 * 1024 * 1024,
        block_count: 64,
        allocated_bytes: 512 * 1024 * 1024,
    }
}
fn verifier(journal: &NativeFinalityJournal, network: NetworkId) -> NativePastaVerifier {
    let genesis = journal.blocks[0].decode_block(limits()).unwrap();
    assert_eq!(genesis.hash(), network.into_genesis_hash());
    NativePastaVerifier::new(
        crate::sumeragi::node::global_instance(&genesis, &chain_id().to_string()),
        network,
    )
}
fn outputs(block: &mut SignedBlock) {
    let values = (0..block.external_transactions().count())
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
            values,
            0,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .unwrap();
}
fn outcome(
    height: u64,
    current: &ValidatorEpochContextV1,
    boundary: Option<ValidatorEpochBoundaryV1>,
) -> ScheduleOutcome {
    let authorized = boundary.as_ref().map_or(current, |value| &value.next);
    let slot = |height| {
        let params = ChainParamsRecord::from_core(&ChainParams {
            epoch_length: 10,
            ..ChainParams::default()
        });
        if height <= authorized.authorization.last_height {
            ScheduledSlot::Ready(ScheduledConfig {
                height,
                epoch: authorized.clone(),
                params,
            })
        } else {
            ScheduledSlot::PendingBoundary {
                height,
                boundary_height: authorized.authorization.last_height,
                predecessor_context_id: authorized.context_id().unwrap(),
                params,
            }
        }
    };
    ScheduleOutcome {
        height,
        current: current.clone(),
        next: slot(height + 1),
        after_next: slot(height + 2),
        boundary,
    }
}
fn certify(
    keys: &[KeyPair],
    header: &BlockHeader,
    result: &ExecutionResultCommitment,
    bytes: &[u8],
) -> iroha_sumeragi::message::Qc {
    let crypto = BlsCrypto::new();
    let pops = keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
        .collect::<Vec<_>>();
    crypto
        .admit_committee(
            keys.iter()
                .zip(&pops)
                .map(|(key, pop)| (key.public_key(), pop.as_slice())),
        )
        .unwrap();
    let votes = (0..3)
        .map(|index| {
            let mut vote = Vote {
                kind: VoteKind::Commit,
                instance: header.instance,
                epoch: header.epoch,
                height: header.height,
                view: 0,
                block_hash: header.hash(&crypto),
                result: result_of_preimage(bytes),
                attest: header.attest,
                signer: index,
                sig: iroha_sumeragi::types::Signature([0; iroha_sumeragi::types::SIGNATURE_LEN]),
                attestation: None,
            };
            if header.attest {
                let message = native_seal_message(
                    header.instance,
                    result.schedule.current.network_id,
                    &vote.statement(),
                    bytes,
                )
                .unwrap();
                let signer = KagemushaMintFinalitySignerV1::from_seed(
                    zeroize::Zeroizing::new([0xA0 + index as u8; 32]),
                    index,
                    &result.schedule.current.authority,
                )
                .unwrap();
                vote.attestation = Some(CommitAttestation {
                    witness: ResultWitness::from_untrusted(bytes.to_vec()).unwrap(),
                    signature: encode_native_seal(signer.sign(&message).unwrap()),
                });
            }
            vote.sig = KeyPairSigner::new(&keys[index as usize])
                .unwrap()
                .sign(&vote.preimage());
            vote
        })
        .collect::<Vec<_>>();
    form_qc(&crypto, 4, &votes.iter().collect::<Vec<_>>()).unwrap()
}
fn evidence_fixture() -> ValidatorCommitteeProvisioningEvidenceV1 {
    use crate::beacon::{
        GlobalThresholdBeaconPartialSignerV1 as _, GlobalThresholdBeaconPulseAggregatorV1,
    };
    let mut keys = (1..=4)
        .map(|index| KeyPair::from_seed(vec![index; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| core_key(key.public_key()).unwrap());
    let validators = keys
        .iter()
        .map(|key| {
            (
                PeerId::new(key.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mut policy = SumeragiNposParameters::default();
    policy.epoch_length_blocks = NonZeroU64::new(10).unwrap();
    policy.evidence_horizon_blocks = 10;
    policy.slashing_delay_blocks = 10;
    let mut genesis = crate::sumeragi::test_chain::signed_genesis_fixture(
        &chain_id(),
        &KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519),
        &validators,
        Vec::new(),
        1000,
        ConsensusMode::Npos,
        Some(policy),
    )
    .unwrap();
    let network = NetworkId::from_genesis_hash(genesis.hash());
    let instance = crate::sumeragi::node::global_instance(&genesis, &chain_id().to_string());
    let mut current = crate::sumeragi::epoch::genesis_epoch(&genesis).unwrap();
    let state = crate::state::State::new_with_chain_and_network_id_for_testing(
        World::new(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id(),
        network,
    );
    let witness = complete_context_witness(network, 1);
    outputs(&mut genesis);
    let initial = ExecutionResultCommitment::new(
        1,
        execution_commitment(&witness, &genesis, &synthetic_world()).unwrap(),
        outcome(1, &current, None),
        None,
        iroha_data_model::sumeragi_finality::NativeLaneStateProof::from_witness(
            &witness,
            &iroha_allocation::AllocationBudget::new(64 * 1024),
        )
        .unwrap(),
    )
    .unwrap();
    let mut parent_result = result_of_preimage(&initial.preimage().unwrap());
    genesis = genesis.with_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        Vec::new(),
        Vec::new(),
        initial.preimage().unwrap(),
        Vec::new(), // Genesis is the signed root, not a native proposal.
    )));
    let mut parent_core = Hash32(*network.as_bytes());
    let mut history = vec![Arc::new(genesis)];
    let mut boundary_pulse = None;
    let mut selected_fixture = None;
    for height in 2..=14 {
        let parent = history.last().unwrap();
        let signer = KeyPair::from_seed(vec![0xCE; 32], Algorithm::Ed25519);
        let mut builder = iroha_data_model::transaction::TransactionBuilder::new(
            network,
            iroha_data_model::account::AccountId::new(signer.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(parent.header().creation_time());
        let input = builder
            .with_instructions([iroha_data_model::isi::Log::new(
                iroha_data_model::Level::INFO,
                format!("offline committee transcript {height}"),
            )])
            .sign(signer.private_key());
        let mut block = payload::assemble(
            &state,
            Assembly {
                parent,
                view: 0,
                cadence: Duration::from_millis(1),
            },
            &[crate::tx::AcceptedTransaction::new_unchecked(
                std::borrow::Cow::Owned(input),
            )],
        )
        .unwrap();
        outputs(&mut block);
        let context = GlobalThresholdBeaconPulseContextV1 {
            instance: instance.0,
            epoch: current.authorization.epoch,
            epoch_context_id: current.context_id().unwrap(),
            parent_consensus_hash: parent_core.0,
            parent_result: parent_result.0,
        };
        let pulse = if height == 9 {
            let fixture = crate::state::validator_committee::tests::fixture_with_native_selection(
                7,
                network,
                block.hash(),
                [0x31; 32],
            );
            let old = fixture
                .world
                .view()
                .global_beacon_key_sessions()
                .get(&[0x71; 32])
                .unwrap()
                .session
                .adaptive_dkg
                .session;
            let (session, signers) =
                crate::beacon::prepared_session_and_signers_fixture_for_keys_v1(old, &keys);
            let mut reducer = GlobalThresholdBeaconPulseAggregatorV1::new(
                session.clone(),
                height,
                GlobalThresholdBeaconChainAnchorV1 {
                    height: height - 1,
                    block_hash: parent.hash(),
                },
                context,
            )
            .unwrap();
            for signer in signers.iter().take(2) {
                reducer
                    .accept_partial(signer.sign_partial(&session, reducer.payload()).unwrap())
                    .unwrap();
            }
            Some(reducer.finalize().unwrap())
        } else {
            None
        };
        let boundary = if height == 10 {
            let pulse = boundary_pulse.as_ref().unwrap();
            let seed = crate::sumeragi::epoch_election::election_seed(network, 0, pulse).unwrap();
            let fixture = crate::state::validator_committee::tests::fixture_with_native_selection(
                7,
                network,
                parent.hash(),
                seed,
            );
            assert_eq!(fixture.incumbent, current.authority);
            let next = ValidatorEpochContextV1 {
                da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
                version: 1,
                network_id: network,
                mode: ConsensusMode::Npos,
                authority: current.authority.clone(),
                authorization: fixture.authorization,
                committee: current.committee.clone(),
                leader_seed: crate::beacon::global_threshold_beacon_npos_successor_seed_v1(
                    pulse, height, 1,
                ),
            };
            let boundary = ValidatorEpochBoundaryV1 {
                version: 1,
                height,
                predecessor_context_id: current.context_id().unwrap(),
                selection_anchor: parent.hash(),
                next,
                preparation: Some(fixture.transition.preparation.clone()),
            };
            selected_fixture = Some(fixture);
            Some(boundary)
        } else {
            None
        };
        let payload = payload::encode(&block).unwrap();
        let header = BlockHeader {
            instance,
            epoch: schedule::core_epoch(&current).unwrap().id,
            height,
            origin_view: 0,
            parent_hash: parent_core,
            parent_result,
            payload_hash: payload_hash(&BlsCrypto::new(), &payload),
            payload_len: u32::try_from(payload.len()).unwrap(),
            availability_digest: Hash32::ZERO,
            proposer: 0,
            skipped_leaders: Vec::new(),
            attest: boundary.is_some(),
            control_witness: crate::sumeragi::epoch_beacon::control::encode(pulse).unwrap(),
        };
        let config = ScheduledConfig {
            height,
            epoch: current.clone(),
            params: ChainParamsRecord::from_core(&ChainParams {
                epoch_length: 10,
                ..ChainParams::default()
            }),
        }
        .height_config()
        .unwrap();
        let budget = state.ivm_execution_budget();
        let mut backing = iroha_allocation::ChargedBuffer::new(payload.len(), &budget).unwrap();
        backing.append(&payload).unwrap();
        let charged_payload = PayloadBytes::from_charged(backing, &budget)
            .unwrap_or_else(|_| panic!("original committee fixture payload admission"));
        let crypto = BlsCrypto::new();
        crypto
            .admit_committee(current.committee.iter().map(|member| {
                (
                    member.validator.public_key(),
                    member.proof_of_possession.as_slice(),
                )
            }))
            .unwrap();
        let signer = KeyPairSigner::new(&keys[0]).unwrap();
        let authored = PayloadAuthoring::new(header, charged_payload)
            .complete(instance, &config, &budget, &crypto, &signer)
            .unwrap_or_else(|(_, error)| {
                panic!("original committee signed RS16 authoring: {error:?}")
            });
        assert!(authored.body.admitted_to(&budget));
        assert_eq!(authored.body.source().config(), &config);
        assert_eq!(authored.body.payload().as_slice(), payload);
        assert!(!authored.body.availability().as_slice().is_empty());
        let header = authored.body.header().clone();
        let availability = norito::encode_canonical(authored.body.availability()).unwrap();
        let witness = complete_context_witness(network, height);
        let result = ExecutionResultCommitment::new(
            height,
            execution_commitment(&witness, &block, &synthetic_world()).unwrap(),
            outcome(height, &current, boundary.clone()),
            pulse,
            iroha_data_model::sumeragi_finality::NativeLaneStateProof::from_witness(
                &witness,
                &iroha_allocation::AllocationBudget::new(64 * 1024),
            )
            .unwrap(),
        )
        .unwrap();
        let preimage = result.preimage().unwrap();
        let qc = certify(&keys, &header, &result, &preimage);
        block = block.with_commit_certificate(Some(
            crate::sumeragi::block_store::commit_certificate(&header, &qc, preimage, availability)
                .unwrap(),
        ));
        parent_core = header.hash(&BlsCrypto::new());
        parent_result = qc.result;
        history.push(Arc::new(block));
        if let Some(pulse) = pulse {
            boundary_pulse = Some(pulse);
        }
        if let Some(boundary) = boundary {
            current = boundary.next;
        }
    }
    let fixture = selected_fixture.unwrap();
    let prep = &fixture.transition.preparation;
    let view = fixture.world.view();
    let record = view
        .global_beacon_key_sessions()
        .get(&prep.beacon_session_id().unwrap())
        .unwrap();
    let BeaconEpochBindingV1::Installed(active) = fixture.authorization.beacon else {
        unreachable!()
    };
    let mut certificate = ThresholdKeyLifecycleCertificateV1 {
        version: 1,
        action: ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
        expected_active_session_id: Some(active.session_id),
        effective_height: 14,
        network_id: network,
        roster_hash: crate::beacon::global_threshold_beacon_roster_hash_v1(
            &validators.iter().map(|v| v.0.clone()).collect::<Vec<_>>(),
        ),
        committee_size: 4,
        quorum: 3,
        session_id: record.session.session_id,
        transcript_hash: record.session.transcript_hash,
        public_state: norito::encode_canonical(record).unwrap(),
        signatures: Vec::new(),
    };
    let preimage = threshold_key_lifecycle_certificate_preimage_v1(&certificate).unwrap();
    certificate.signatures = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, key)| ThresholdKeyLifecycleSignatureV1 {
            signer_index: index as u16,
            signature: Signature::new(key.private_key(), &preimage),
        })
        .collect();
    let candidates = prep
        .committee
        .iter()
        .map(|seat| {
            view.validator_candidate_keys()
                .get(&ValidatorCandidateKeysV1::key_id(
                    network,
                    1,
                    &seat.validator,
                ))
                .unwrap()
                .clone()
        })
        .collect();
    let blocks = history
        .iter()
        .map(|block| NativeFinalityArtifact::from_block(block, limits()).unwrap())
        .collect::<Vec<_>>();
    ValidatorCommitteeProvisioningEvidenceV1 {
        status: ValidatorCommitteeStatusV1 {
            network_id: network,
            target_epoch: 2,
            latest_finality: blocks[13].clone(),
            selected: Some(ValidatorCommitteeSelectionStatusV1 {
                transition: fixture.transition.clone(),
                selecting_finality: blocks[9].clone(),
            }),
            candidate_keys: candidates,
            pending_beacon_session: Some(record.session.clone()),
        },
        finality_journal: NativeFinalityJournal { blocks },
        beacon_finalization: certificate,
    }
}

fn selection_evidence_fixture() -> ValidatorCommitteeSelectionEvidenceV1 {
    let fixture = evidence_fixture();
    let mut status = fixture.status;
    let selected = status.selected.as_mut().expect("frozen selection fixture");
    selected.transition.credentials = None;
    selected.transition.readiness.clear();
    status.candidate_keys.clear();
    status.pending_beacon_session = None;
    ValidatorCommitteeSelectionEvidenceV1 {
        status,
        finality_journal: fixture.finality_journal,
    }
}

#[test]
fn selection_evidence_authenticates_only_frozen_roster_before_any_dkg_credentials() {
    let evidence = selection_evidence_fixture();
    let network = evidence.status.network_id;
    let chain_id = chain_id();
    let verifier = verifier(&evidence.finality_journal, network);
    let preparation = &evidence
        .status
        .selected
        .as_ref()
        .expect("selection")
        .transition
        .preparation;
    let attempt = preparation.transition_id().unwrap();
    let verify = |evidence: &ValidatorCommitteeSelectionEvidenceV1| {
        verify_validator_committee_selection_evidence_v1(
            evidence,
            &chain_id,
            network,
            2,
            attempt,
            limits(),
            &verifier,
        )
    };
    let selected = verify(&evidence).expect("contiguous incumbent-certified E+1 selection");
    assert_eq!(selected.preparation(), preparation);
    assert_eq!(selected.observed_height(), 14);
    assert_eq!(selected.incumbent_authority().validators.len(), 4);
    assert_ne!(
        selected.incumbent_beacon().session_id,
        selected.preparation().beacon_session_id().unwrap()
    );
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &ValidatorCommitteeProvisioningEvidenceV1 {
                status: evidence.status.clone(),
                finality_journal: evidence.finality_journal.clone(),
                beacon_finalization: evidence_fixture().beacon_finalization,
            },
            &chain_id,
            network,
            2,
            attempt,
            limits(),
            &verifier,
        )
        .is_err(),
        "selection-only proof cannot authorize private-share import"
    );
    let binary = norito::encode_canonical(&evidence).unwrap();
    assert_eq!(
        norito::decode_canonical::<ValidatorCommitteeSelectionEvidenceV1>(&binary).unwrap(),
        evidence
    );
    let json = norito::json::to_vec(&evidence).unwrap();
    assert_eq!(
        norito::json::from_slice::<ValidatorCommitteeSelectionEvidenceV1>(&json).unwrap(),
        evidence
    );
    for mutation in 0..5 {
        let mut changed = evidence.clone();
        match mutation {
            0 => {
                changed
                    .status
                    .selected
                    .as_mut()
                    .unwrap()
                    .transition
                    .preparation
                    .election_seed[0] ^= 1
            }
            1 => changed.status.latest_finality.block_wire.push(0),
            2 => {
                changed.finality_journal.blocks.remove(1);
            }
            3 => changed
                .status
                .selected
                .as_mut()
                .unwrap()
                .selecting_finality
                .block_wire
                .push(0),
            _ => {
                changed.finality_journal.blocks[9].block_wire.push(0);
            }
        }
        assert!(verify(&changed).is_err(), "selection mutation {mutation}");
    }
    assert!(
        verify_validator_committee_selection_evidence_v1(
            &evidence,
            &ChainId::from("foreign configured chain"),
            network,
            2,
            attempt,
            limits(),
            &verifier
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_selection_evidence_v1(
            &evidence,
            &chain_id,
            network,
            3,
            attempt,
            limits(),
            &verifier
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_selection_evidence_v1(
            &evidence,
            &chain_id,
            network,
            2,
            [0xEE; 32],
            limits(),
            &verifier
        )
        .is_err()
    );
}

#[test]
fn committee_custody_evidence_authenticates_exact_pending_attempt_and_rejects_substitution() {
    let evidence = evidence_fixture();
    let network = evidence.status.network_id;
    let chain_id = chain_id();
    let verifier = verifier(&evidence.finality_journal, network);
    let attempt = evidence
        .status
        .selected
        .as_ref()
        .unwrap()
        .transition
        .preparation
        .transition_id()
        .unwrap();
    let verify = |evidence: &ValidatorCommitteeProvisioningEvidenceV1| {
        verify_validator_committee_provisioning_evidence_v1(
            evidence,
            &chain_id,
            network,
            2,
            attempt,
            limits(),
            &verifier,
        )
    };
    let accepted = verify(&evidence).unwrap();
    assert_eq!(accepted.observed_height(), 14);
    assert_eq!(
        accepted.session(),
        evidence.status.pending_beacon_session.as_ref().unwrap()
    );
    assert_eq!(accepted.transition().preparation.target_epoch, 2);
    assert_eq!(accepted.incumbent_authority().validators.len(), 4);
    assert_ne!(
        accepted.incumbent_beacon().session_id,
        accepted.session().session_id
    );
    let binary = norito::encode_canonical(&evidence).unwrap();
    assert_eq!(
        norito::decode_canonical::<ValidatorCommitteeProvisioningEvidenceV1>(&binary).unwrap(),
        evidence
    );
    let json = norito::json::to_vec(&evidence).unwrap();
    assert_eq!(
        norito::json::from_slice::<ValidatorCommitteeProvisioningEvidenceV1>(&json).unwrap(),
        evidence
    );
    for mutation in 0..8 {
        let mut changed = evidence.clone();
        match mutation {
            0 => changed
                .beacon_finalization
                .signatures
                .pop()
                .map(|_| ())
                .unwrap(),
            1 => changed.beacon_finalization.expected_active_session_id = Some([0xEE; 32]),
            2 => changed.beacon_finalization.transcript_hash[0] ^= 1,
            3 => {
                changed.finality_journal.blocks.remove(1);
            }
            4 => changed.status.candidate_keys.swap(0, 1),
            5 => {
                changed
                    .status
                    .pending_beacon_session
                    .as_mut()
                    .unwrap()
                    .session_id[0] ^= 1
            }
            6 => {
                changed
                    .status
                    .selected
                    .as_mut()
                    .unwrap()
                    .selecting_finality
                    .block_wire
                    .push(1);
            }
            _ => changed.status.latest_finality.block_wire.push(0),
        }
        assert!(verify(&changed).is_err(), "mutation {mutation}");
    }
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &evidence,
            &ChainId::from("foreign configured chain"),
            network,
            2,
            attempt,
            limits(),
            &verifier
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &evidence,
            &chain_id,
            network,
            3,
            attempt,
            limits(),
            &verifier
        )
        .is_err()
    );
    assert!(
        verify_validator_committee_provisioning_evidence_v1(
            &evidence,
            &chain_id,
            network,
            2,
            [0xEE; 32],
            limits(),
            &verifier
        )
        .is_err()
    );
}

#[test]
fn status_selection_binding_uses_the_same_actual_native_boundary_as_custody() {
    let evidence = selection_evidence_fixture();
    let verifier = verifier(&evidence.finality_journal, evidence.status.network_id);
    with_verified_native_journal(
        &evidence.finality_journal,
        &chain_id(),
        &evidence.status.network_id,
        limits(),
        &verifier,
        |reader| {
            let latest = reader.certified(14).map_err(|error| error.to_string())?;
            let selecting = reader.certified(10).map_err(|error| error.to_string())?;
            let before = reader.certified(9).map_err(|error| error.to_string())?;
            let selection = &evidence.status.selected.as_ref().unwrap().transition;
            validate_validator_committee_selection_binding_v1(selection, &selecting, &latest, 2)?;
            assert!(
                validate_validator_committee_selection_binding_v1(selection, &before, &latest, 2)
                    .is_err(),
                "ordinary result cannot stand in for a boundary"
            );
            assert!(
                validate_validator_committee_selection_binding_v1(
                    selection, &selecting, &latest, 3
                )
                .is_err()
            );
            for mutation in 0..4 {
                let mut changed = selection.clone();
                match mutation {
                    0 => changed.preparation.election_seed[0] ^= 1,
                    1 => {
                        changed.preparation.selection_anchor = HashOf::from_untyped_unchecked(
                            Hash::new(b"foreign selection predecessor"),
                        )
                    }
                    2 => changed.preparation.selection_height += 1,
                    _ => changed.preparation.preparing_authorization_id[0] ^= 1,
                }
                assert!(
                    validate_validator_committee_selection_binding_v1(
                        &changed, &selecting, &latest, 2
                    )
                    .is_err(),
                    "status binding mutation {mutation}"
                );
            }
            Ok(())
        },
    )
    .expect("genuine native prefix");
}

/// Complete-set proof fixture derived from an actual canonical ordinary write.
fn complete_context_witness(network: NetworkId, height: u64) -> ExecWitness {
    let contexts = iroha_data_model::sumeragi_lanes::SumeragiLaneState::default();
    let commitment = iroha_data_model::sumeragi_finality::SumeragiLaneStateCommitment::from_state(
        network, height, &contexts,
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

/// Synthetic complete-World roots: these fixtures certify structural results, not a World.
fn synthetic_world() -> crate::sumeragi::commitment::WorldStateTransition {
    crate::sumeragi::commitment::WorldStateTransition {
        parent_world_state_root: iroha_crypto::Hash::new(b"synthetic parent World"),
        world_state_root: iroha_crypto::Hash::new(b"synthetic World"),
        event_commitment: None,
    }
}
