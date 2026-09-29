//! Genuine native BLS certificates over explicitly synthetic execution results.
//! These portable verifier fixtures do not execute World, custody, DKG or Pasta application seals.
use super::*;
use halo2curves::{
    group::{Curve, GroupEncoding},
    pasta::{Fp, Fq, Pallas, Vesta},
};
use iroha_crypto::{
    Algorithm, Hash, KeyPair, Signature, SignatureOf, bls_normal_aggregate_signatures,
    bls_normal_pop_prove,
};
use iroha_data_model::{
    account::AccountId,
    block::{CommitCertificate, builder::BlockBuilder, decode_versioned_signed_block},
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
        GlobalThresholdBeaconPulseContextV1,
    },
    isi::{InstructionBox, Log, RegisterPeerWithPop, SetParameter, kagemusha_v1::*},
    level::Level,
    parameter::{
        CustomParameter, Parameter,
        system::{
            ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
            SumeragiNposParameters, consensus_metadata,
        },
    },
    sumeragi::{
        SumeragiStatus,
        epoch::{ValidatorCommitteeMemberV1, ValidatorEpochBoundaryV1, ValidatorEpochContextV1},
    },
    sumeragi_finality::{
        ChainParamsRecord, ExecutionCommitment, ExecutionResultCommitment, NativeLaneStateProof,
        ScheduleOutcome, ScheduledConfig, SumeragiFinalityAttestationBody, chain_hash, core_epoch,
        genesis_epoch, global_threshold_beacon_npos_successor_seed_v1,
        global_threshold_beacon_pulse_id_v1, global_threshold_beacon_pulse_payload_v1,
        test_fixtures::NativeFinalityFixture,
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_sumeragi::{
    message::{AttestationSignature, BlockHeader as CoreHeader, Qc, ResultWitness, VoteKind},
    preimage::{TAG_PAY, block_hash_preimage},
    types::{AggregateSignature, Bitmap, ChainParams, ControlWitness, Hash32},
};
use norito::codec::Encode as _;
use std::{cell::RefCell, collections::BTreeSet, ops::Range, time::Duration};

const SIZES: [usize; 3] = [4, 7, 10];
const CHALLENGE: [u8; 32] = [0xC1; 32];
const CHAIN: &str = "deployment-native-finality";
fn nz(height: u64) -> NonZeroU64 {
    NonZeroU64::new(height).unwrap()
}
fn key(index: usize) -> KeyPair {
    KeyPair::from_seed(
        Hash::new(format!("deploy-native-{index}"))
            .as_ref()
            .to_vec(),
        Algorithm::BlsNormal,
    )
}
fn peer(key: &KeyPair) -> PeerId {
    PeerId::new(key.public_key().clone())
}
fn ordered_keys(range: Range<usize>) -> Vec<KeyPair> {
    let mut keys: Vec<_> = range.map(key).collect();
    keys.sort_by_key(|k| k.public_key().try_to_bytes().unwrap().1.to_vec());
    keys
}
fn validators(keys: &[KeyPair]) -> Vec<FinalityValidator> {
    keys.iter()
        .map(|k| FinalityValidator {
            public_key: k.public_key().clone(),
            proof_of_possession: bls_normal_pop_prove(k.private_key()).unwrap(),
        })
        .collect()
}
fn pasta(keys: &[KeyPair], generation: u64) -> Vec<KagemushaMintFinalityValidatorKeysV1> {
    keys.iter()
        .enumerate()
        .map(|(index, k)| {
            let n = generation * 32 + index as u64 + 1;
            KagemushaMintFinalityValidatorKeysV1 {
                validator: peer(k),
                eq_proof_public_key: (Pallas::generator() * Fq::from(n))
                    .to_affine()
                    .to_bytes()
                    .as_ref()
                    .try_into()
                    .unwrap(),
                ep_proof_public_key: (Vesta::generator() * Fp::from(n))
                    .to_affine()
                    .to_bytes()
                    .as_ref()
                    .try_into()
                    .unwrap(),
            }
        })
        .collect()
}
fn block(proof: &SumeragiFinalityProof) -> SignedBlock {
    decode_versioned_signed_block(&proof.block_wire).unwrap()
}
fn result(proof: &SumeragiFinalityProof) -> ExecutionResultCommitment {
    ExecutionResultCommitment::decode(block(proof).commit_certificate().unwrap().result_preimage())
        .unwrap()
}
fn core(proof: &SumeragiFinalityProof) -> CoreHeader {
    norito::decode_canonical(
        block(proof)
            .commit_certificate()
            .unwrap()
            .consensus_header(),
    )
    .unwrap()
}
fn sign_qc(qc: &mut Qc, keys: &[KeyPair], indices: &[u32]) {
    qc.signers = Bitmap::from_indices(keys.len(), indices.iter().copied()).unwrap();
    let signatures: Vec<_> = indices
        .iter()
        .map(|i| Signature::try_new(keys[*i as usize].private_key(), &qc.preimage()).unwrap())
        .collect();
    qc.agg_sig = AggregateSignature(
        bls_normal_aggregate_signatures(
            &signatures
                .iter()
                .map(Signature::payload)
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
}
fn replace_certificate(
    proof: &mut SumeragiFinalityProof,
    header: &CoreHeader,
    qc: &Qc,
    result: &ExecutionResultCommitment,
) {
    let mut b = block(proof);
    b.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        norito::encode_canonical(header).unwrap(),
        norito::encode_canonical(qc).unwrap(),
        result.preimage().unwrap(),
    )));
    proof.block_wire = b.encode_wire().unwrap();
}
struct Epoch {
    keys: Vec<KeyPair>,
    context: ValidatorEpochContextV1,
}
struct Chain {
    anchor: GenesisAnchor,
    epochs: Vec<Epoch>,
    proofs: Vec<SumeragiFinalityProof>,
}
impl Chain {
    fn constant(size: usize, tip: u64) -> Self {
        Self::new(&[(0..size, tip + 3)], tip)
    }
    fn new(ranges: &[(Range<usize>, u64)], tip: u64) -> Self {
        let keys = ordered_keys(ranges[0].0.clone());
        let authority = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let metadata = ConsensusHandshakeMetadata {
            mode: SumeragiConsensusMode::Npos, block_cadence_ms: nz(1000), wire_protocol_version: u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION), consensus_fingerprint: ConsensusFingerprint::new([0x71;32]),
            kagemusha_mint_finality: KagemushaMintFinalityGenesisParametersV1 { authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 { version:1,generation:0, validators:pasta(&keys,0) } },
            sumeragi_v2: iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
        };
        let mut instructions: Vec<InstructionBox> = validators(&keys)
            .into_iter()
            .map(|v| {
                RegisterPeerWithPop::new(PeerId::new(v.public_key), v.proof_of_possession).into()
            })
            .collect();
        instructions.push(
            SetParameter::new(Parameter::Custom(CustomParameter::new(
                consensus_metadata::handshake_meta_id(),
                iroha_primitives::json::Json::new(metadata),
            )))
            .into(),
        );
        let npos = SumeragiNposParameters {
            max_validators: 31,
            epoch_length_blocks: nz(ranges[0].1),
            evidence_horizon_blocks: 1,
            slashing_delay_blocks: 1,
            ..Default::default()
        };
        instructions
            .push(SetParameter::new(Parameter::Custom(npos.into_custom_parameter())).into());
        let mut tx = TransactionBuilder::new_genesis(
            AccountId::new(authority.public_key().clone()),
            FeePaymentIntent::authority(vec![], None),
        );
        tx.set_creation_time(Duration::from_millis(0));
        let tx = tx
            .with_instructions(instructions)
            .sign(authority.private_key());
        let genesis =
            SignedBlock::try_genesis(vec![tx], authority.private_key(), None, None).unwrap();
        let network = NetworkId::from_genesis_hash(genesis.hash());
        let context = genesis_epoch(&genesis).unwrap();
        let mut epochs = vec![Epoch { keys, context }];
        for (range, end) in &ranges[1..] {
            let keys = ordered_keys(range.clone());
            let previous = &epochs.last().unwrap().context;
            let retained = validators(&keys)
                .iter()
                .map(|v| &v.public_key)
                .eq(previous.committee.iter().map(|v| v.validator.public_key()));
            let generation = previous.authority.generation + u64::from(!retained);
            let authority = if retained {
                previous.authority.clone()
            } else {
                KagemushaMintFinalityAuthorityGenerationV1 {
                    version: 1,
                    network_id: network,
                    generation,
                    validators: pasta(&keys, generation),
                }
            };
            let authorization = KagemushaMintFinalityEpochAuthorizationV1 {
                epoch: previous.authorization.epoch + 1,
                first_height: previous.authorization.last_height + 1,
                last_height: *end,
                authority_generation: generation,
                authority_id: authority.authority_id().unwrap(),
                beacon: BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
                    session_id: [7; 32],
                    transcript_hash: [8; 32],
                }),
                previous_authorization_id: previous.authorization.authorization_id().unwrap(),
                transition_id: if retained {
                    [0; 32]
                } else {
                    [generation as u8; 32]
                },
                decision: if retained {
                    KagemushaMintFinalityEpochDecisionV1::Retain
                } else {
                    KagemushaMintFinalityEpochDecisionV1::Activate
                },
                ..previous.authorization
            };
            let committee = validators(&keys)
                .into_iter()
                .map(|v| ValidatorCommitteeMemberV1 {
                    validator: PeerId::new(v.public_key),
                    proof_of_possession: v.proof_of_possession,
                })
                .collect();
            epochs.push(Epoch {
                keys,
                context: ValidatorEpochContextV1 {
                    authority,
                    authorization,
                    committee,
                    leader_seed: [9; 32],
                    ..previous.clone()
                },
            });
        }
        let anchor = GenesisAnchor {
            network_id: network,
            chain_id: CHAIN.into(),
            genesis: genesis.clone(),
            validators: validators(&epochs[0].keys),
        };
        let mut chain = Self {
            anchor,
            epochs,
            proofs: vec![],
        };
        let mut native =
            SumeragiFinalityVerifier::new(&genesis, CHAIN, chain.anchor.validators.clone())
                .unwrap();
        for height in 1..=tip {
            let index = chain
                .epochs
                .iter()
                .position(|e| {
                    height >= e.context.authorization.first_height
                        && height <= e.context.authorization.last_height
                })
                .unwrap();
            let mut b = if height == 1 {
                genesis.clone()
            } else {
                let mut builder = BlockBuilder::new(BlockHeader::new(
                    nz(height),
                    Some(chain.proof(height - 1).block_header.hash()),
                    None,
                    height,
                    0,
                ));
                let mut tx = TransactionBuilder::new(
                    network,
                    AccountId::new(authority.public_key().clone()),
                    FeePaymentIntent::authority(vec![], None),
                );
                tx.set_creation_time(Duration::from_millis(height));
                builder.push_transaction(
                    tx.with_instructions([Log::new(
                        Level::INFO,
                        format!("native fixture {height}"),
                    )])
                    .sign(authority.private_key()),
                );
                builder.build(BTreeSet::new())
            };
            NativeFinalityFixture::install_network_results(&mut b, vec![Ok(Default::default())]);
            let context = chain.epochs[index].context.clone();
            let parent = height
                .checked_sub(1)
                .filter(|h| *h > 0)
                .map(|h| chain.proof(h).clone());
            let parent_hash = parent
                .as_ref()
                .map(|p| {
                    if p.height() == 1 {
                        Hash32(Hash::from(p.block_header.hash()).into())
                    } else {
                        chain_hash(&block_hash_preimage(&core(p)))
                    }
                })
                .unwrap_or(Hash32([1; 32]));
            let parent_result = parent
                .as_ref()
                .map(|p| result(p).result().unwrap())
                .unwrap_or(Hash32([1; 32]));
            let boundary = if height == context.authorization.last_height {
                let pulse = result(parent.as_ref().unwrap()).beacon.unwrap();
                chain.epochs[index + 1].context.leader_seed =
                    global_threshold_beacon_npos_successor_seed_v1(
                        &pulse,
                        height,
                        context.authorization.epoch + 1,
                    );
                Some(ValidatorEpochBoundaryV1 {
                    version: 1,
                    height,
                    predecessor_context_id: context.context_id().unwrap(),
                    selection_anchor: parent.as_ref().unwrap().block_header.hash(),
                    next: chain.epochs[index + 1].context.clone(),
                    preparation: None,
                })
            } else {
                None
            };
            let authorized = boundary.as_ref().map_or(&context, |b| &b.next);
            let params = ChainParamsRecord::from_core(&ChainParams::default());
            let slot = |h| {
                if h <= authorized.authorization.last_height {
                    ScheduledSlot::Ready(ScheduledConfig {
                        height: h,
                        epoch: authorized.clone(),
                        params,
                    })
                } else {
                    ScheduledSlot::PendingBoundary {
                        height: h,
                        boundary_height: authorized.authorization.last_height,
                        predecessor_context_id: authorized.context_id().unwrap(),
                        params,
                    }
                }
            };
            let schedule = ScheduleOutcome {
                height,
                current: context.clone(),
                next: slot(height + 1),
                after_next: slot(height + 2),
                boundary,
            };
            let beacon = if height > 1 && height + 1 == context.authorization.last_height {
                // The QC certifies this synthetic execution's public pulse. Threshold session/DKG
                // execution is deliberately outside this portable-finality fixture's claim.
                let beacon_key = KeyPair::from_seed(vec![77; 32], Algorithm::BlsSmall);
                let mut pulse = FinalizedGlobalThresholdBeaconPulseV1 {
                    version: 1,
                    network_id: network,
                    session_id: [7; 32],
                    roster_hash: [6; 32],
                    transcript_hash: [8; 32],
                    context: GlobalThresholdBeaconPulseContextV1 {
                        instance: native.instance().0,
                        epoch: context.authorization.epoch,
                        epoch_context_id: context.context_id().unwrap(),
                        parent_consensus_hash: parent_hash.0,
                        parent_result: parent_result.0,
                    },
                    height,
                    round: 0,
                    finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
                        height: height - 1,
                        block_hash: parent.as_ref().unwrap().block_header.hash(),
                    },
                    signature: [0; 48],
                    seed: [0; 32],
                    pulse_id: [0; 32],
                };
                pulse.signature = Signature::try_new(
                    beacon_key.private_key(),
                    &global_threshold_beacon_pulse_payload_v1(&pulse),
                )
                .unwrap()
                .payload()
                .try_into()
                .unwrap();
                pulse.seed = Hash::new(pulse.signature).into();
                pulse.pulse_id = global_threshold_beacon_pulse_id_v1(&pulse, pulse.seed);
                Some(pulse)
            } else {
                None
            };
            let (len, hash) = b.executed_block_wire_identity().unwrap();
            let (native_lanes, ordinary_root) =
                NativeLaneStateProof::empty_for_testing(network, height);
            let commitment = ExecutionResultCommitment::new(
                height,
                ExecutionCommitment {
                    parent_state_root: Hash::new(b"synthetic parent"),
                    post_state_root: ordinary_root,
                    ordinary_writes_root: ordinary_root,
                    kagemusha_top_up_root: None,
                    kagemusha_top_up_count: 0,
                    executed_block_wire_len: len,
                    executed_block_wire_hash: hash,
                    transaction_input_commitment: b.network_input_merkle_commitment(),
                    transaction_output_commitment: b.output_merkle_commitment(),
                },
                schedule,
                beacon,
                native_lanes,
            )
            .unwrap();
            let certificate = if height == 1 {
                CommitCertificate::from_untrusted_parts(
                    vec![],
                    vec![],
                    commitment.preimage().unwrap(),
                )
            } else {
                let payload = b.canonical_resultless_proposal().encode_wire().unwrap();
                let header = CoreHeader {
                    instance: native.instance(),
                    epoch: core_epoch(&context).unwrap().id,
                    height,
                    origin_view: 0,
                    parent_hash,
                    parent_result,
                    payload_hash: Hash32(Hash::new_from_chunks(&[TAG_PAY, &payload]).into()),
                    payload_len: payload.len().try_into().unwrap(),
                    proposer: 0,
                    skipped_leaders: vec![],
                    control_witness: ControlWitness::empty(),
                    attest: commitment.schedule.boundary.is_some(),
                };
                let keys = &chain.epochs[index].keys;
                let q = CommitteeSize::new(keys.len()).unwrap().quorum();
                let mut qc = Qc {
                    kind: VoteKind::Commit,
                    instance: header.instance,
                    epoch: header.epoch,
                    height,
                    view: 0,
                    block_hash: chain_hash(&block_hash_preimage(&header)),
                    result: commitment.result().unwrap(),
                    attest: header.attest,
                    signers: Bitmap::new(keys.len()),
                    agg_sig: AggregateSignature([0; 96]),
                    attestations: vec![],
                    attestation_witness: None,
                };
                if qc.attest {
                    // Application-specific Pasta seal verification is not performed by this verifier.
                    qc.attestations =
                        vec![
                            AttestationSignature::try_from_slice(b"synthetic application seal")
                                .unwrap();
                            q
                        ];
                    qc.attestation_witness = Some(
                        ResultWitness::from_untrusted(commitment.preimage().unwrap()).unwrap(),
                    );
                }
                sign_qc(&mut qc, keys, &(0..q as u32).collect::<Vec<_>>());
                CommitCertificate::from_untrusted_parts(
                    norito::encode_canonical(&header).unwrap(),
                    norito::encode_canonical(&qc).unwrap(),
                    commitment.preimage().unwrap(),
                )
            };
            b.set_commit_certificate(Some(certificate));
            let proof = SumeragiFinalityProof {
                block_header: b.header(),
                block_wire: b.encode_wire().unwrap(),
                committee: validators(&chain.epochs[index].keys),
            };
            native.verify(&proof).unwrap();
            chain.proofs.push(proof);
        }
        chain
    }
    fn proof(&self, height: u64) -> &SumeragiFinalityProof {
        &self.proofs[height as usize - 1]
    }
    fn epoch(&self, height: u64) -> &Epoch {
        self.epochs
            .iter()
            .find(|e| {
                height >= e.context.authorization.first_height
                    && height <= e.context.authorization.last_height
            })
            .unwrap()
    }
    fn verifier(&self) -> FinalityVerifier {
        FinalityVerifier::from_genesis(&self.anchor, self.proof(1)).unwrap()
    }
    fn verifier_at(&self, height: u64) -> FinalityVerifier {
        let mut v = self.verifier();
        v.advance(&Source::new(self), self.proof(height)).unwrap();
        v
    }
    fn attest(&self, k: &KeyPair, height: u64) -> SumeragiFinalityAttestation {
        let body = SumeragiFinalityAttestationBody {
            challenge: CHALLENGE,
            network_id: self.anchor.network_id,
            node_id: peer(k),
            node_fingerprint: Hash::new(peer(k).encode()),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            genesis_block_hash: self.anchor.genesis.hash(),
            genesis_finality_proof: self.proof(1).clone(),
            status: SumeragiStatus {
                protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
                config_fingerprint: Hash::new(b"config"),
                beacon_horizon: None,
                instance: SumeragiFinalityVerifier::new(
                    &self.anchor.genesis,
                    CHAIN,
                    self.anchor.validators.clone(),
                )
                .unwrap()
                .instance()
                .0,
                height: height + 1,
                view: 0,
                stage: 0,
                leader: None,
                proxy_tail: None,
                high_qc_view: None,
                level: 0,
                start_level: 0,
                t_retx_ms: 100,
                committed_height: height,
                applied_height: height,
                awaiting: false,
                signer: Some(k.public_key().clone()),
                unanchored: false,
                abstaining: false,
                halted: None,
                footprint: Default::default(),
            },
            finality_proof: self.proof(height).clone(),
        };
        let signature = SignatureOf::try_from_hash(k.private_key(), body.signing_hash()).unwrap();
        SumeragiFinalityAttestation { body, signature }
    }
    fn alternate(&self, height: u64) -> SumeragiFinalityProof {
        let mut p = self.proof(height).clone();
        let h = core(&p);
        let r = result(&p);
        let mut qc: Qc =
            norito::decode_canonical(block(&p).commit_certificate().unwrap().commit_qc()).unwrap();
        let keys = &self.epoch(height).keys;
        let q = CommitteeSize::new(keys.len()).unwrap().quorum();
        sign_qc(&mut qc, keys, &(1..=q as u32).collect::<Vec<_>>());
        replace_certificate(&mut p, &h, &qc, &r);
        p
    }
}
struct Source<'a> {
    chain: &'a Chain,
    proofs: RefCell<BTreeMap<u64, SumeragiFinalityProof>>,
    tips: BTreeMap<PeerId, u64>,
    faults: BTreeSet<PeerId>,
    substitutions: BTreeMap<PeerId, PeerId>,
    attestation_overrides: BTreeMap<PeerId, SumeragiFinalityAttestation>,
    proof_calls: RefCell<Vec<u64>>,
    reads: RefCell<Vec<PeerId>>,
}
impl<'a> Source<'a> {
    fn new(chain: &'a Chain) -> Self {
        Self {
            chain,
            proofs: RefCell::new(BTreeMap::new()),
            tips: BTreeMap::new(),
            faults: BTreeSet::new(),
            substitutions: BTreeMap::new(),
            attestation_overrides: BTreeMap::new(),
            proof_calls: RefCell::new(vec![]),
            reads: RefCell::new(vec![]),
        }
    }
}
impl FinalitySource for Source<'_> {
    type Error = std::io::Error;
    fn finality_proof(&self, height: NonZeroU64) -> Result<SumeragiFinalityProof, Self::Error> {
        self.proof_calls.borrow_mut().push(height.get());
        Ok(self
            .proofs
            .borrow()
            .get(&height.get())
            .unwrap_or_else(|| self.chain.proof(height.get()))
            .clone())
    }
    fn latest_attestation(
        &self,
        p: &PeerId,
        c: &[u8; 32],
    ) -> Result<SumeragiFinalityAttestation, Self::Error> {
        assert_eq!(*c, CHALLENGE);
        self.reads.borrow_mut().push(p.clone());
        if self.faults.contains(p) {
            return Err(std::io::Error::other("offline"));
        }
        if let Some(attestation) = self.attestation_overrides.get(p) {
            return Ok(attestation.clone());
        }
        let actual = self.substitutions.get(p).unwrap_or(p);
        let k = self
            .chain
            .epochs
            .iter()
            .flat_map(|e| &e.keys)
            .find(|k| peer(k) == *actual)
            .unwrap();
        Ok(self.chain.attest(
            k,
            *self
                .tips
                .get(p)
                .unwrap_or(&(self.chain.proofs.len() as u64)),
        ))
    }
}
fn resign(a: &mut SumeragiFinalityAttestation, k: &KeyPair) {
    a.signature = SignatureOf::try_from_hash(k.private_key(), a.body.signing_hash()).unwrap();
}

