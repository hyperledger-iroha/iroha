//! Cryptographic proof and wire regressions for the current finality format.
use super::*;
use crate::sumeragi::epoch::ValidatorEpochContextV1;
use crate::{
    account::AccountId,
    block::{CommitCertificate, builder::BlockBuilder, output_test_support},
    isi::Log,
    level::Level,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_crypto::{KeyPair, bls_normal_pop_prove};
use iroha_sumeragi::types::{Bitmap, ChainParams};
use std::{collections::BTreeSet, num::NonZeroU64};

pub(crate) struct Fixture {
    pub(super) genesis: SignedBlock,
    pub(crate) first: SumeragiFinalityProof,
    pub(crate) second: SumeragiFinalityProof,
    pub(crate) keys: Vec<KeyPair>,
    pub(super) validators: Vec<FinalityValidator>,
    pub(crate) network: NetworkId,
}

pub(super) fn result(
    block: &SignedBlock,
    epoch: &ValidatorEpochContextV1,
) -> ExecutionResultCommitment {
    let (len, hash) = block.executed_block_wire_identity().unwrap();
    let height = block.header().height().get();
    let slot = |height| {
        ScheduledSlot::Ready(ScheduledConfig {
            height,
            epoch: epoch.clone(),
            params: ChainParamsRecord::from_core(&ChainParams::default()),
        })
    };
    let (native_lanes, ordinary_root) =
        NativeLaneStateProof::empty_for_testing(epoch.network_id, height);
    ExecutionResultCommitment::new(
        height,
        ExecutionCommitment {
            parent_state_root: Hash::new(b"parent"),
            post_state_root: ordinary_root,
            ordinary_writes_root: ordinary_root,
            kagemusha_top_up_root: None,
            kagemusha_top_up_count: 0,
            parent_world_state_root: Hash::new(b"fixture parent world"),
            world_state_root: Hash::new(b"fixture world"),
            event_commitment: None,
            executed_block_wire_len: len,
            executed_block_wire_hash: hash,
            transaction_input_commitment: block.network_input_merkle_commitment(),
            transaction_output_commitment: block.output_merkle_commitment(),
        },
        ScheduleOutcome {
            height,
            current: epoch.clone(),
            boundary: None,
            next: slot(height + 1),
            after_next: slot(height + 2),
        },
        None,
        native_lanes,
    )
    .unwrap()
}

pub(super) fn sign_qc(qc: &mut Qc, keys: &[KeyPair], chosen: &[u32]) {
    qc.signers = Bitmap::from_indices(keys.len(), chosen.iter().copied()).unwrap();
    let signatures: Vec<_> = chosen
        .iter()
        .map(|index| {
            iroha_crypto::Signature::try_new(keys[*index as usize].private_key(), &qc.preimage())
                .unwrap()
        })
        .collect();
    let bytes: Vec<_> = signatures
        .iter()
        .map(iroha_crypto::Signature::payload)
        .collect();
    qc.agg_sig = AggregateSignature(
        bls_normal_aggregate_signatures(&bytes)
            .unwrap()
            .try_into()
            .unwrap(),
    );
}

// Preserve the selected original epoch and use the sole fixture authoring path.
pub(super) fn author_payload(
    header: CoreHeader,
    payload: &[u8],
    epoch: &ValidatorEpochContextV1,
    keys: &[KeyPair],
) -> iroha_sumeragi::availability::AuthoredBody {
    let config = ScheduledConfig {
        height: header.height,
        epoch: epoch.clone(),
        params: ChainParamsRecord::from_core(&ChainParams::default()),
    }
    .height_config()
    .unwrap();
    let validators = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect::<Vec<_>>();
    let proposer = &keys[header.proposer as usize];
    let budget = iroha_allocation::AllocationBudget::new(128 * 1024 * 1024);
    super::test_fixtures::author_payload(header, payload, &config, &budget, &validators, proposer)
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let mut keys: Vec<_> = (1..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect();
        keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
        let validators: Vec<_> = keys
            .iter()
            .map(|key| FinalityValidator {
                public_key: key.public_key().clone(),
                proof_of_possession: bls_normal_pop_prove(key.private_key()).unwrap(),
            })
            .collect();
        let (crypto, _) = ProofCrypto::new(&validators).unwrap();
        let authority = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let account = AccountId::new(authority.public_key().clone());
        use crate::{
            block::consensus::SumeragiGenesisContextParameters,
            isi::{
                InstructionBox, RegisterPeerWithPop, SetParameter,
                kagemusha_v1::{
                    KagemushaMintFinalityAuthorityGenerationTemplateV1,
                    KagemushaMintFinalityGenesisParametersV1,
                },
            },
            parameter::{
                CustomParameter, Parameter,
                system::{
                    ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
                    consensus_metadata,
                },
            },
        };
        let epoch_fixture = crate::sumeragi::epoch::tests::fixture(4);
        let metadata = ConsensusHandshakeMetadata {
            mode: SumeragiConsensusMode::Permissioned,
            block_cadence_ms: NonZeroU64::new(1000).unwrap(),
            wire_protocol_version: u32::from(crate::sumeragi::PROTOCOL_VERSION),
            consensus_fingerprint: ConsensusFingerprint::new([0x71; 32]),
            kagemusha_mint_finality: KagemushaMintFinalityGenesisParametersV1 {
                authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
                    version: 1,
                    generation: 0,
                    validators: epoch_fixture.authority.validators,
                },
            },
            sumeragi_context: SumeragiGenesisContextParameters::recommended(),
        };
        let mut instructions = validators
            .iter()
            .map(|validator| {
                InstructionBox::from(RegisterPeerWithPop::new(
                    PeerId::new(validator.public_key.clone()),
                    validator.proof_of_possession.clone(),
                ))
            })
            .collect::<Vec<_>>();
        instructions.push(
            SetParameter::new(Parameter::Custom(CustomParameter::new(
                consensus_metadata::handshake_meta_id(),
                iroha_primitives::json::Json::new(metadata),
            )))
            .into(),
        );
        let tx = TransactionBuilder::new_genesis(
            account.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions(instructions)
        .sign(authority.private_key());
        let genesis =
            SignedBlock::try_genesis(vec![tx], authority.private_key(), None, None).unwrap();
        let network = NetworkId::from_genesis_hash(genesis.hash());
        let epoch = genesis_epoch(&genesis).unwrap();
        let instance = instance_id(
            &crypto,
            &Hash32(Hash::from(genesis.hash()).into()),
            b"portable-finality-test",
            InstanceKind::Global,
            0,
        );
        let mut first_block = genesis.clone();
        output_test_support::install_network(&mut first_block, vec![Ok(Default::default())])
            .unwrap();
        let first_result = result(&first_block, &epoch);
        first_block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            vec![],
            vec![],
            first_result.preimage().unwrap(),
            vec![],
        )));
        let first = SumeragiFinalityProof {
            block_header: first_block.header(),
            block_wire: first_block.encode_wire().unwrap(),
            committee: validators.clone(),
        };
        let tx =
            TransactionBuilder::new(network, account, FeePaymentIntent::authority(vec![], None))
                .with_instructions([Log::new(Level::INFO, "real submitted work".into())])
                .sign(authority.private_key());
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(genesis.hash()),
            None,
            genesis.header().creation_time_ms.saturating_add(1),
            0,
        ));
        builder.push_transaction(tx);
        let mut block = builder.build(BTreeSet::new());
        output_test_support::install_network(&mut block, vec![Ok(Default::default())]).unwrap();
        let result = result(&block, &epoch);
        let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
        let header = CoreHeader {
            control_witness: iroha_sumeragi::types::ControlWitness::empty(),
            instance,
            epoch: core_epoch(&epoch).unwrap().id,
            height: 2,
            origin_view: 0,
            parent_hash: Hash32(Hash::from(genesis.hash()).into()),
            parent_result: first_result.result().unwrap(),
            payload_hash: payload_hash(&crypto, &payload),
            availability_digest: Hash32::ZERO,
            payload_len: payload.len().try_into().unwrap(),
            proposer: 0,
            skipped_leaders: vec![],
            attest: false,
        };
        let authored = author_payload(header, &payload, &epoch, &keys);
        let header = authored.body.header().clone();
        let availability = norito::encode_canonical(authored.body.availability()).unwrap();
        let mut qc = Qc {
            kind: VoteKind::Commit,
            instance,
            epoch: header.epoch,
            height: 2,
            view: 0,
            block_hash: header.hash(&crypto),
            result: result.result().unwrap(),
            attest: false,
            signers: Bitmap::new(4),
            agg_sig: AggregateSignature([0; 96]),
            attestations: vec![],
            attestation_witness: None,
        };
        sign_qc(&mut qc, &keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            norito::encode_canonical(&header).unwrap(),
            norito::encode_canonical(&qc).unwrap(),
            result.preimage().unwrap(),
            availability,
        )));
        let second = SumeragiFinalityProof {
            block_header: block.header(),
            block_wire: block.encode_wire().unwrap(),
            committee: validators.clone(),
        };
        Self {
            genesis,
            first,
            second,
            keys,
            validators,
            network,
        }
    }
    pub(crate) fn verifier(&self) -> SumeragiFinalityVerifier {
        SumeragiFinalityVerifier::new(
            &self.genesis,
            "portable-finality-test",
            self.validators.clone(),
        )
        .unwrap()
    }
    pub(super) fn alternate(&self) -> SumeragiFinalityProof {
        let mut proof = self.second.clone();
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let consensus_header = certificate.consensus_header().to_vec();
        let result_preimage = certificate.result_preimage().to_vec();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        qc.view = 1;
        sign_qc(&mut qc, &self.keys, &[1, 2, 3]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            consensus_header,
            norito::encode_canonical(&qc).unwrap(),
            result_preimage,
            certificate.availability().to_vec(),
        )));
        proof.block_wire = block.encode_wire().unwrap();
        proof
    }
}

