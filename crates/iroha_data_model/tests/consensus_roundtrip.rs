//! Ensure the Norito consensus message types support encode/decode roundtrips.
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    block::{
        Header as BlockHeader,
        consensus::{
            ConsensusGenesisModeParams, ConsensusGenesisParams, Evidence, EvidencePenaltyStatus,
            EvidenceRecord, ExecKv, ExecWitness, ExecWitnessMsg, LaneBlockCommitment,
            LaneSettlementReceipt, NposGenesisParams, SumeragiV2EquivocationEvidence,
        },
        consensus_v2::{
            BeaconHorizonStatusV1, BlockSubject, ConsensusMode, ConsensusRound,
            DataAvailabilityLayout, DualQuorum, ExecutionCommitment, GlobalPhase, HeightContext,
            HeightContextId, PROTOCOL_VERSION as V2_PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificateRef, SumeragiV2BodyState, SumeragiV2Equivocation,
            SumeragiV2GenesisContextParameters, SumeragiV2HeightContextStatus,
            SumeragiV2QcResponse, SumeragiV2Status, SumeragiV2StatusPhase, TimeoutVote,
            ValidationError, ValidatorPower,
        },
    },
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, KAGEMUSHA_CHAIN_VERSION_V1,
        KagemushaMintFinalityAuthorityGenerationTemplateV1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1, KagemushaMintFinalityGenesisParametersV1,
        KagemushaMintFinalityValidatorKeysV1,
    },
};
use iroha_model_base::peer::PeerId;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use norito::{
    DeserializePayload,
    codec::{Decode, DecodeAll, Encode},
};
use std::{
    convert::TryFrom,
    fmt::Debug,
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
};
use tempfile::tempdir;
fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; Hash::LENGTH];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let idx_u8 = u8::try_from(idx).expect("hash length fits in u8");
        *byte = seed.wrapping_add(idx_u8);
    }
    Hash::prehashed(bytes)
}
fn sample_block_hash(seed: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(sample_hash(seed))
}

fn mint_finality_authority(
    network_id: NetworkId,
    generation: u64,
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityAuthorityGenerationV1 {
    KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        generation,
        validators: roster
            .iter()
            .enumerate()
            .map(|(index, validator)| KagemushaMintFinalityValidatorKeysV1 {
                validator: validator.validator.clone(),
                eq_proof_public_key: [u8::try_from(index + 1).expect("small fixture roster"); 32],
                ep_proof_public_key: [u8::try_from(index + 17).expect("small fixture roster"); 32],
            })
            .collect(),
    }
}

fn mint_finality_genesis_authorization(
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    last_height: u64,
) -> KagemushaMintFinalityEpochAuthorizationV1 {
    let authorization = KagemushaMintFinalityEpochAuthorizationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: authority.network_id,
        epoch: 0,
        first_height: 1,
        last_height,
        authority_generation: authority.generation,
        authority_id: authority.authority_id().expect("valid fixture authority"),
        beacon: BeaconEpochBindingV1::Bootstrap,
        previous_authorization_id: [0; 32],
        transition_id: [0; 32],
        decision: KagemushaMintFinalityEpochDecisionV1::Genesis,
    };
    authorization
        .validate_against_authority(authority)
        .expect("valid fixture genesis authorization");
    authorization
}