#[test]
fn committee_size_accepts_exact_3f_plus_1_from_4_to_128() {
    for n in 0..=140 {
        assert_eq!(
            CommitteeSize::new(n).is_ok(),
            (4..=128).contains(&n) && (n - 1) % 3 == 0
        );
    }
    for n in SIZES {
        let s = CommitteeSize::new(n).unwrap();
        assert_eq!(s.members(), n);
        assert_eq!(s.quorum(), 2 * s.faults() + 1);
    }
}
#[test]
fn genesis_anchor_pins_signed_body_ordered_roster_network_and_chain() {
    for size in SIZES {
        let chain = Chain::constant(size, 2);
        let v = chain.verifier();
        assert_eq!(v.committee_size().members(), size);
        assert_eq!(v.checkpoint().height(), 1);
        assert_eq!(v.checkpoint().network_id(), chain.anchor.network_id);
        assert_eq!(v.checkpoint().chain_id(), CHAIN);
        let mut bad = chain.anchor.clone();
        bad.validators.swap(0, 1);
        assert!(FinalityVerifier::from_genesis(&bad, chain.proof(1)).is_err());
        let mut bad = chain.anchor.clone();
        bad.validators[0].proof_of_possession[0] ^= 1;
        assert!(FinalityVerifier::from_genesis(&bad, chain.proof(1)).is_err());
        let mut bad = chain.anchor.clone();
        bad.network_id =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
        assert!(matches!(
            FinalityVerifier::from_genesis(&bad, chain.proof(1)),
            Err(FinalityError::WrongNetwork { .. })
        ));
        assert!(FinalityVerifier::from_genesis(&chain.anchor, chain.proof(2)).is_err());
    }
}
#[test]
fn advance_within_epoch_fetches_every_missing_successor() {
    for size in SIZES {
        let c = Chain::constant(size, 6);
        let s = Source::new(&c);
        let mut v = c.verifier();
        assert_eq!(v.advance(&s, c.proof(6)).unwrap(), 4);
        assert_eq!(*s.proof_calls.borrow(), [2, 3, 4, 5]);
        assert_eq!(v.checkpoint().height(), 6);
    }
}
#[test]
fn advance_is_idempotent_and_never_moves_backward() {
    let c = Chain::constant(4, 4);
    let s = Source::new(&c);
    let mut v = c.verifier_at(4);
    let cp = v.checkpoint().clone();
    assert_eq!(v.advance(&s, c.proof(4)).unwrap(), 0);
    assert!(s.proof_calls.borrow().is_empty());
    assert!(matches!(
        v.advance(&s, c.proof(3)),
        Err(FinalityError::StaleTip { .. })
    ));
    assert_eq!(*v.checkpoint(), cp);
    assert_eq!(v.advance(&s, &c.alternate(4)).unwrap(), 0);
}
#[test]
fn advance_verifies_contiguous_four_seven_four_and_non_four_committees() {
    let c = Chain::new(&[(0..4, 3), (0..7, 6), (3..7, 9)], 8);
    let s = Source::new(&c);
    let mut v = c.verifier();
    assert_eq!(v.advance(&s, c.proof(8)).unwrap(), 6);
    assert_eq!(*s.proof_calls.borrow(), [2, 3, 4, 5, 6, 7]);
    assert_eq!(v.committee_size().members(), 4);
    let c = Chain::new(&[(0..7, 3), (0..10, 6), (0..7, 9)], 8);
    assert_eq!(c.verifier_at(8).committee_size().members(), 7);
}
#[test]
fn retained_generations_advance_scheduling_epoch_without_key_substitution() {
    let c = Chain::new(&[(0..4, 3), (0..4, 6), (0..4, 9)], 8);
    assert_eq!(c.epochs[0].context.authority, c.epochs[2].context.authority);
    assert_ne!(
        c.epochs[0].context.context_id().unwrap(),
        c.epochs[2].context.context_id().unwrap()
    );
    assert_eq!(c.verifier_at(8).checkpoint().height(), 8);
}
#[test]
fn advance_rejects_missing_or_reordered_successors_atomically() {
    let c = Chain::constant(4, 5);
    for actual in [1, 3, 4] {
        let s = Source::new(&c);
        s.proofs.borrow_mut().insert(2, c.proof(actual).clone());
        let mut v = c.verifier();
        let cp = v.checkpoint().clone();
        assert!(matches!(
            v.advance(&s, c.proof(5)),
            Err(FinalityError::UnexpectedHeight { expected: 2, .. })
        ));
        assert_eq!(*v.checkpoint(), cp);
    }
}
#[test]
fn exact_quorum_requires_distinct_members_and_valid_possession_proofs() {
    for size in SIZES {
        let c = Chain::constant(size, 2);
        let q = CommitteeSize::new(size).unwrap().quorum();
        for count in [q - 1, q + 1] {
            let mut p = c.proof(2).clone();
            let h = core(&p);
            let r = result(&p);
            let mut qc: Qc =
                norito::decode_canonical(block(&p).commit_certificate().unwrap().commit_qc())
                    .unwrap();
            sign_qc(
                &mut qc,
                &c.epoch(2).keys,
                &(0..count as u32).collect::<Vec<_>>(),
            );
            replace_certificate(&mut p, &h, &qc, &r);
            assert!(c.verifier().advance(&Source::new(&c), &p).is_err());
        }
        let mut p = c.proof(2).clone();
        p.committee[1] = p.committee[0].clone();
        assert!(c.verifier().advance(&Source::new(&c), &p).is_err());
        let mut p = c.proof(2).clone();
        p.committee[0].proof_of_possession[0] ^= 1;
        assert!(c.verifier().advance(&Source::new(&c), &p).is_err());
        let mut p = c.proof(2).clone();
        p.committee.swap(0, 1);
        assert!(c.verifier().advance(&Source::new(&c), &p).is_err());
        assert!(Bitmap::from_indices(size, [size as u32]).is_none());
    }
}
#[test]
fn advance_rejects_mutated_proofs_and_keeps_original_checkpoint() {
    let c = Chain::constant(4, 4);
    for mutation in 0..7 {
        let mut p = c.proof(4).clone();
        let mut h = core(&p);
        let mut r = result(&p);
        let mut qc: Qc =
            norito::decode_canonical(block(&p).commit_certificate().unwrap().commit_qc()).unwrap();
        match mutation {
            0 => h.parent_result.0[0] ^= 1,
            1 => h.instance.0[0] ^= 1,
            2 => h.epoch.context.0[0] ^= 1,
            3 => qc.agg_sig.0[0] ^= 1,
            4 => qc.result.0[0] ^= 1,
            5 => r.execution.parent_state_root = Hash::new(b"changed"),
            _ => h.payload_hash.0[0] ^= 1,
        }
        replace_certificate(&mut p, &h, &qc, &r);
        let mut v = c.verifier();
        let cp = v.checkpoint().clone();
        assert!(
            v.advance(&Source::new(&c), &p).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(*v.checkpoint(), cp);
    }
}
#[test]
fn altered_boundary_cannot_select_a_foreign_committee_or_freshness_seed() {
    let c = Chain::new(&[(0..4, 3), (0..7, 6), (3..7, 9)], 5);
    for mutation in 0..3 {
        let mut p = c.proof(3).clone();
        let h = core(&p);
        let mut r = result(&p);
        let mut qc: Qc =
            norito::decode_canonical(block(&p).commit_certificate().unwrap().commit_qc()).unwrap();
        let b = r.schedule.boundary.as_mut().unwrap();
        match mutation {
            0 => b.next.leader_seed[0] ^= 1,
            1 => b.selection_anchor = HashOf::from_untyped_unchecked(Hash::new(b"wrong parent")),
            _ => b.predecessor_context_id[0] ^= 1,
        }
        // Re-sign the entire changed result. A genuine incumbent quorum is still insufficient
        // to bypass deterministic epoch/parent/pulse bindings.
        if let Some(b) = &r.schedule.boundary {
            for slot in [&mut r.schedule.next, &mut r.schedule.after_next] {
                if let ScheduledSlot::Ready(s) = slot {
                    s.epoch = b.next.clone();
                }
            }
        }
        qc.result = r.result().unwrap();
        sign_qc(&mut qc, &c.epoch(3).keys, &[0, 1, 2]);
        replace_certificate(&mut p, &h, &qc, &r);
        let s = Source::new(&c);
        s.proofs.borrow_mut().insert(3, p);
        let mut v = c.verifier();
        let cp = v.checkpoint().clone();
        assert!(v.advance(&s, c.proof(5)).is_err());
        assert_eq!(*v.checkpoint(), cp);
    }
}
#[test]
fn complete_native_checkpoint_roundtrips_and_requires_independent_network_and_chain() {
    let c = Chain::new(&[(0..4, 3), (0..7, 6), (3..7, 9)], 7);
    for h in 1..=7 {
        let v = c.verifier_at(h);
        let bytes = v.checkpoint().encode_canonical().unwrap();
        let cp = SumeragiFinalityCheckpoint::decode_canonical(&bytes).unwrap();
        assert_eq!(&cp, v.checkpoint());
        let resumed =
            FinalityVerifier::from_checkpoint(cp.clone(), c.anchor.network_id, CHAIN).unwrap();
        assert_eq!(resumed, v);
        assert!(
            FinalityVerifier::from_checkpoint(cp.clone(), c.anchor.network_id, "foreign-chain")
                .is_err()
        );
        assert!(
            FinalityVerifier::from_checkpoint(
                cp,
                NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign"))),
                CHAIN
            )
            .is_err()
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(SumeragiFinalityCheckpoint::decode_canonical(&trailing).is_err());
        assert!(SumeragiFinalityCheckpoint::decode_canonical(&bytes[..bytes.len() - 1]).is_err());
    }
}
#[test]
fn restart_retains_exact_previous_decision_but_refuses_older_prefix_claims() {
    let c = Chain::constant(4, 5);
    let cp = c.verifier_at(5).checkpoint().clone();
    let v = FinalityVerifier::from_checkpoint(cp, c.anchor.network_id, CHAIN).unwrap();
    let k = &c.epoch(5).keys[0];
    assert!(v.verify_attestation(&CHALLENGE, &c.attest(k, 4)).is_ok());
    assert!(matches!(
        v.verify_attestation(&CHALLENGE, &c.attest(k, 3)),
        Err(FinalityError::OutsideRetainedPrefix { .. })
    ));
}
#[test]
fn proof_count_and_byte_budgets_refuse_without_publishing_progress() {
    let c = Chain::constant(4, 4);
    let mut v = c.verifier();
    let cp = v.checkpoint().clone();
    let s = Source::new(&c);
    let mut claimed = c.proof(4).clone();
    claimed.block_header = BlockHeader::new(nz(u64::MAX), Some(cp.block_hash()), None, 4, 0);
    assert!(matches!(
        v.advance(&s, &claimed),
        Err(FinalityError::ResourceLimit("proof count"))
    ));
    assert!(s.proof_calls.borrow().is_empty());
    let mut budget = Budget {
        proofs: 4,
        bytes: c.proof(2).block_wire.len(),
    };
    assert!(matches!(
        v.advance_with_budget(&s, c.proof(4), &mut budget),
        Err(FinalityError::ResourceLimit("proof bytes"))
    ));
    assert_eq!(*v.checkpoint(), cp);
}
#[test]
fn attestations_require_exactly_distinct_current_members() {
    for size in SIZES {
        let c = Chain::constant(size, 3);
        let v = c.verifier_at(3);
        let q = v.committee_size().quorum();
        let all: Vec<_> = c.epoch(3).keys.iter().map(|k| c.attest(k, 3)).collect();
        assert_eq!(
            v.attestation_quorum(&CHALLENGE, &all).unwrap().verified(),
            size
        );
        assert!(v.attestation_quorum(&CHALLENGE, &all[..q - 1]).is_err());
        let mut duplicated = all[..q - 1].to_vec();
        duplicated.extend(vec![all[0].clone(); size]);
        assert!(v.attestation_quorum(&CHALLENGE, &duplicated).is_err());
        assert_eq!(
            v.attestation_quorum(&CHALLENGE, &all[..q])
                .unwrap()
                .verified(),
            q
        );
        let outsider = c.attest(&key(100), 3);
        assert!(matches!(
            v.verify_attestation(&CHALLENGE, &outsider),
            Err(FinalityError::NotInCommittee { .. })
        ));
    }
}
#[test]
fn attestations_bind_challenge_signature_network_status_and_runtime_identity() {
    let c = Chain::constant(4, 3);
    let v = c.verifier_at(3);
    let k = &c.epoch(3).keys[0];
    for mutation in 0..9 {
        let mut a = c.attest(k, 3);
        match mutation {
            0 => a.body.challenge = [8; 32],
            1 => {
                a.body.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"foreign"),
                ))
            }
            2 => a.body.status.applied_height = 2,
            3 => a.body.status.committed_height = 2,
            4 => a.body.status.instance[0] ^= 1,
            5 => a.body.node_fingerprint = Hash::new(b"other"),
            6 => a.body.status.signer = Some(key(100).public_key().clone()),
            7 => a.body.genesis_block_hash = HashOf::from_untyped_unchecked(Hash::new(b"foreign")),
            _ => a.body.config_fingerprint = Hash::new(b"changed after signing"),
        };
        if mutation != 8 {
            resign(&mut a, k);
        }
        assert!(
            v.verify_attestation(&CHALLENGE, &a).is_err(),
            "mutation {mutation}"
        );
    }
    assert!(matches!(
        v.verify_attestation(&[0; 32], &c.attest(k, 3)),
        Err(FinalityError::ZeroChallenge)
    ));
}
#[test]
fn independent_certificate_witnesses_attest_same_authenticated_decision() {
    let c = Chain::constant(7, 4);
    let v = c.verifier_at(4);
    for h in [3, 4] {
        let k = &c.epoch(4).keys[0];
        let mut a = c.attest(k, h);
        a.body.finality_proof = c.alternate(h);
        resign(&mut a, k);
        assert!(v.verify_attestation(&CHALLENGE, &a).is_ok());
    }
}
#[test]
fn genuinely_signed_conflicting_result_is_rejected_even_one_block_behind() {
    let c = Chain::constant(4, 5);
    let v = c.verifier_at(5);
    for height in [4, 5] {
        let mut p = c.proof(height).clone();
        let h = core(&p);
        let mut r = result(&p);
        r.execution.parent_state_root = Hash::new(b"conflicting execution");
        let mut qc: Qc =
            norito::decode_canonical(block(&p).commit_certificate().unwrap().commit_qc()).unwrap();
        qc.result = r.result().unwrap();
        sign_qc(&mut qc, &c.epoch(height).keys, &[0, 1, 2]);
        replace_certificate(&mut p, &h, &qc, &r);
        assert!(p.decode_checked().is_ok());
        let k = &c.epoch(5).keys[0];
        let mut a = c.attest(k, height);
        a.body.finality_proof = p;
        resign(&mut a, k);
        assert!(v.verify_attestation(&CHALLENGE, &a).is_err());
    }
}
#[test]
fn future_attestation_requires_contiguous_advance() {
    let c = Chain::constant(4, 5);
    let v = c.verifier_at(3);
    assert!(matches!(
        v.verify_attestation(&CHALLENGE, &c.attest(&c.epoch(5).keys[0], 5)),
        Err(FinalityError::AheadOfCheckpoint { .. })
    ));
}
#[test]
fn observe_reads_every_member_and_promotes_only_fresh_quorum() {
    for size in SIZES {
        let c = Chain::constant(size, 4);
        let mut v = c.verifier();
        let s = Source::new(&c);
        let q = v.observe(&s, &CHALLENGE).unwrap();
        assert_eq!(q.verified(), size);
        assert_eq!(s.reads.borrow().len(), size);
        assert_eq!(*s.proof_calls.borrow(), [2, 3]);
        assert_eq!(v.checkpoint().height(), 4);
    }
}
#[test]
fn observe_follows_authenticated_rotation_and_counts_members_just_behind_boundary() {
    let c = Chain::new(&[(0..4, 3), (0..7, 6), (3..7, 9)], 7);
    let mut v = c.verifier();
    let s = Source::new(&c);
    assert_eq!(v.observe(&s, &CHALLENGE).unwrap().verified(), 4);
    assert_eq!(v.checkpoint().height(), 7);
    assert_eq!(
        s.reads.borrow().iter().collect::<BTreeSet<_>>().len(),
        s.reads.borrow().len()
    );
    let c = Chain::new(&[(0..4, 3), (0..7, 6), (3..7, 9)], 4);
    let mut v = c.verifier_at(3);
    let mut s = Source::new(&c);
    for k in &c.epoch(4).keys[..3] {
        s.tips.insert(peer(k), 3);
    }
    assert_eq!(v.observe(&s, &CHALLENGE).unwrap().verified(), 7);
    assert_eq!(v.checkpoint().height(), 4);
}
#[test]
fn observe_tolerates_f_faults_and_rolls_back_with_f_plus_one() {
    for size in SIZES {
        let c = Chain::constant(size, 4);
        let f = CommitteeSize::new(size).unwrap().faults();
        for faults in [f, f + 1] {
            let mut s = Source::new(&c);
            s.faults.extend(c.epoch(4).keys[..faults].iter().map(peer));
            let mut v = c.verifier();
            let cp = v.checkpoint().clone();
            let result = v.observe(&s, &CHALLENGE);
            if faults == f {
                assert_eq!(result.unwrap().verified(), size - f);
                assert_eq!(v.checkpoint().height(), 4);
            } else {
                assert!(matches!(
                    result,
                    Err(FinalityError::InsufficientAttestations(_))
                ));
                assert_eq!(*v.checkpoint(), cp);
            }
        }
    }
}
#[test]
fn observe_rejects_zero_challenge_and_substituted_nodes_without_advancing() {
    let c = Chain::constant(4, 4);
    let mut v = c.verifier();
    let cp = v.checkpoint().clone();
    let mut s = Source::new(&c);
    assert!(matches!(
        v.observe(&s, &[0; 32]),
        Err(FinalityError::ZeroChallenge)
    ));
    assert!(s.reads.borrow().is_empty());
    for k in &c.epoch(4).keys {
        s.substitutions.insert(peer(k), peer(&c.epoch(4).keys[0]));
    }
    assert!(v.observe(&s, &CHALLENGE).is_err());
    assert_eq!(*v.checkpoint(), cp);
}
#[test]
fn invalid_higher_tip_does_not_block_a_valid_lower_quorum() {
    let c = Chain::constant(4, 5);
    let mut s = Source::new(&c);
    for k in &c.epoch(4).keys {
        s.tips.insert(peer(k), 4);
    }
    s.tips.insert(peer(&c.epoch(4).keys[0]), 5);
    // H5 is supplied by one peer but its intermediate H4 source response is reordered.
    // The remaining peers' direct H4 tips still extend H1 via genuine H2/H3.
    s.proofs.borrow_mut().insert(4, c.proof(3).clone());
    let mut v = c.verifier();
    assert_eq!(v.observe(&s, &CHALLENGE).unwrap().verified(), 3);
    assert_eq!(v.checkpoint().height(), 4);
}