#[test]
fn current_proofs_roundtrip_and_verify_successful_exact_execution() {
    let fixture = Fixture::new();
    for proof in [&fixture.first, &fixture.second] {
        let encoded = norito::encode_canonical(proof).unwrap();
        assert_eq!(
            norito::decode_canonical::<SumeragiFinalityProof>(&encoded).unwrap(),
            *proof
        );
        let json = norito::json::to_vec(proof).unwrap();
        assert_eq!(
            norito::json::from_slice::<SumeragiFinalityProof>(&json).unwrap(),
            *proof
        );
    }
    let mut verifier = fixture.verifier();
    assert!(verifier.verify(&fixture.second).is_err(), "no gaps");
    verifier.verify(&fixture.first).unwrap();
    let verified = verifier.verify(&fixture.second).unwrap();
    let committed = output_test_support::committed(verified.block(), 0);
    verified
        .verify_committed_transaction(&fixture.network, &committed)
        .unwrap();
    let foreign =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
    assert!(
        verified
            .verify_committed_transaction(&foreign, &committed)
            .is_err()
    );
    assert!(
        decode_versioned_signed_block(&verified.canonical_executed_wire().unwrap())
            .unwrap()
            .commit_certificate()
            .is_none()
    );
}

#[test]
fn alternate_current_quorum_witnesses_have_one_authenticated_execution() {
    let fixture = Fixture::new();
    let alternate = fixture.alternate();
    let mut verifier = fixture.verifier();
    assert!(
        verifier
            .verify_same_decision(&fixture.second, &fixture.second)
            .is_err()
    );
    verifier.verify(&fixture.first).unwrap();
    let first = verifier.verify(&fixture.second).unwrap();
    let second = verifier
        .verify_same_decision(&fixture.second, &alternate)
        .unwrap();
    assert_ne!(fixture.second.block_wire, alternate.block_wire);
    assert_eq!(
        first.canonical_executed_wire().unwrap(),
        second.canonical_executed_wire().unwrap()
    );
    assert_eq!(first.result(), second.result());
}

