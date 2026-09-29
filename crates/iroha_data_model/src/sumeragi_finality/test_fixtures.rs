//! Genuine native BLS proof fixtures; caller-supplied results are synthetic, never executed World.
use super::*;
use crate::{
    account::AccountId,
    block::{
        CommitCertificate,
        builder::BlockBuilder,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        output_budget::ExecutionOutputLimits,
    },
    isi::{
        InstructionBox, Log, RegisterPeerWithPop, SetParameter,
        kagemusha_v1::{
            KagemushaMintFinalityAuthorityGenerationTemplateV1,
            KagemushaMintFinalityGenesisParametersV1, KagemushaMintFinalityValidatorKeysV1,
        },
    },
    level::Level,
    parameter::{
        CustomParameter, Parameter,
        system::{
            ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode,
            consensus_metadata,
        },
    },
    sumeragi::epoch::ValidatorEpochContextV1,
    transaction::{FeePaymentIntent, TransactionBuilder, TransactionResultInner},
};
use iroha_crypto::{KeyPair, bls_normal_pop_prove};
use iroha_sumeragi::types::{Bitmap, ChainParams};
use std::{collections::BTreeSet, num::NonZeroU64, time::Duration};

// Existing reviewed public multiples 1..4 of the Pasta generator; no private monetary keys.
const PALLAS: [&str; 4] = [
    "00000000ed302d991bf94c09fc98462200000000000000000000000000000040",
    "030000b067c50313fcac1144eee2fe0e0000000000000000000000000000001c",
    "63d232eb3b8af0b75cfcf55ade47f6ff4cdf4e47a7454cb8ed67a9ba6f56e788",
    "fc86bc8efbbcb878f49427618b6940409b9157e3d777a4c4c0514a8e0d92db18",
];
const VESTA: [&str; 4] = [
    "0000000021eb468cdda89409fc98462200000000000000000000000000000040",
    "03000070de065fede0093144eee2fe0e0000000000000000000000000000001c",
    "5fce556feb6fee5a15560ddabae10224b026a5d0281af4c613955c39a8797837",
    "f79037a77e26a2c0794dc326d866c664616499c064073a8f8ebf3080297be5ab",
];

/// A deterministic signed genesis and genuine exact-quorum native proof chain for tests.
///
/// Certifying supplied results is not World execution or monetary qualification. The public
/// fixed fixture keys must never be used as a deployment's trust root or signing credentials.
#[derive(Clone)]
pub struct NativeFinalityFixture {
    genesis: SignedBlock,
    first: SumeragiFinalityProof,
    tip: SumeragiFinalityProof,
    keys: Vec<KeyPair>,
    validators: Vec<FinalityValidator>,
    epoch: ValidatorEpochContextV1,
    verifier: SumeragiFinalityVerifier,
    chain_id: String,
}

impl Default for NativeFinalityFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeFinalityFixture {
    /// Construct a genesis prefix and one real three-of-four native H2 certificate.
    #[must_use]
    pub fn new() -> Self {
        let mut fixture = Self::start("portable-native-fixture");
        let signer = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let mut tx = TransactionBuilder::new(
            fixture.network_id(),
            AccountId::new(signer.public_key().clone()),
            FeePaymentIntent::authority(vec![], None),
        );
        tx.set_creation_time(Duration::from_millis(1));
        let tx = tx
            .with_instructions([Log::new(Level::INFO, "fixture submitted work".into())])
            .sign(signer.private_key());
        let mut builder = BlockBuilder::new(fixture.next_header());
        builder.push_transaction(tx);
        let mut block = builder.build(BTreeSet::new());
        Self::install_network_results(&mut block, vec![Ok(Default::default())]);
        fixture.certify(block);
        fixture
    }

    /// Start at deterministic signed genesis for a separately selected fixture chain label.
    /// The result-only H1 prefix alone does not authenticate genesis execution outputs.
    #[must_use]
    pub fn start(chain_id: &str) -> Self {
        Self::start_with_mode(chain_id, SumeragiConsensusMode::Permissioned)
    }