#[test]
fn observation_keeps_original_verified_responses_across_boundary_reads() {
    for members in SIZES {
        let chain = Chain::new(&[(0..members, 3), (0..members, 20)], 5);
        let nodes = &chain.epoch(5).keys;
        let behind = CommitteeSize::new(members).unwrap().faults() + 1;
        let mut source = Source::new(&chain);
        source.tips.insert(peer(&nodes[0]), 2);
        for node in &nodes[1..behind] {
            source.tips.insert(peer(node), 3);
        }
        let mut verifier = chain.verifier();
        let report = verifier.observe(&source, &CHALLENGE).unwrap();
        assert_eq!(report.height.get(), 5);
        assert_eq!(report.verified(), members);
        assert_eq!(*source.proof_calls.borrow(), [2, 3, 4]);
        let mut heights = report
            .peers
            .iter()
            .filter_map(|(_, result)| match result {
                AttestationOutcome::Verified(tip) => Some(tip.height.get()),
                _ => None,
            })
            .collect::<Vec<_>>();
        heights.sort_unstable();
        assert_eq!(heights[0], 2);
        assert!(heights[1..behind].iter().all(|h| *h == 3));
        assert!(heights[behind..].iter().all(|h| *h == 5));
    }
}

#[test]
fn observation_does_not_promote_a_signed_conflict_from_an_earlier_verified_position() {
    let chain = Chain::new(&[(0..4, 3), (0..4, 20)], 5);
    let mut source = Source::new(&chain);
    let key = &chain.epoch(5).keys[0];
    let mut attestation = chain.attest(key, 2);
    let proof = &mut attestation.body.finality_proof;
    let header = core(proof);
    let mut commitment = result(proof);
    commitment.execution.parent_state_root = Hash::new(b"conflicting early result");
    let mut qc: Qc =
        norito::decode_canonical(block(proof).commit_certificate().unwrap().commit_qc()).unwrap();
    qc.result = commitment.result().unwrap();
    sign_qc(&mut qc, &chain.epoch(2).keys, &[0, 1, 2]);
    replace_certificate(proof, &header, &qc, &commitment);
    assert!(proof.decode_checked().is_ok());
    resign(&mut attestation, key);
    source.attestation_overrides.insert(peer(key), attestation);
    let report = chain.verifier().observe(&source, &CHALLENGE).unwrap();
    assert_eq!(report.verified(), 3);
    assert!(matches!(
        report
            .peers
            .iter()
            .find(|(p, _)| *p == peer(key))
            .unwrap()
            .1,
        AttestationOutcome::Rejected(_)
    ));
}