#[test]
fn current_proof_rejects_tampered_qc_result_committee_parent_wire_and_availability() {
    let fixture = Fixture::new();
    for mutation in 0..8 {
        let mut bad = fixture.second.clone();
        let mut block = decode_versioned_signed_block(&bad.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let mut consensus_header = certificate.consensus_header().to_vec();
        let mut result_preimage = certificate.result_preimage().to_vec();
        let mut availability = certificate.availability().to_vec();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        match mutation {
            0 => qc.agg_sig.0[0] ^= 1,
            1 => qc.result = Hash32([9; 32]),
            2 => bad.committee[0].proof_of_possession[0] ^= 1,
            3 => {
                let mut header: CoreHeader = norito::decode_canonical(&consensus_header).unwrap();
                header.parent_result = Hash32([9; 32]);
                let epoch = genesis_epoch(&fixture.genesis).unwrap();
                let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
                let authored = author_payload(header, &payload, &epoch, &fixture.keys);
                let header = authored.body.header();
                availability = norito::encode_canonical(authored.body.availability()).unwrap();
                let (crypto, _) = ProofCrypto::new(&fixture.validators).unwrap();
                qc.block_hash = header.hash(&crypto);
                sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
                consensus_header = norito::encode_canonical(header).unwrap();
            }
            4 => sign_qc(&mut qc, &fixture.keys, &[0, 1]),
            5 => result_preimage.push(0),
            6 => availability.clear(),
            _ => *availability.last_mut().unwrap() ^= 1,
        }
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            consensus_header,
            norito::encode_canonical(&qc).unwrap(),
            result_preimage,
            availability,
        )));
        bad.block_wire = block.encode_wire().unwrap();
        let mut verifier = fixture.verifier();
        verifier.verify(&fixture.first).unwrap();
        assert!(verifier.verify(&bad).is_err(), "mutation {mutation}");
    }
    let mut trailing = fixture.second.clone();
    trailing.block_wire.push(0);
    assert!(trailing.decode_checked().is_err());
}