    /// Start a signed genesis with the explicitly selected consensus policy.
    /// All certificates are genuine native BLS; supplied execution results remain synthetic.
    #[must_use]
    pub fn start_with_mode(chain_id: &str, mode: SumeragiConsensusMode) -> Self {
        assert!(!chain_id.is_empty(), "fixture chain label must be selected");
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
        let authority = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let metadata = ConsensusHandshakeMetadata {
            mode,
            block_cadence_ms: NonZeroU64::new(1000).unwrap(),
            wire_protocol_version: u32::from(crate::sumeragi::PROTOCOL_VERSION),
            consensus_fingerprint: ConsensusFingerprint::new([0x71; 32]),
            kagemusha_mint_finality: KagemushaMintFinalityGenesisParametersV1 {
                authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
                    version: 1,
                    generation: 0,
                    validators: validators
                        .iter()
                        .enumerate()
                        .map(|(index, member)| KagemushaMintFinalityValidatorKeysV1 {
                            validator: PeerId::new(member.public_key.clone()),
                            eq_proof_public_key: hex::decode(PALLAS[index])
                                .unwrap()
                                .try_into()
                                .unwrap(),
                            ep_proof_public_key: hex::decode(VESTA[index])
                                .unwrap()
                                .try_into()
                                .unwrap(),
                        })
                        .collect(),
                },
            },
            sumeragi_v2:
                crate::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
        };
        let mut instructions: Vec<InstructionBox> = validators
            .iter()
            .map(|validator| {
                RegisterPeerWithPop::new(
                    PeerId::new(validator.public_key.clone()),
                    validator.proof_of_possession.clone(),
                )
                .into()
            })
            .collect();
        instructions.push(
            SetParameter::new(Parameter::Custom(CustomParameter::new(
                consensus_metadata::handshake_meta_id(),
                iroha_primitives::json::Json::new(metadata),
            )))
            .into(),
        );
        if mode == SumeragiConsensusMode::Npos {
            instructions.push(
                SetParameter::new(Parameter::Custom(
                    crate::parameter::system::SumeragiNposParameters::default()
                        .into_custom_parameter(),
                ))
                .into(),
            );
        }
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
        let epoch = genesis_epoch(&genesis).unwrap();
        let mut block = genesis.clone();
        Self::install_network_results(&mut block, vec![Ok(Default::default())]);
        let result = Self::result(&block, &epoch);
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            vec![],
            vec![],
            result.preimage().unwrap(),
        )));
        let first = SumeragiFinalityProof {
            block_header: block.header(),
            block_wire: block.encode_wire().unwrap(),
            committee: validators.clone(),
        };
        let mut verifier =
            SumeragiFinalityVerifier::new(&genesis, chain_id, validators.clone()).unwrap();
        verifier.verify(&first).unwrap();
        Self {
            genesis,
            first: first.clone(),
            tip: first,
            keys,
            validators,
            epoch,
            verifier,
            chain_id: chain_id.into(),
        }
    }

    /// Exact genesis-derived network identity selected by this fixture.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        NetworkId::from_genesis_hash(self.genesis.hash())
    }
    /// Independently selected chain label used for native instance derivation.
    #[must_use]
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }
    /// Original signed genesis without a result-only certificate.
    #[must_use]
    pub fn genesis(&self) -> &SignedBlock {
        &self.genesis
    }
    /// Original height-one proof, required when constructing a full prefix.
    #[must_use]
    pub fn genesis_proof(&self) -> &SumeragiFinalityProof {
        &self.first
    }
    /// Last exact proof authenticated by the fixture's retained verifier.
    #[must_use]
    pub fn latest(&self) -> &SumeragiFinalityProof {
        &self.tip
    }
    /// Independent verifier retained at this fixture's authenticated tip.
    #[must_use]
    pub fn verifier(&self) -> SumeragiFinalityVerifier {
        self.verifier.clone()
    }
    /// Complete canonical checkpoint exported from the retained authenticated prefix.
    #[must_use]
    pub fn checkpoint(&self) -> SumeragiFinalityCheckpoint {
        self.verifier.export_checkpoint(&self.tip).unwrap()
    }
    /// Exact next Iroha header; caller-supplied payloads must preserve these parent coordinates.
    #[must_use]
    pub fn next_header(&self) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(self.tip.height() + 1).unwrap(),
            Some(self.tip.block_header.hash()),
            None,
            self.tip.block_header.creation_time_ms + 1,
            0,
        )
    }

    /// Install explicit synthetic network outputs for codec/verifier tests.
    /// This does not execute the transaction instructions or transfer any assets.
    pub fn install_network_results(block: &mut SignedBlock, results: Vec<TransactionResultInner>) {
        let outputs = results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                    input_index: index.try_into().unwrap(),
                    result: result.into(),
                    completions: vec![],
                })
            })
            .collect();
        block
            .set_execution_outputs(
                outputs,
                0,
                Default::default(),
                vec![],
                Default::default(),
                Default::default(),
                &ExecutionOutputLimits {
                    max_outputs: 1024,
                    max_output_bytes: 16 * 1024 * 1024,
                    max_total_output_bytes: 32 * 1024 * 1024,
                    max_executed_wire_bytes: MAX_FINALITY_BLOCK_BYTES as u64,
                },
            )
            .unwrap();
    }

    /// Certify one caller-supplied synthetic result block with the real native three-of-four QC.
    /// No World execution, monetary-policy verification or committee-transition claim is made.
    ///
    /// # Panics
    /// Panics if the block does not extend the fixture's exact tip or violates native proof rules.
    pub fn certify(&mut self, block: SignedBlock) -> SumeragiFinalityProof {
        let result = Self::result(&block, &self.epoch);
        self.certify_result(block, result)
    }

    /// Certify explicit synthetic ordinary writes with their mandatory native lane-state proof.
    /// The complete witness is checked and its exact scratch capacity admitted before signing.
    /// This helper executes no World transition and grants no monetary authority.
    ///
    /// # Panics
    /// Panics on a malformed witness, a foreign lane commitment, or a non-successor block.
    pub fn certify_with_witness(
        &mut self,
        block: SignedBlock,
        witness: &crate::block::consensus::ExecWitness,
    ) -> SumeragiFinalityProof {
        let scratch = NativeLaneStateProof::scratch_bytes(witness.writes.len()).unwrap();
        let proof = NativeLaneStateProof::from_witness(
            witness,
            &mv::allocation::AllocationBudget::new(scratch),
        )
        .unwrap();
        let root = proof.computed_root().unwrap();
        assert!(proof.verify(self.network_id(), block.header().height().get(), root));
        let result = Self::result_with_lane_proof(&block, &self.epoch, proof, root);
        self.certify_result(block, result)
    }

    fn certify_result(
        &mut self,
        mut block: SignedBlock,
        result: ExecutionResultCommitment,
    ) -> SumeragiFinalityProof {
        assert_eq!(block.header().height().get(), self.tip.height() + 1);
        assert_eq!(
            block.header().prev_block_hash(),
            Some(self.tip.block_header.hash())
        );
        assert!(block.commit_certificate().is_none());
        let parent = self.tip.decode_checked().unwrap();
        let (crypto, _) = ProofCrypto::new(&self.validators).unwrap();
        let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
        let header = CoreHeader {
            instance: self.verifier.instance(),
            epoch: core_epoch(&self.epoch).unwrap().id,
            height: block.header().height().get(),
            origin_view: block.header().view_change_index(),
            parent_hash: parent.core_hash,
            parent_result: parent.result,
            payload_hash: payload_hash(&crypto, &payload),
            payload_len: payload.len().try_into().unwrap(),
            proposer: 0,
            skipped_leaders: vec![],
            control_witness: iroha_sumeragi::types::ControlWitness::empty(),
            attest: false,
        };
        let mut qc = Qc {
            kind: VoteKind::Commit,
            instance: header.instance,
            epoch: header.epoch,
            height: header.height,
            view: header.origin_view,
            block_hash: header.hash(&crypto),
            result: result.result().unwrap(),
            attest: false,
            signers: Bitmap::from_indices(4, [0, 1, 2]).unwrap(),
            agg_sig: AggregateSignature([0; 96]),
            attestations: vec![],
            attestation_witness: None,
        };
        let shares: Vec<_> = self.keys[..3]
            .iter()
            .map(|key| iroha_crypto::Signature::try_new(key.private_key(), &qc.preimage()).unwrap())
            .collect();
        let refs: Vec<_> = shares
            .iter()
            .map(iroha_crypto::Signature::payload)
            .collect();
        qc.agg_sig = AggregateSignature(
            bls_normal_aggregate_signatures(&refs)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            norito::encode_canonical(&header).unwrap(),
            norito::encode_canonical(&qc).unwrap(),
            result.preimage().unwrap(),
        )));
        let proof = SumeragiFinalityProof {
            block_header: block.header(),
            block_wire: block.encode_wire().unwrap(),
            committee: self.validators.clone(),
        };
        self.verifier.verify(&proof).unwrap();
        self.tip = proof.clone();
        proof
    }

    fn result(block: &SignedBlock, epoch: &ValidatorEpochContextV1) -> ExecutionResultCommitment {
        let (native_lanes, ordinary_root) = NativeLaneStateProof::empty_for_testing(
            epoch.network_id,
            block.header().height().get(),
        );
        Self::result_with_lane_proof(block, epoch, native_lanes, ordinary_root)
    }

    fn result_with_lane_proof(
        block: &SignedBlock,
        epoch: &ValidatorEpochContextV1,
        native_lanes: NativeLaneStateProof,
        ordinary_root: Hash,
    ) -> ExecutionResultCommitment {
        let height = block.header().height().get();
        let (len, hash) = block.executed_block_wire_identity().unwrap();
        let slot = |height| {
            ScheduledSlot::Ready(ScheduledConfig {
                height,
                epoch: epoch.clone(),
                params: ChainParamsRecord::from_core(&ChainParams::default()),
            })
        };
        ExecutionResultCommitment::new(
            height,
            ExecutionCommitment {
                parent_state_root: Hash::new(b"fixture parent"),
                post_state_root: ordinary_root,
                ordinary_writes_root: ordinary_root,
                kagemusha_top_up_root: None,
                kagemusha_top_up_count: 0,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_witness_certification_binds_all_writes_and_rejects_foreign_height() {
        use crate::block::consensus::{ExecKv, ExecWitness};
        let mut fixture = NativeFinalityFixture::new();
        let height = fixture.latest().height() + 1;
        let state = SumeragiLaneStateCommitment::from_state(
            fixture.network_id(),
            height,
            &crate::sumeragi_lanes::SumeragiLaneState::default(),
        )
        .unwrap();
        let writes = ExecWitness {
            writes: vec![
                ExecKv {
                    key: SUMERAGI_LANE_STATE_WITNESS_KEY.to_vec(),
                    value: norito::encode_canonical(&state).unwrap(),
                },
                ExecKv {
                    key: b"synthetic-fixture-application".to_vec(),
                    value: vec![7],
                },
            ],
            ..ExecWitness::default()
        };
        let mut block = BlockBuilder::new(fixture.next_header()).build(BTreeSet::new());
        NativeFinalityFixture::install_network_results(&mut block, vec![]);
        let proof = fixture.certify_with_witness(block.clone(), &writes);
        assert_eq!(proof.height(), height);
        fixture.verifier().verify_retained_decision(&proof).unwrap();
        let mut next = BlockBuilder::new(fixture.next_header()).build(BTreeSet::new());
        NativeFinalityFixture::install_network_results(&mut next, vec![]);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fixture.certify_with_witness(next, &writes)
            }))
            .is_err()
        );
        assert_eq!(fixture.latest(), &proof);
    }

    #[test]
    #[ignore = "explicit deterministic canonical checkpoint capture, not a qualification gate"]
    fn capture_native_checkpoint_fixtures() {
        let genesis = NativeFinalityFixture::start("portable-native-fixture");
        let second = NativeFinalityFixture::new();
        assert_eq!(genesis.genesis(), second.genesis());
        assert_eq!(genesis.network_id(), second.network_id());
        let first = genesis.checkpoint();
        let second_checkpoint = second.checkpoint();
        let first_bytes = first.encode_canonical().unwrap();
        let second_bytes = second_checkpoint.encode_canonical().unwrap();
        assert_eq!(
            SumeragiFinalityCheckpoint::decode_canonical(&first_bytes).unwrap(),
            first
        );
        assert_eq!(
            SumeragiFinalityCheckpoint::decode_canonical(&second_bytes).unwrap(),
            second_checkpoint
        );
        verify_checkpoint_page(
            second.network_id(),
            &first,
            &[genesis.latest().clone(), second.latest().clone()],
            2,
            4 * 1024 * 1024,
        )
        .unwrap();
        println!("NATIVE_CHECKPOINT_CHAIN_ID={}", genesis.chain_id());
        println!("NATIVE_CHECKPOINT_NETWORK_ID={}", genesis.network_id());
        println!("NATIVE_CHECKPOINT_H1={}", hex::encode(first_bytes));
        println!("NATIVE_CHECKPOINT_H2={}", hex::encode(second_bytes));
    }
    #[test]
    fn explicitly_selected_npos_fixture_binds_signed_epoch_policy() {
        let fixture = NativeFinalityFixture::start_with_mode(
            "native-npos-fixture",
            SumeragiConsensusMode::Npos,
        );
        let epoch = genesis_epoch(fixture.genesis()).unwrap();
        assert_eq!(epoch.mode, crate::parameter::system::ConsensusMode::Npos);
        assert_eq!(epoch.authorization.first_height, 1);
        assert!(epoch.authorization.last_height >= 3);
        assert_eq!(epoch.network_id, fixture.network_id());
        let checkpoint = fixture.checkpoint();
        SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &fixture.network_id(),
            fixture.chain_id(),
        )
        .unwrap()
        .verify_retained_decision(fixture.genesis_proof())
        .unwrap();
    }

    #[test]
    fn exact_native_fixture_checkpoint_and_successor_verify() {
        let first = NativeFinalityFixture::new();
        let second = NativeFinalityFixture::new();
        assert_eq!(first.network_id(), second.network_id());
        assert_eq!(first.latest(), second.latest());
        assert_eq!(first.latest().height(), 2);
        let checkpoint = first.checkpoint();
        assert_eq!(checkpoint.height(), 2);
        let restored =
            SumeragiFinalityCheckpoint::decode_canonical(&checkpoint.encode_canonical().unwrap())
                .unwrap();
        verify_checkpoint_page(
            first.network_id(),
            &restored,
            std::slice::from_ref(first.latest()),
            4,
            4 * 1024 * 1024,
        )
        .unwrap();
        let mut changed = first.latest().clone();
        changed.committee[0].proof_of_possession[0] ^= 1;
        assert!(
            verify_checkpoint_page(
                first.network_id(),
                &restored,
                &[changed],
                4,
                4 * 1024 * 1024
            )
            .is_err()
        );
        let foreign_chain = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &first.network_id(),
            "other chain",
        );
        assert!(foreign_chain.is_err());
    }
}