fn sample_bytes(seed: u8, len: usize) -> Vec<u8> {
    assert!(u8::try_from(len).is_ok(), "len must fit in u8");
    (0..len)
        .map(|idx| {
            let idx_u8 = u8::try_from(idx).expect("iterator bound checked");
            seed.wrapping_add(idx_u8)
        })
        .collect()
}
fn checked_bls_peer_id_from_seed(seed: u8) -> PeerId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("derive checked BLS consensus fixture keypair");
    PeerId::new(key_pair.public_key().clone())
}
fn assert_roundtrip<T>(value: &T)
where
    T: Encode + Decode + PartialEq + Debug,
{
    let bytes = Encode::encode(value);
    let mut cursor = bytes.as_slice();
    let decoded = <T as Decode>::decode(&mut cursor).expect("decode succeeds");
    assert!(cursor.is_empty(), "decoder must consume all bytes");
    assert_eq!(decoded, *value, "roundtrip must preserve value");
}
#[derive(Clone)]
struct DeterministicRng(u64);
impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        const A: u64 = 6_364_136_223_846_793_005;
        const C: u64 = 1_442_695_040_888_963_407;
        self.0 = self.0.wrapping_mul(A).wrapping_add(C);
        self.0
    }
    fn next_u32(&mut self) -> u32 {
        let masked = self.next_u64() & u64::from(u32::MAX);
        u32::try_from(masked).expect("masked value fits into u32")
    }
    fn next_u8(&mut self) -> u8 {
        let masked = self.next_u64() & u64::from(u8::MAX);
        u8::try_from(masked).expect("masked value fits into u8")
    }
    fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }
    fn up_to(&mut self, upper_inclusive: usize) -> usize {
        if upper_inclusive == 0 {
            0
        } else {
            let upper =
                u64::try_from(upper_inclusive).expect("upper bound must fit into u64 for testing");
            let sample = match upper.checked_add(1) {
                Some(modulus) => self.next_u64() % modulus,
                None => self.next_u64(),
            };
            usize::try_from(sample).expect("sample must fit into usize for testing")
        }
    }
    fn array32(&mut self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for byte in &mut bytes {
            *byte = self.next_u8();
        }
        bytes
    }
    fn bytes(&mut self, max_len: usize) -> Vec<u8> {
        let len = self.up_to(max_len);
        (0..len).map(|_| self.next_u8()).collect()
    }
}
fn rng_hash(rng: &mut DeterministicRng) -> Hash {
    Hash::prehashed(rng.array32())
}
fn rng_block_hash(rng: &mut DeterministicRng) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(rng_hash(rng))
}
fn rng_consensus_genesis_params(rng: &mut DeterministicRng) -> ConsensusGenesisParams {
    let mode = if rng.next_bool() {
        ConsensusGenesisModeParams::Npos(rng_npos_genesis_params(rng))
    } else {
        ConsensusGenesisModeParams::Permissioned
    };
    ConsensusGenesisParams {
        block_cadence_ms: NonZeroU64::new(rng.next_u64()).unwrap_or(NonZeroU64::MIN),
        block_max_transactions: NonZeroU64::new(rng.next_u64()).unwrap_or(NonZeroU64::MIN),
        mode,
        protocol_version: rng.next_u32(),
        v2_context: recommended_genesis_context(),
    }
}
fn rng_npos_genesis_params(rng: &mut DeterministicRng) -> NposGenesisParams {
    let mut epoch_seed = [0u8; 32];
    for chunk in epoch_seed.chunks_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    NposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(rng.next_u64()).unwrap_or(NonZeroU64::MIN),
        epoch_seed,
        max_validators: rng.next_u32(),
        min_self_bond: rng.next_u64().into(),
        min_nomination_bond: rng.next_u64().into(),
        finality_margin_blocks: rng.next_u64(),
        evidence_horizon_blocks: rng.next_u64(),
        activation_lag_blocks: rng.next_u64(),
        slashing_delay_blocks: rng.next_u64(),
    }
}
fn rng_exec_kv(rng: &mut DeterministicRng) -> ExecKv {
    ExecKv {
        key: rng.bytes(16),
        value: rng.bytes(24),
    }
}
fn rng_exec_witness(rng: &mut DeterministicRng) -> ExecWitness {
    let read_len = rng.up_to(3);
    let write_len = rng.up_to(3);
    let mut reads = Vec::with_capacity(read_len);
    for _ in 0..read_len {
        reads.push(rng_exec_kv(rng));
    }
    let mut writes = Vec::with_capacity(write_len);
    for _ in 0..write_len {
        writes.push(rng_exec_kv(rng));
    }
    ExecWitness {
        reads,
        writes,
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    }
}
fn rng_exec_witness_msg(rng: &mut DeterministicRng) -> ExecWitnessMsg {
    ExecWitnessMsg {
        block_hash: rng_block_hash(rng),
        height: rng.next_u64(),
        view: rng.next_u64(),
        epoch: rng.next_u64(),
        witness: rng_exec_witness(rng),
    }
}
#[test]
fn consensus_genesis_norito_roundtrip() {
    let npos = NposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(120).unwrap(),
        epoch_seed: [0x11; 32],
        max_validators: 19,
        min_self_bond: 10_u64.into(),
        min_nomination_bond: 2_u64.into(),
        finality_margin_blocks: 9,
        evidence_horizon_blocks: 1_024,
        activation_lag_blocks: 12,
        slashing_delay_blocks: 17,
    };
    let with_npos = ConsensusGenesisParams {
        block_cadence_ms: NonZeroU64::new(750).unwrap(),
        block_max_transactions: NonZeroU64::new(512).unwrap(),
        mode: ConsensusGenesisModeParams::Npos(npos.clone()),
        protocol_version: u32::from(V2_PROTOCOL_VERSION),
        v2_context: recommended_genesis_context(),
    };
    let without_npos = ConsensusGenesisParams {
        mode: ConsensusGenesisModeParams::Permissioned,
        ..with_npos.clone()
    };
    assert_roundtrip(&npos);
    assert_roundtrip(&with_npos);
    assert_roundtrip(&without_npos);
}
#[test]
fn kagemusha_mint_finality_genesis_parameters_norito_roundtrip() {
    let network_id = NetworkId::from_genesis_hash(sample_block_hash(0xD0));
    let mut roster = [0xD1, 0xD2, 0xD3, 0xD4]
        .into_iter()
        .map(|seed| ValidatorPower {
            validator: checked_bls_peer_id_from_seed(seed),
            power: 1,
        })
        .collect::<Vec<_>>();
    roster.sort();
    let authority = mint_finality_authority(network_id, 0, &roster);
    let parameters = KagemushaMintFinalityGenesisParametersV1 {
        authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
            version: authority.version,
            generation: authority.generation,
            validators: authority.validators,
        },
    };
    parameters.validate().expect("valid genesis authority");
    assert_roundtrip(&parameters);
    let mut non_genesis = parameters.clone();
    non_genesis.authority_generation.generation = 1;
    assert!(
        non_genesis.validate().is_err(),
        "genesis cannot install a relabeled successor generation"
    );
    assert_eq!(
        parameters
            .authority_generation
            .bind_network_id(network_id)
            .expect("bind final network identity")
            .network_id,
        network_id
    );
}
#[test]
fn consensus_persistence_norito_roundtrip() {
    let evidence = rng_evidence(&mut DeterministicRng::new(0xE1D3_0002));
    let evidence_record = EvidenceRecord {
        evidence: evidence.clone(),
        recorded_at_height: 44,
        recorded_at_view: 8,
        recorded_at_ms: 1_702_000_123,
        penalty_status: EvidencePenaltyStatus::Cancelled { height: 45 },
    };
    let exec_witness = ExecWitness {
        reads: vec![ExecKv {
            key: sample_bytes(0x20, 4),
            value: sample_bytes(0x21, 6),
        }],
        writes: vec![ExecKv {
            key: sample_bytes(0x22, 5),
            value: sample_bytes(0x23, 7),
        }],
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    };
    let exec_witness_msg = ExecWitnessMsg {
        block_hash: sample_block_hash(0x0F),
        height: 44,
        view: 7,
        epoch: 2,
        witness: exec_witness.clone(),
    };
    assert_roundtrip(&evidence);
    assert_roundtrip(&evidence_record);
    assert_roundtrip(&exec_witness);
    assert_roundtrip(&exec_witness_msg);
}
#[test]
fn consensus_roundtrip_deterministic_fuzz() {
    let mut rng = DeterministicRng::new(0xD4E5_F607_89AB_CDEF);
    assert_roundtrip(&SumeragiV2QcResponse::default());
    for _ in 0..64 {
        let status = rng_sumeragi_v2_status(&mut rng);
        assert_roundtrip(&status);
        let qc_response = rng_sumeragi_v2_qc_response(&mut rng);
        assert_roundtrip(&qc_response);
        let genesis = rng_consensus_genesis_params(&mut rng);
        if let ConsensusGenesisModeParams::Npos(npos) = &genesis.mode {
            assert_roundtrip(npos);
        }
        let genesis_bytes = genesis.encode();
        let mut genesis_cursor = genesis_bytes.as_slice();
        let decoded_genesis =
            ConsensusGenesisParams::decode(&mut genesis_cursor).expect("decode genesis");
        assert!(
            genesis_cursor.is_empty(),
            "genesis decode must consume all bytes"
        );
        if decoded_genesis != genesis {
            eprintln!(
                "consensus genesis mismatch\n  original: {genesis:?}\n  decoded:  {decoded_genesis:?}\n  bytes: {genesis_bytes:02x?}"
            );
            panic!("consensus genesis roundtrip mismatch");
        }
        let exec_kv = rng_exec_kv(&mut rng);
        assert_roundtrip(&exec_kv);
        let exec_witness = rng_exec_witness(&mut rng);
        assert_roundtrip(&exec_witness);
        let exec_witness_msg = rng_exec_witness_msg(&mut rng);
        assert_roundtrip(&exec_witness_msg);
        let evidence = rng_evidence(&mut rng);
        assert_roundtrip(&evidence);
        let evidence_record = rng_evidence_record(&mut rng, evidence);
        assert_roundtrip(&evidence_record);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaneCommitmentFixtureMode {
    Verify,
    Regenerate,
}