#[test]
fn current_attestation_roundtrip_binds_challenge_node_status_and_runtime_identity() {
    let fixture = Fixture::new();
    let node_id = PeerId::new(fixture.keys[0].public_key().clone());
    let body = SumeragiFinalityAttestationBody {
        challenge: [7; 32],
        network_id: fixture.network,
        node_fingerprint: Hash::new(node_id.encode()),
        node_id,
        build_fingerprint: Hash::new(b"compiled build"),
        config_fingerprint: Hash::new(b"effective config"),
        genesis_block_hash: fixture.genesis.hash(),
        genesis_finality_proof: fixture.first.clone(),
        status: SumeragiStatus {
            protocol_version: crate::sumeragi::PROTOCOL_VERSION,
            config_fingerprint: Hash::new(b"effective config"),
            beacon_horizon: None,
            instance: fixture.verifier().instance().0,
            height: 3,
            view: 0,
            stage: 0,
            leader: None,
            proxy_tail: None,
            high_qc_view: None,
            level: 0,
            start_level: 0,
            t_retx_ms: 100,
            committed_height: 2,
            applied_height: 2,
            awaiting: false,
            signer: Some(fixture.keys[0].public_key().clone()),
            unanchored: false,
            abstaining: false,
            halted: None,
            footprint: Default::default(),
        },
        finality_proof: fixture.second,
    };
    let attestation = SumeragiFinalityAttestation {
        signature: SignatureOf::try_from_hash(fixture.keys[0].private_key(), body.signing_hash())
            .unwrap(),
        body,
    };
    attestation.verify().unwrap();
    let wire = norito::encode_canonical(&attestation).unwrap();
    assert_eq!(
        norito::decode_canonical::<SumeragiFinalityAttestation>(&wire).unwrap(),
        attestation
    );
    let json = norito::json::to_vec(&attestation).unwrap();
    assert_eq!(
        norito::json::from_slice::<SumeragiFinalityAttestation>(&json).unwrap(),
        attestation
    );
    for version in [0, 2, 4, 8, u16::MAX] {
        let mut bad = attestation.clone();
        bad.body.status.protocol_version = version;
        bad.signature =
            SignatureOf::try_from_hash(fixture.keys[0].private_key(), bad.body.signing_hash())
                .unwrap();
        assert!(
            bad.verify().is_err(),
            "authentic signature cannot authorize protocol {version}"
        );
    }
    for mutation in 0..5 {
        let mut bad = attestation.clone();
        match mutation {
            0 => bad.body.challenge = [0; 32],
            1 => bad.body.status.committed_height = 3,
            2 => bad.body.node_id = PeerId::new(fixture.keys[1].public_key().clone()),
            3 => bad.body.build_fingerprint = Hash::new(b"different binary"),
            _ => bad.body.config_fingerprint = Hash::new(b"different config"),
        }
        assert!(bad.verify().is_err(), "mutation {mutation}");
    }
}