#[test]
fn observation_rejects_responses_older_than_the_previous_epoch() {
    let chain = Chain::new(&[(0..4, 3), (0..4, 6), (0..4, 20)], 8);
    let mut source = Source::new(&chain);
    source.tips.insert(peer(&chain.epoch(8).keys[0]), 2);
    let report = chain.verifier().observe(&source, &CHALLENGE).unwrap();
    assert_eq!(report.verified(), 3);
}

#[test]
fn native_result_decode_bounds_complete_committee_graphs_and_preserves_outer_limits() {
    for size in [4, 7, 10, 31] {
        let chain = Chain::constant(size, 2);
        let proof = chain.proof(2);
        let carrier = block(proof);
        let bytes = carrier.commit_certificate().unwrap().result_preimage();
        let decoded = ExecutionResultCommitment::decode(bytes).unwrap();
        assert_eq!(decoded.schedule.current.committee.len(), size);
        assert_eq!(decoded.preimage().unwrap(), bytes);
        assert!(ExecutionResultCommitment::decode(&bytes[..bytes.len() - 1]).is_err());
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(ExecutionResultCommitment::decode(&trailing).is_err());
        let outer_refusal = norito::with_decode_limits(
            norito::DecodeLimits::new(96, bytes.len(), 8192, 0, 32),
            || Ok(ExecutionResultCommitment::decode(bytes)),
        );
        assert!(
            matches!(outer_refusal, Err(_) | Ok(Err(_))),
            "inner canonical policy cannot replace the caller's exhausted budget"
        );
    }
    let oversized = vec![0; iroha_data_model::sumeragi_finality::MAX_RESULT_PREIMAGE_BYTES + 1];
    assert!(matches!(
        ExecutionResultCommitment::decode(&oversized),
        Err(iroha_data_model::sumeragi_finality::CommitmentError::PreimageLength(_))
    ));
    // A boundary repeats complete target credentials in its exact atomic successor slots.
    let chain = Chain::new(&[(0..4, 3), (0..31, 6), (0..4, 9)], 5);
    let mut verifier = chain.verifier();
    verifier
        .advance(&Source::new(&chain), chain.proof(5))
        .unwrap();
    assert_eq!(verifier.committee_size().members(), 31);
}