#[test]
fn complete_result_roundtrip_rejects_retired_scalar_schedule_layout() {
    let fixture = Fixture::new();
    let value = fixture.second.decode_checked().unwrap().commitment;
    let bytes = value.preimage().unwrap();
    assert_eq!(ExecutionResultCommitment::decode(&bytes).unwrap(), value);
    #[derive(norito::NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::sumeragi_finality::ExecutionResultCommitment")]
    struct RetiredResult {
        execution: ExecutionCommitment,
        next_committee_digest: [u8; 32],
        next_params: ChainParamsRecord,
    }
    let retired = RetiredResult {
        execution: value.execution,
        next_committee_digest: [3; 32],
        next_params: *value.schedule.next.params(),
    };
    assert!(
        ExecutionResultCommitment::decode(&norito::encode_canonical(&retired).unwrap()).is_err()
    );
}

#[test]
fn certified_result_cannot_replace_its_incumbent_or_fixed_next_parameters() {
    let fixture = Fixture::new();
    for change_epoch in [false, true] {
        let mut proof = fixture.second.clone();
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let mut value = ExecutionResultCommitment::decode(certificate.result_preimage()).unwrap();
        let mut header: CoreHeader =
            norito::decode_canonical(certificate.consensus_header()).unwrap();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        if change_epoch {
            value.schedule.current.leader_seed[0] ^= 1;
            let changed = value.schedule.current.clone();
            for slot in [&mut value.schedule.next, &mut value.schedule.after_next] {
                let ScheduledSlot::Ready(config) = slot else {
                    panic!("permissioned ready slot");
                };
                config.epoch = changed.clone();
            }
            header.epoch = core_epoch(&changed).unwrap().id;
            qc.epoch = header.epoch;
        } else {
            let ScheduledSlot::Ready(config) = &mut value.schedule.next else {
                panic!("ready slot");
            };
            config.params.max_block_bytes -= 1;
        }
        value.validate().unwrap();
        let (crypto, _) = ProofCrypto::new(&fixture.validators).unwrap();
        let availability = if change_epoch {
            let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
            let authored = author_payload(header, &payload, &value.schedule.current, &fixture.keys);
            header = authored.body.header().clone();
            norito::encode_canonical(authored.body.availability()).unwrap()
        } else {
            certificate.availability().to_vec()
        };
        qc.block_hash = header.hash(&crypto);
        qc.result = value.result().unwrap();
        sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            norito::encode_canonical(&header).unwrap(),
            norito::encode_canonical(&qc).unwrap(),
            value.preimage().unwrap(),
            availability,
        )));
        proof.block_wire = block.encode_wire().unwrap();
        proof.decode_checked().unwrap();
        let mut verifier = fixture.verifier();
        verifier.verify(&fixture.first).unwrap();
        assert!(
            verifier.verify(&proof).is_err(),
            "a valid signature cannot replace independently scheduled authority"
        );
    }
}

#[test]
fn certified_beacon_pulse_requires_the_exact_committed_parent() {
    use crate::consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        GlobalThresholdBeaconChainAnchorV1,
    };

    let fixture = Fixture::new();
    for foreign_parent in [false, true] {
        let mut proof = fixture.second.clone();
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let header = certificate.consensus_header().to_vec();
        let native_header: CoreHeader = norito::decode_canonical(&header).unwrap();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        let mut value = ExecutionResultCommitment::decode(certificate.result_preimage()).unwrap();
        // The portable receipt authenticates the execution result with its quorum signature;
        // threshold-beacon verification belongs to execution. Supply a canonical nonzero G1
        // point to isolate the exact public anchor binding checked by this receipt.
        let (_, signature) = fixture.keys[0].public_key().try_to_bytes().unwrap();
        let mut pulse = FinalizedGlobalThresholdBeaconPulseV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: fixture.network,
            session_id: [1; 32],
            roster_hash: [2; 32],
            transcript_hash: [3; 32],
            context: crate::consensus::GlobalThresholdBeaconPulseContextV1 {
                instance: native_header.instance.0,
                epoch: value.schedule.current.authorization.epoch,
                epoch_context_id: native_header.epoch.context.0,
                parent_consensus_hash: native_header.parent_hash.0,
                parent_result: native_header.parent_result.0,
            },
            height: 2,
            round: 0,
            finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
                height: 1,
                block_hash: if foreign_parent {
                    HashOf::from_untyped_unchecked(Hash::new(b"foreign finalized parent"))
                } else {
                    fixture.genesis.hash()
                },
            },
            signature: signature.try_into().unwrap(),
            seed: [4; 32],
            pulse_id: [0; 32],
        };
        pulse.pulse_id = global_threshold_beacon_pulse_id_v1(&pulse, pulse.seed);
        value.beacon = Some(pulse);
        value.validate().unwrap();
        qc.result = value.result().unwrap();
        sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            header,
            norito::encode_canonical(&qc).unwrap(),
            value.preimage().unwrap(),
            certificate.availability().to_vec(),
        )));
        proof.block_wire = block.encode_wire().unwrap();
        let mut verifier = fixture.verifier();
        verifier.verify(&fixture.first).unwrap();
        if foreign_parent {
            assert_eq!(
                proof.decode_checked().unwrap_err().0,
                "beacon pulse names another committed parent"
            );
            assert!(verifier.verify(&proof).is_err());
        } else {
            proof.decode_checked().unwrap();
            verifier.verify(&proof).unwrap();
        }
    }
}

#[test]
fn boundary_schedule_roundtrip_preserves_barrier_and_exact_successor_authority() {
    use crate::sumeragi::epoch::{
        ValidatorEpochBoundaryV1,
        tests::{fixture, retained},
    };
    let current = fixture(4);
    let next_epoch = retained(&current);
    let params = ChainParamsRecord::from_core(&ChainParams::default());
    let slot = |height, epoch: &ValidatorEpochContextV1| {
        ScheduledSlot::Ready(ScheduledConfig {
            height,
            epoch: epoch.clone(),
            params,
        })
    };
    let before = ScheduleOutcome {
        height: 9,
        current: current.clone(),
        boundary: None,
        next: slot(10, &current),
        after_next: ScheduledSlot::PendingBoundary {
            height: 11,
            boundary_height: 10,
            predecessor_context_id: current.context_id().unwrap(),
            params,
        },
    };
    let boundary = ScheduleOutcome {
        height: 10,
        current: current.clone(),
        boundary: Some(ValidatorEpochBoundaryV1 {
            version: 1,
            height: 10,
            predecessor_context_id: current.context_id().unwrap(),
            selection_anchor: HashOf::from_untyped_unchecked(Hash::new(
                b"certified boundary parent",
            )),
            next: next_epoch.clone(),
            preparation: None,
        }),
        next: slot(11, &next_epoch),
        after_next: slot(12, &next_epoch),
    };
    before.validate_successor(&boundary).unwrap();
    let bytes = norito::encode_canonical(&boundary).unwrap();
    let decoded: ScheduleOutcome = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, boundary);
    before.validate_successor(&decoded).unwrap();
    let mut changed = boundary.clone();
    let ScheduledSlot::Ready(next) = &mut changed.next else {
        panic!("installed successor");
    };
    next.params.max_block_bytes -= 1;
    assert!(before.validate_successor(&changed).is_err());
    let mut no_barrier = before;
    no_barrier.after_next = slot(11, &next_epoch);
    assert!(no_barrier.validate_successor(&boundary).is_err());
}

#[test]
fn certified_beacon_pulse_requires_exact_parent_and_native_context() {
    use crate::consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconPulseContextV1,
    };

    let fixture = Fixture::new();
    for mutation in 0..7 {
        let mut proof = fixture.second.clone();
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let header = certificate.consensus_header().to_vec();
        let native_header: CoreHeader = norito::decode_canonical(&header).unwrap();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        let mut value = ExecutionResultCommitment::decode(certificate.result_preimage()).unwrap();
        // The portable receipt authenticates the execution result with its quorum signature;
        // threshold-beacon verification belongs to execution. Supply a canonical nonzero G1
        // point to isolate the exact public anchor binding checked by this receipt.
        let (_, signature) = fixture.keys[0].public_key().try_to_bytes().unwrap();
        let mut pulse = FinalizedGlobalThresholdBeaconPulseV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: fixture.network,
            session_id: [1; 32],
            roster_hash: [2; 32],
            transcript_hash: [3; 32],
            context: GlobalThresholdBeaconPulseContextV1 {
                instance: native_header.instance.0,
                epoch: value.schedule.current.authorization.epoch,
                epoch_context_id: native_header.epoch.context.0,
                parent_consensus_hash: native_header.parent_hash.0,
                parent_result: native_header.parent_result.0,
            },
            height: 2,
            round: 0,
            finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
                height: 1,
                block_hash: if mutation == 6 {
                    HashOf::from_untyped_unchecked(Hash::new(b"foreign finalized parent"))
                } else {
                    fixture.genesis.hash()
                },
            },
            signature: signature.try_into().unwrap(),
            seed: [4; 32],
            pulse_id: [0; 32],
        };
        match mutation {
            1 => pulse.context.instance[0] ^= 1,
            2 => pulse.context.epoch += 1,
            3 => pulse.context.epoch_context_id[0] ^= 1,
            4 => pulse.context.parent_consensus_hash[0] ^= 1,
            5 => pulse.context.parent_result[0] ^= 1,
            _ => {}
        }
        pulse.pulse_id = global_threshold_beacon_pulse_id_v1(&pulse, pulse.seed);
        value.beacon = Some(pulse);
        value.validate().unwrap();
        qc.result = value.result().unwrap();
        sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            header,
            norito::encode_canonical(&qc).unwrap(),
            value.preimage().unwrap(),
            certificate.availability().to_vec(),
        )));
        proof.block_wire = block.encode_wire().unwrap();
        let mut verifier = fixture.verifier();
        verifier.verify(&fixture.first).unwrap();
        if mutation == 6 {
            assert_eq!(
                proof.decode_checked().unwrap_err().0,
                "beacon pulse names another committed parent"
            );
            assert!(verifier.verify(&proof).is_err());
        } else if mutation != 0 {
            assert_eq!(
                proof.decode_checked().unwrap_err().0,
                "beacon pulse names another native consensus context"
            );
            assert!(verifier.verify(&proof).is_err());
        } else {
            proof.decode_checked().unwrap();
            verifier.verify(&proof).unwrap();
        }
    }
}

#[test]
fn quorum_certificate_and_control_bytes_cannot_authorize_no_work() {
    let fixture = Fixture::new();
    let (crypto, _) = ProofCrypto::new(&fixture.validators).unwrap();
    for with_control in [false, true] {
        let mut proof = fixture.second.clone();
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let mut header: CoreHeader =
            norito::decode_canonical(certificate.consensus_header()).unwrap();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        let epoch = genesis_epoch(&fixture.genesis).unwrap();
        let availability = certificate.availability().to_vec();
        block.set_external_entrypoints(Vec::new());
        block.set_commit_certificate(None);
        output_test_support::install_network(&mut block, Vec::new()).unwrap();
        assert!(!block.has_consensus_work());
        let result = result(&block, &epoch);
        let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
        header.origin_view = 19;
        // Public control bytes cannot change the original work predicate. No
        // threshold validity is claimed or needed for this early no-work rejection.
        header.control_witness = if with_control {
            iroha_sumeragi::types::ControlWitness::try_from_slice(&[1, 2, 3]).unwrap()
        } else {
            iroha_sumeragi::types::ControlWitness::empty()
        };
        header.payload_hash = payload_hash(&crypto, &payload);
        header.payload_len = payload.len().try_into().unwrap();
        qc.view = 19;
        qc.block_hash = header.hash(&crypto);
        qc.result = result.result().unwrap();
        sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            norito::encode_canonical(&header).unwrap(),
            norito::encode_canonical(&qc).unwrap(),
            result.preimage().unwrap(),
            availability,
        )));
        proof.block_header = block.header();
        proof.block_wire = block.encode_wire().unwrap();
        assert_eq!(
            proof.decode_checked().unwrap_err().0,
            "empty blocks are invalid"
        );
    }
}

#[test]
fn native_proof_rejects_retired_bridge_context_and_artifact_fields() {
    let fixture = Fixture::new();
    let canonical = norito::json::to_json(&fixture.second).unwrap();
    for field in [
        "version",
        "finality_artifact",
        "height_context_id",
        "commitment",
    ] {
        let hostile = canonical.replacen('{', &format!("{{\"{field}\":null,"), 1);
        assert!(
            norito::json::from_json::<SumeragiFinalityProof>(&hostile).is_err(),
            "{field}"
        );
    }
    for missing in ["block_header", "block_wire", "committee"] {
        let mut value: norito::json::Value = norito::json::from_json(&canonical).unwrap();
        value.as_object_mut().unwrap().remove(missing);
        let hostile = norito::json::to_json(&value).unwrap();
        assert!(
            norito::json::from_json::<SumeragiFinalityProof>(&hostile).is_err(),
            "{missing}"
        );
    }
}

impl Fixture {
    pub(crate) fn next_header(&self) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(self.genesis.hash()),
            None,
            self.genesis.header().creation_time_ms + 1,
            0,
        )
    }
    pub(crate) fn proof_for_block(&self, mut block: SignedBlock) -> SumeragiFinalityProof {
        assert_eq!(block.header().height().get(), 2);
        assert_eq!(block.header().prev_block_hash(), Some(self.genesis.hash()));
        let parent = self.first.decode_checked().unwrap();
        let epoch = &parent.commitment.schedule.current;
        let result = result(&block, epoch);
        let (crypto, _) = ProofCrypto::new(&self.validators).unwrap();
        let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
        let header = CoreHeader {
            instance: self.verifier().instance(),
            epoch: core_epoch(epoch).unwrap().id,
            height: 2,
            origin_view: 0,
            parent_hash: parent.core_hash,
            parent_result: parent.result,
            payload_hash: payload_hash(&crypto, &payload),
            availability_digest: Hash32::ZERO,
            payload_len: payload.len().try_into().unwrap(),
            proposer: 0,
            skipped_leaders: vec![],
            control_witness: iroha_sumeragi::types::ControlWitness::empty(),
            attest: false,
        };
        let authored = author_payload(header, &payload, epoch, &self.keys);
        let header = authored.body.header().clone();
        let availability = norito::encode_canonical(authored.body.availability()).unwrap();
        let mut qc = Qc {
            kind: VoteKind::Commit,
            instance: header.instance,
            epoch: header.epoch,
            height: 2,
            view: 0,
            block_hash: header.hash(&crypto),
            result: result.result().unwrap(),
            attest: false,
            signers: Bitmap::new(4),
            agg_sig: AggregateSignature([0; 96]),
            attestations: vec![],
            attestation_witness: None,
        };
        sign_qc(&mut qc, &self.keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            norito::encode_canonical(&header).unwrap(),
            norito::encode_canonical(&qc).unwrap(),
            result.preimage().unwrap(),
            availability,
        )));
        SumeragiFinalityProof {
            block_header: block.header(),
            block_wire: block.encode_wire().unwrap(),
            committee: self.validators.clone(),
        }
    }
    pub(crate) fn verify_block(&self, block: SignedBlock) -> VerifiedSumeragiBlock {
        let proof = self.proof_for_block(block);
        let mut verifier = self.verifier();
        verifier.verify(&self.first).unwrap();
        verifier.verify(&proof).unwrap()
    }
}

#[test]
fn signed_genesis_layout_reaches_the_native_epoch_exactly() {
    let fixture = Fixture::new();
    let signed = signed_genesis_consensus_metadata(&fixture.genesis).unwrap();
    let epoch = genesis_epoch(&fixture.genesis).unwrap();
    assert_eq!(epoch.da_layout, signed.sumeragi_context.da_layout);
    assert_eq!(core_epoch(&epoch).unwrap().da_layout, epoch.da_layout);
}
