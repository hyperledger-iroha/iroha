//! Ensure the Norito consensus message types support encode/decode roundtrips.
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    block::{
        Header as BlockHeader,
        consensus::{
            CertPhase, ConsensusBlockHeader, ConsensusGenesisModeParams, ConsensusGenesisParams,
            Evidence, EvidenceRecord, ExecKv, ExecWitness, ExecWitnessMsg, LaneBlockCommitment,
            LaneSettlementReceipt, NposGenesisParams, PERMISSIONED_TAG, Proposal, Qc, QcAggregate,
            QcRef, QcVote, SumeragiV2EquivocationEvidence, VrfCommit, VrfReveal,
        },
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, HeightContextId,
            PROTOCOL_VERSION as V2_PROTOCOL_VERSION, PayloadEncoding, QuorumCertificateRef,
            SumeragiV2BodyState, SumeragiV2Equivocation, SumeragiV2GenesisContextParameters,
            SumeragiV2HeightContextStatus, SumeragiV2QcResponse, SumeragiV2Status,
            SumeragiV2StatusPhase, TimeoutVote, ValidationError, ValidatorPower,
        },
    },
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
};
use norito::{
    NoritoDeserialize,
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

#[test]
fn genesis_context_parameters_reject_noncanonical_hash_markers() {
    let mut context = SumeragiV2GenesisContextParameters::recommended();
    context.nexus_amx_context_hash[Hash::LENGTH - 1] &= !1;
    assert_eq!(
        context.validate(),
        Err(ValidationError::InvalidNexusAmxContextHash),
    );

    let mut context = SumeragiV2GenesisContextParameters::recommended();
    context.execution_policy_hash[Hash::LENGTH - 1] &= !1;
    assert_eq!(
        context.validate(),
        Err(ValidationError::InvalidExecutionPolicyHash),
    );
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
fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
        panic!("{algorithm:?} consensus fixture key generation should succeed: {err}")
    })
}
fn checked_bls_keypair() -> KeyPair {
    checked_random_keypair_with_algorithm(Algorithm::BlsNormal)
}
fn checked_bls_peer_id() -> PeerId {
    PeerId::new(checked_bls_keypair().public_key().clone())
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
    fn range_inclusive(&mut self, min: usize, max: usize) -> usize {
        debug_assert!(min <= max);
        let span = max - min;
        min + self.up_to(span)
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
fn rng_cert_phase_any(rng: &mut DeterministicRng) -> CertPhase {
    match rng.up_to(2) {
        0 => CertPhase::Prepare,
        1 => CertPhase::Commit,
        _ => CertPhase::NewView,
    }
}
fn rng_commit_qc_ref(rng: &mut DeterministicRng) -> QcRef {
    QcRef {
        height: rng.next_u64(),
        view: rng.next_u64(),
        epoch: rng.next_u64(),
        subject_block_hash: rng_block_hash(rng),
        phase: rng_cert_phase_any(rng),
    }
}
fn rng_consensus_block_header(rng: &mut DeterministicRng) -> ConsensusBlockHeader {
    ConsensusBlockHeader {
        parent_hash: rng_block_hash(rng),
        tx_root: rng_hash(rng),
        state_root: rng_hash(rng),
        proposer: rng.next_u32(),
        height: rng.next_u64(),
        view: rng.next_u64(),
        epoch: rng.next_u64(),
        highest_qc: rng_commit_qc_ref(rng),
    }
}
fn rng_commit_aggregate(rng: &mut DeterministicRng) -> QcAggregate {
    let signers_bitmap = rng.bytes(8);
    let bls_len = rng.range_inclusive(0, 96);
    let bls_aggregate_signature = (0..bls_len).map(|_| rng.next_u8()).collect();
    QcAggregate {
        signers_bitmap,
        bls_aggregate_signature,
    }
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
        v2_context:
            iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
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
        vrf_commit_window_blocks: rng.next_u64(),
        vrf_reveal_window_blocks: rng.next_u64(),
        max_validators: rng.next_u32(),
        min_self_bond: rng.next_u64().into(),
        min_nomination_bond: rng.next_u64().into(),
        max_nominator_concentration_pct: u8::try_from(rng.up_to(100))
            .expect("percentage bound fits into u8"),
        seat_band_pct: u8::try_from(rng.up_to(100)).expect("percentage bound fits into u8"),
        max_entity_correlation_pct: u8::try_from(rng.up_to(100))
            .expect("percentage bound fits into u8"),
        finality_margin_blocks: rng.next_u64(),
        evidence_horizon_blocks: rng.next_u64(),
        activation_lag_blocks: rng.next_u64(),
        slashing_delay_blocks: rng.next_u64(),
    }
}
fn rng_proposal(rng: &mut DeterministicRng) -> Proposal {
    Proposal {
        header: rng_consensus_block_header(rng),
        payload_hash: rng_hash(rng),
    }
}
fn rng_commit_vote(rng: &mut DeterministicRng) -> QcVote {
    let phase = rng_cert_phase_any(rng);
    let highest_qc = matches!(phase, CertPhase::NewView).then(|| rng_commit_qc_ref(rng));
    let (block_hash, height, epoch) = highest_qc.as_ref().map_or_else(
        || (rng_block_hash(rng), rng.next_u64(), rng.next_u64()),
        |cert| (cert.subject_block_hash, cert.height, cert.epoch),
    );
    let (parent_state_root, post_state_root) = if matches!(phase, CertPhase::Commit) {
        (rng_hash(rng), rng_hash(rng))
    } else {
        (
            Hash::prehashed([0u8; Hash::LENGTH]),
            Hash::prehashed([0u8; Hash::LENGTH]),
        )
    };
    QcVote {
        phase,
        block_hash,
        parent_state_root,
        post_state_root,
        height,
        view: rng.next_u64(),
        epoch,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc,
        signer: rng.next_u32(),
        bls_sig: rng.bytes(64),
    }
}
fn rng_commit_qc(rng: &mut DeterministicRng) -> Qc {
    let phase = rng_cert_phase_any(rng);
    let highest_qc = matches!(phase, CertPhase::NewView).then(|| rng_commit_qc_ref(rng));
    let (subject_block_hash, height, epoch) = highest_qc.as_ref().map_or_else(
        || (rng_block_hash(rng), rng.next_u64(), rng.next_u64()),
        |cert| (cert.subject_block_hash, cert.height, cert.epoch),
    );
    let (parent_state_root, post_state_root) = if matches!(phase, CertPhase::Commit) {
        (rng_hash(rng), rng_hash(rng))
    } else {
        (
            Hash::prehashed([0u8; Hash::LENGTH]),
            Hash::prehashed([0u8; Hash::LENGTH]),
        )
    };
    let roster_len = rng.range_inclusive(1, 4);
    let mut validator_set = Vec::with_capacity(roster_len);
    for _ in 0..roster_len {
        validator_set.push(checked_bls_peer_id());
    }
    Qc {
        phase,
        subject_block_hash,
        parent_state_root,
        post_state_root,
        height,
        view: rng.next_u64(),
        epoch,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set,
        aggregate: rng_commit_aggregate(rng),
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
fn rng_vrf_commit(rng: &mut DeterministicRng) -> VrfCommit {
    VrfCommit {
        epoch: rng.next_u64(),
        commitment: rng.array32(),
        signer: rng.next_u32(),
        bls_sig: rng.bytes(96),
    }
}
fn rng_vrf_reveal(rng: &mut DeterministicRng) -> VrfReveal {
    VrfReveal {
        epoch: rng.next_u64(),
        reveal: rng.array32(),
        signer: rng.next_u32(),
        bls_sig: rng.bytes(96),
    }
}
fn rng_evidence(rng: &mut DeterministicRng) -> Evidence {
    let mut roster = [0xA1, 0xA2, 0xA3, 0xA4]
        .into_iter()
        .map(|seed| ValidatorPower {
            validator: checked_bls_peer_id_from_seed(seed),
            power: 1,
        })
        .collect::<Vec<_>>();
    roster.sort();
    let height = rng.next_u64().max(1);
    let context = HeightContext {
        network_id: NetworkId::from_genesis_hash(rng_block_hash(rng)),
        protocol_version: V2_PROTOCOL_VERSION,
        height,
        epoch: rng.next_u64(),
        epoch_end_height: height,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("strict four-validator fixture quorum"),
        roster,
        nexus_amx_context_hash: rng_hash(rng),
        execution_policy_hash: rng_hash(rng),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 4,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1024,
            max_chunk_count: 512,
        },
        leader_seed: <[u8; Hash::LENGTH]>::from(rng_hash(rng)),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        view: rng.next_u64(),
    };
    let proofs_of_possession = (0..context.roster.len()).map(|_| rng.bytes(96)).collect();
    Evidence {
        equivocation: SumeragiV2EquivocationEvidence {
            context,
            proofs_of_possession,
            conflict: SumeragiV2Equivocation::TimeoutVote {
                first: TimeoutVote {
                    round,
                    highest_prepare_qc: None,
                    signer: 0,
                    signature: rng.bytes(96),
                },
                second: TimeoutVote {
                    round,
                    highest_prepare_qc: None,
                    signer: 0,
                    signature: rng.bytes(96),
                },
            },
        },
    }
}
fn rng_evidence_record(rng: &mut DeterministicRng, evidence: Evidence) -> EvidenceRecord {
    EvidenceRecord {
        evidence,
        recorded_at_height: rng.next_u64(),
        recorded_at_view: rng.next_u64(),
        recorded_at_ms: rng.next_u64(),
        penalty_applied: false,
        penalty_cancelled: false,
        penalty_cancelled_at_height: None,
        penalty_applied_at_height: None,
        consensus_admitted_at_height: None,
    }
}
fn rng_sumeragi_v2_status(rng: &mut DeterministicRng) -> SumeragiV2Status {
    SumeragiV2Status {
        protocol_version: V2_PROTOCOL_VERSION,
        node_fingerprint: rng_hash(rng),
        build_fingerprint: rng_hash(rng),
        config_fingerprint: rng_hash(rng),
        restart_required: rng.next_bool(),
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(rng_hash(rng))),
        height: rng.next_u64(),
        view: rng.next_u64(),
        phase: SumeragiV2StatusPhase::Prepare,
        leader: rng.next_u32(),
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: rng.next_bool().then(|| rng.next_u64()),
        last_committed_height: rng.next_u64(),
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: rng.next_u64(),
            epoch_end_height: rng.next_u64(),
            mode: ConsensusMode::Permissioned,
            epoch_seed: rng_hash(rng).into(),
            validator_count: 4,
            quorum: DualQuorum {
                min_signers: 3,
                total_power: 4,
            },
        },
        last_commit_qc: None,
        liveness: Default::default(),
    }
}
fn rng_sumeragi_v2_qc_response(rng: &mut DeterministicRng) -> SumeragiV2QcResponse {
    fn prepare_qc(rng: &mut DeterministicRng) -> QuorumCertificateRef {
        let round = ConsensusRound {
            context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(rng_hash(
                rng,
            ))),
            height: rng.next_u64(),
            view: rng.next_u64(),
        };
        QuorumCertificateRef {
            round,
            proposal_round: round,
            phase: GlobalPhase::Prepare,
            subject: BlockSubject {
                parent_block_hash: rng.next_bool().then(|| rng_block_hash(rng)),
                block_hash: rng_block_hash(rng),
                payload_hash: rng_hash(rng),
            },
            execution_commitment: ExecutionCommitment::without_topups_or_merge_carrier(
                rng_hash(rng),
                rng_hash(rng),
                rng_hash(rng),
                rng.next_u64().max(1),
                rng_hash(rng),
            ),
        }
    }
    SumeragiV2QcResponse {
        highest_prepare_qc: Some(prepare_qc(rng)),
        locked_prepare_qc: Some(prepare_qc(rng)),
    }
}
#[test]
fn consensus_genesis_norito_roundtrip() {
    let npos = NposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(120).unwrap(),
        epoch_seed: [0x11; 32],
        vrf_commit_window_blocks: 8,
        vrf_reveal_window_blocks: 5,
        max_validators: 19,
        min_self_bond: 10_u64.into(),
        min_nomination_bond: 2_u64.into(),
        max_nominator_concentration_pct: 35,
        seat_band_pct: 15,
        max_entity_correlation_pct: 25,
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
        v2_context:
            iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
    };
    let without_npos = ConsensusGenesisParams {
        mode: ConsensusGenesisModeParams::Permissioned,
        ..with_npos.clone()
    };
    assert_roundtrip(&npos);
    assert_roundtrip(&with_npos);
    assert_roundtrip(&without_npos);
}
#[allow(clippy::too_many_lines)]
#[test]
fn consensus_messages_norito_roundtrip() {
    let validator_set = vec![checked_bls_peer_id(), checked_bls_peer_id()];
    let cert_header = QcRef {
        height: 42,
        view: 4,
        epoch: 2,
        subject_block_hash: sample_block_hash(0x10),
        phase: CertPhase::Commit,
    };
    let block_header = ConsensusBlockHeader {
        parent_hash: sample_block_hash(0x01),
        tx_root: sample_hash(0x02),
        state_root: sample_hash(0x03),
        proposer: 7,
        height: 43,
        view: 5,
        epoch: 2,
        highest_qc: cert_header,
    };
    let proposal = Proposal {
        header: block_header,
        payload_hash: sample_hash(0x04),
    };
    let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
    let prepare_vote = QcVote {
        phase: CertPhase::Prepare,
        block_hash: sample_block_hash(0x05),
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height: 43,
        view: 5,
        epoch: 2,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: None,
        signer: 11,
        bls_sig: sample_bytes(0xA0, 32),
    };
    let other_prepare_vote = QcVote {
        block_hash: sample_block_hash(0x06),
        ..prepare_vote.clone()
    };
    let commit_vote = QcVote {
        phase: CertPhase::Commit,
        block_hash: sample_block_hash(0x07),
        parent_state_root: sample_hash(0x0B),
        post_state_root: sample_hash(0x0C),
        ..prepare_vote.clone()
    };
    let aggregate = QcAggregate {
        signers_bitmap: sample_bytes(0xE0, 8),
        bls_aggregate_signature: sample_bytes(0xF0, 96),
    };
    let commit_cert = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: sample_block_hash(0x09),
        parent_state_root: sample_hash(0x0A),
        post_state_root: sample_hash(0x0F),
        height: 43,
        view: 6,
        epoch: 2,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set: validator_set.clone(),
        aggregate: aggregate.clone(),
    };
    let new_view_vote = QcVote {
        phase: CertPhase::NewView,
        block_hash: cert_header.subject_block_hash,
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height: cert_header.height,
        view: 7,
        epoch: cert_header.epoch,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: Some(cert_header),
        signer: 12,
        bls_sig: sample_bytes(0xC0, 32),
    };
    let new_view_cert = Qc {
        phase: CertPhase::NewView,
        subject_block_hash: cert_header.subject_block_hash,
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height: cert_header.height,
        view: 7,
        epoch: cert_header.epoch,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: Some(cert_header),
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set: validator_set.clone(),
        aggregate: aggregate.clone(),
    };
    let evidence = rng_evidence(&mut DeterministicRng::new(0xE1D3_0002));
    let evidence_record = EvidenceRecord {
        evidence: evidence.clone(),
        recorded_at_height: 44,
        recorded_at_view: 8,
        recorded_at_ms: 1_702_000_123,
        penalty_applied: false,
        penalty_cancelled: true,
        penalty_cancelled_at_height: Some(45),
        penalty_applied_at_height: None,
        consensus_admitted_at_height: Some(44),
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
    let vrf_commit = VrfCommit {
        epoch: 3,
        commitment: [0x33; 32],
        signer: 5,
        bls_sig: vec![0x35; 96],
    };
    let vrf_reveal = VrfReveal {
        epoch: 3,
        reveal: [0x44; 32],
        signer: 5,
        bls_sig: vec![0x45; 96],
    };
    assert_roundtrip(&cert_header);
    assert_roundtrip(&block_header);
    assert_roundtrip(&proposal);
    assert_roundtrip(&prepare_vote);
    assert_roundtrip(&other_prepare_vote);
    assert_roundtrip(&commit_vote);
    assert_roundtrip(&new_view_vote);
    assert_roundtrip(&aggregate);
    assert_roundtrip(&commit_cert);
    assert_roundtrip(&new_view_cert);
    assert_roundtrip(&evidence);
    assert_roundtrip(&evidence_record);
    assert_roundtrip(&exec_witness);
    assert_roundtrip(&exec_witness_msg);
    assert_roundtrip(&vrf_commit);
    assert_roundtrip(&vrf_reveal);
}
#[test]
#[allow(clippy::too_many_lines)]
fn current_qc_json_requires_highest_qc_and_rejects_unknown_fields() {
    macro_rules! assert_required_highest_qc {
        ($ty:ty, $value:expr) => {{
            let expected: $ty = $value;
            let json = norito::json::to_value(&expected).expect("serialize current QC JSON");
            assert!(
                json.get("highest_qc")
                    .is_some_and(norito::json::Value::is_null),
                "{} must serialize an absent highest QC as an explicit null",
                stringify!($ty)
            );
            assert_eq!(
                norito::json::from_value::<$ty>(json.clone())
                    .expect("decode explicit highest-QC slot"),
                expected
            );

            let mut missing = json.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("current QC JSON object")
                    .remove("highest_qc")
                    .is_some()
            );
            assert!(
                norito::json::from_value::<$ty>(missing).is_err(),
                "{} must reject an omitted highest-QC slot",
                stringify!($ty)
            );

            let mut unknown = json;
            unknown
                .as_object_mut()
                .expect("current QC JSON object")
                .insert(
                    "pre_release_field".to_owned(),
                    norito::json::Value::Bool(true),
                );
            assert!(
                norito::json::from_value::<$ty>(unknown).is_err(),
                "{} must reject unknown JSON fields",
                stringify!($ty)
            );
        }};
    }

    let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
    let vote = QcVote {
        phase: CertPhase::Prepare,
        block_hash: sample_block_hash(0x71),
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height: 70,
        view: 8,
        epoch: 3,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: None,
        signer: 2,
        bls_sig: sample_bytes(0x72, 48),
    };
    assert_required_highest_qc!(QcVote, vote);

    let aggregate = QcAggregate {
        signers_bitmap: vec![0b0000_0111],
        bls_aggregate_signature: sample_bytes(0x73, 96),
    };
    let validator_set = Vec::<PeerId>::new();
    let qc = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: sample_block_hash(0x74),
        parent_state_root: sample_hash(0x75),
        post_state_root: sample_hash(0x76),
        height: 70,
        view: 8,
        epoch: 3,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_owned(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set,
        aggregate: aggregate.clone(),
    };
    assert_required_highest_qc!(Qc, qc);

    let mut aggregate_json =
        norito::json::to_value(&aggregate).expect("serialize current QC aggregate JSON");
    assert_eq!(
        norito::json::from_value::<QcAggregate>(aggregate_json.clone())
            .expect("decode current QC aggregate JSON"),
        aggregate
    );
    aggregate_json
        .as_object_mut()
        .expect("current QC aggregate JSON object")
        .insert(
            "pre_release_field".to_owned(),
            norito::json::Value::Bool(true),
        );
    assert!(
        norito::json::from_value::<QcAggregate>(aggregate_json).is_err(),
        "QcAggregate must reject unknown JSON fields"
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn qc_and_evidence_record_reject_shortened_pre_release_binary_layouts() {
    #[derive(Encode)]
    struct PreReleaseQcVote {
        phase: CertPhase,
        block_hash: HashOf<BlockHeader>,
        parent_state_root: Hash,
        post_state_root: Hash,
        height: u64,
        view: u64,
        epoch: u64,
        chain_order_hash: Hash,
        rechain_seq: u64,
        signer: u32,
        bls_sig: Vec<u8>,
    }
    #[derive(Encode)]
    struct PreReleaseQc {
        phase: CertPhase,
        subject_block_hash: HashOf<BlockHeader>,
        parent_state_root: Hash,
        post_state_root: Hash,
        height: u64,
        view: u64,
        epoch: u64,
        chain_order_hash: Hash,
        rechain_seq: u64,
        mode_tag: String,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
        validator_set: Vec<PeerId>,
        aggregate: QcAggregate,
    }
    #[derive(Encode)]
    struct PreReleaseEvidenceRecord {
        evidence: Evidence,
        recorded_at_height: u64,
        recorded_at_view: u64,
        recorded_at_ms: u64,
    }
    #[derive(Encode)]
    struct PreReleaseEvidenceRecordWithoutNullableSlots {
        evidence: Evidence,
        recorded_at_height: u64,
        recorded_at_view: u64,
        recorded_at_ms: u64,
        penalty_applied: bool,
        penalty_cancelled: bool,
    }

    let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
    let vote = QcVote {
        phase: CertPhase::Prepare,
        block_hash: sample_block_hash(0x81),
        parent_state_root: zero_root,
        post_state_root: zero_root,
        height: 80,
        view: 9,
        epoch: 4,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: None,
        signer: 7,
        bls_sig: sample_bytes(0x82, 48),
    };
    assert_roundtrip(&vote);
    let shortened_vote = PreReleaseQcVote {
        phase: vote.phase,
        block_hash: vote.block_hash,
        parent_state_root: vote.parent_state_root,
        post_state_root: vote.post_state_root,
        height: vote.height,
        view: vote.view,
        epoch: vote.epoch,
        chain_order_hash: vote.chain_order_hash,
        rechain_seq: vote.rechain_seq,
        signer: vote.signer,
        bls_sig: vote.bls_sig.clone(),
    }
    .encode();
    assert!(
        QcVote::decode_all(&mut shortened_vote.as_slice()).is_err(),
        "QcVote must reject the pre-release layout without highest_qc"
    );

    let validator_set = Vec::<PeerId>::new();
    let aggregate = QcAggregate {
        signers_bitmap: vec![0b0000_0111],
        bls_aggregate_signature: sample_bytes(0x83, 96),
    };
    let qc = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: sample_block_hash(0x84),
        parent_state_root: sample_hash(0x85),
        post_state_root: sample_hash(0x86),
        height: 80,
        view: 9,
        epoch: 4,
        chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_owned(),
        highest_qc: None,
        validator_set_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH])),
        validator_set_hash_version: 1,
        validator_set,
        aggregate,
    };
    assert_roundtrip(&qc);
    let shortened_qc = PreReleaseQc {
        phase: qc.phase,
        subject_block_hash: qc.subject_block_hash,
        parent_state_root: qc.parent_state_root,
        post_state_root: qc.post_state_root,
        height: qc.height,
        view: qc.view,
        epoch: qc.epoch,
        chain_order_hash: qc.chain_order_hash,
        rechain_seq: qc.rechain_seq,
        mode_tag: qc.mode_tag.clone(),
        validator_set_hash: qc.validator_set_hash,
        validator_set_hash_version: qc.validator_set_hash_version,
        validator_set: qc.validator_set.clone(),
        aggregate: qc.aggregate.clone(),
    }
    .encode();
    assert!(
        Qc::decode_all(&mut shortened_qc.as_slice()).is_err(),
        "Qc must reject the pre-release layout without highest_qc"
    );

    let evidence = rng_evidence(&mut DeterministicRng::new(0xE1D3_0084));
    let record = EvidenceRecord {
        evidence,
        recorded_at_height: 84,
        recorded_at_view: 9,
        recorded_at_ms: 1_702_000_456,
        penalty_applied: true,
        penalty_cancelled: false,
        penalty_cancelled_at_height: None,
        penalty_applied_at_height: Some(85),
        consensus_admitted_at_height: Some(84),
    };
    assert_roundtrip(&record);
    let shortened_record = PreReleaseEvidenceRecord {
        evidence: record.evidence.clone(),
        recorded_at_height: record.recorded_at_height,
        recorded_at_view: record.recorded_at_view,
        recorded_at_ms: record.recorded_at_ms,
    }
    .encode();
    assert!(
        EvidenceRecord::decode_all(&mut shortened_record.as_slice()).is_err(),
        "EvidenceRecord must reject the pre-release layout without penalty and admission state"
    );

    let pending_record = EvidenceRecord {
        evidence: record.evidence.clone(),
        recorded_at_height: 86,
        recorded_at_view: 10,
        recorded_at_ms: 1_702_000_789,
        penalty_applied: false,
        penalty_cancelled: false,
        penalty_cancelled_at_height: None,
        penalty_applied_at_height: None,
        consensus_admitted_at_height: None,
    };
    assert_roundtrip(&pending_record);
    let omitted_nullable_slots = PreReleaseEvidenceRecordWithoutNullableSlots {
        evidence: pending_record.evidence.clone(),
        recorded_at_height: pending_record.recorded_at_height,
        recorded_at_view: pending_record.recorded_at_view,
        recorded_at_ms: pending_record.recorded_at_ms,
        penalty_applied: pending_record.penalty_applied,
        penalty_cancelled: pending_record.penalty_cancelled,
    }
    .encode();
    assert!(
        EvidenceRecord::decode_all(&mut omitted_nullable_slots.as_slice()).is_err(),
        "EvidenceRecord must encode explicit None tags for every nullable storage slot"
    );
}
#[test]
fn sumeragi_v2_equivocation_evidence_json_is_closed_and_exact() {
    let evidence = rng_evidence(&mut DeterministicRng::new(0xE1D3_0090)).equivocation;
    let json = norito::json::to_value(&evidence).expect("serialize current v2 evidence JSON");
    assert_eq!(
        norito::json::from_value::<SumeragiV2EquivocationEvidence>(json.clone())
            .expect("decode current v2 evidence JSON"),
        evidence
    );

    let context = json
        .get("context")
        .and_then(norito::json::Value::as_object)
        .expect("v2 evidence context JSON object");
    for field in [
        "next_epoch_snapshot",
        "parent_commit_qc",
        "snapshot_bootstrap",
    ] {
        assert!(
            context.get(field).is_some_and(norito::json::Value::is_null),
            "nullable context field {field} must remain an explicit null"
        );
    }

    for field in ["context", "proofs_of_possession", "conflict"] {
        let mut missing = json.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("v2 evidence JSON object")
                .remove(field)
                .is_some()
        );
        assert!(
            norito::json::from_value::<SumeragiV2EquivocationEvidence>(missing).is_err(),
            "current v2 evidence JSON must require {field}"
        );
    }

    let mut unknown = json;
    unknown
        .as_object_mut()
        .expect("v2 evidence JSON object")
        .insert(
            "pre_release_field".to_owned(),
            norito::json::Value::Bool(true),
        );
    assert!(
        norito::json::from_value::<SumeragiV2EquivocationEvidence>(unknown).is_err(),
        "current v2 evidence JSON must reject unknown fields"
    );
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
        let cert_header = rng_commit_qc_ref(&mut rng);
        assert_roundtrip(&cert_header);
        let block_header = rng_consensus_block_header(&mut rng);
        assert_roundtrip(&block_header);
        let proposal = rng_proposal(&mut rng);
        assert_roundtrip(&proposal);
        let vote = rng_commit_vote(&mut rng);
        assert_roundtrip(&vote);
        let aggregate = rng_commit_aggregate(&mut rng);
        assert_roundtrip(&aggregate);
        let cert = rng_commit_qc(&mut rng);
        assert_roundtrip(&cert);
        let exec_kv = rng_exec_kv(&mut rng);
        assert_roundtrip(&exec_kv);
        let exec_witness = rng_exec_witness(&mut rng);
        assert_roundtrip(&exec_witness);
        let exec_witness_msg = rng_exec_witness_msg(&mut rng);
        assert_roundtrip(&exec_witness_msg);
        let vrf_commit = rng_vrf_commit(&mut rng);
        assert_roundtrip(&vrf_commit);
        let vrf_reveal = rng_vrf_reveal(&mut rng);
        assert_roundtrip(&vrf_reveal);
        let evidence = rng_evidence(&mut rng);
        assert_roundtrip(&evidence);
        let evidence_record = rng_evidence_record(&mut rng, evidence);
        assert_roundtrip(&evidence_record);
    }
}
#[test]
fn sumeragi_v2_qc_response_requires_both_current_fields() {
    let retired = r#"{
        "highest_qc": {"height": 10, "view": 2, "subject_block_hash": null},
        "locked_qc": {"height": 9, "view": 1, "subject_block_hash": null}
    }"#;
    assert!(norito::json::from_str::<SumeragiV2QcResponse>(retired).is_err());
    for missing in [
        "{}",
        r#"{"highest_prepare_qc":null}"#,
        r#"{"locked_prepare_qc":null}"#,
    ] {
        assert!(norito::json::from_str::<SumeragiV2QcResponse>(missing).is_err());
    }
    assert_eq!(
        norito::json::from_str::<SumeragiV2QcResponse>(
            r#"{"highest_prepare_qc":null,"locked_prepare_qc":null}"#,
        )
        .expect("explicit null PrepareQC options are canonical"),
        SumeragiV2QcResponse::default(),
    );
    let canonical = norito::json::to_value(&SumeragiV2QcResponse::default())
        .expect("render required nullable PrepareQC slots");
    assert!(
        canonical
            .get("highest_prepare_qc")
            .is_some_and(|value| value.is_null())
    );
    assert!(
        canonical
            .get("locked_prepare_qc")
            .is_some_and(|value| value.is_null())
    );
}
#[test]
fn lane_commitment_fixtures_roundtrip() {
    let fixtures_dir = workspace_root()
        .join("fixtures")
        .join("nexus")
        .join("lane_commitments");
    assert!(
        fixtures_dir.is_dir(),
        "lane commitment fixtures directory {fixtures_dir:?} must exist"
    );
    let seen = process_lane_commitment_fixtures(&fixtures_dir, LaneCommitmentFixtureMode::Verify);
    assert!(
        seen > 0,
        "expected at least one lane commitment fixture under {fixtures_dir:?}"
    );
}
#[test]
#[ignore = "regenerates lane commitment Norito fixtures"]
fn regenerate_lane_commitment_fixtures() {
    let fixtures_dir = workspace_root()
        .join("fixtures")
        .join("nexus")
        .join("lane_commitments");
    assert!(
        fixtures_dir.is_dir(),
        "lane commitment fixtures directory {fixtures_dir:?} must exist"
    );
    let seen =
        process_lane_commitment_fixtures(&fixtures_dir, LaneCommitmentFixtureMode::Regenerate);
    assert!(
        seen > 0,
        "expected at least one lane commitment fixture under {fixtures_dir:?}"
    );
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaneCommitmentFixtureMode {
    Verify,
    Regenerate,
}
fn process_lane_commitment_fixtures(fixtures_dir: &Path, mode: LaneCommitmentFixtureMode) -> usize {
    let mut seen = 0usize;
    for entry in fs::read_dir(fixtures_dir).expect("read lane commitment fixtures") {
        let entry = entry.expect("fixture entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read lane commitment fixture {}: {err}", path.display()));
        let commitment: LaneBlockCommitment = norito::json::from_str(&raw).unwrap_or_else(|err| {
            panic!("parse lane commitment fixture {}: {err}", path.display())
        });
        let reserialized =
            norito::json::to_json_pretty(&commitment).expect("serialize commitment to JSON");
        let replay: LaneBlockCommitment =
            norito::json::from_str(&reserialized).expect("parse reserialized commitment");
        assert_eq!(
            commitment,
            replay,
            "JSON roundtrip mismatch for fixture {}",
            path.display()
        );
        let norito_bytes =
            norito::to_bytes(&commitment).expect("encode commitment to Norito bytes");
        let archived =
            norito::from_bytes::<LaneBlockCommitment>(&norito_bytes).expect("archive commitment");
        let decoded = NoritoDeserialize::try_deserialize(archived)
            .expect("deserialize commitment from Norito bytes");
        assert_eq!(
            commitment,
            decoded,
            "Norito roundtrip mismatch for fixture {}",
            path.display()
        );
        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("fixture stem");
        let to_path = fixtures_dir.join(format!("{stem}.to"));
        match mode {
            LaneCommitmentFixtureMode::Verify => {
                if to_path.is_file() {
                    let fixture_bytes = fs::read(&to_path).unwrap_or_else(|err| {
                        panic!("read Norito bytes {}: {err}", to_path.display())
                    });
                    let archived_file = norito::from_bytes::<LaneBlockCommitment>(&fixture_bytes)
                        .expect("archive fixture");
                    let decoded_from_file = NoritoDeserialize::try_deserialize(archived_file)
                        .expect("deserialize fixture Norito bytes");
                    assert_eq!(
                        commitment,
                        decoded_from_file,
                        "Norito fixture bytes mismatch for {}",
                        to_path.display()
                    );
                    assert_eq!(
                        norito_bytes,
                        fixture_bytes,
                        "canonical Norito bytes do not match fixture {}",
                        to_path.display()
                    );
                }
            }
            LaneCommitmentFixtureMode::Regenerate => {
                fs::write(&to_path, &norito_bytes).unwrap_or_else(|err| {
                    panic!("write lane commitment fixture {}: {err}", to_path.display())
                });
            }
        }
        seen += 1;
    }
    seen
}
fn sample_lane_commitment_fixture() -> LaneBlockCommitment {
    let receipt = LaneSettlementReceipt {
        source_id: [0xAB; 32],
        local_amount: "4".parse().expect("valid settlement quantity"),
        xor_due: "1.62".parse().expect("valid settlement quantity"),
        xor_after_haircut: "1.6".parse().expect("valid settlement quantity"),
        xor_variance: "0.02".parse().expect("valid settlement quantity"),
        timestamp_ms: 1_726_296_400_000,
    };
    LaneBlockCommitment {
        block_height: 8_642,
        lane_id: LaneId::new(1),
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: DataSpaceId::new(7),
        tx_count: 1,
        total_local_amount: receipt.local_amount.clone(),
        total_xor_due: receipt.xor_due.clone(),
        total_xor_after_haircut: receipt.xor_after_haircut.clone(),
        total_xor_variance: receipt.xor_variance.clone(),
        swap_metadata: Some(iroha_data_model::block::consensus::LaneSwapMetadata {
            epsilon_bps: 25,
            twap_window_seconds: 60,
            liquidity_profile: iroha_data_model::block::consensus::LaneLiquidityProfile::Tier1,
            twap_local_per_xor: "8123.4455".parse().expect("canonical TWAP"),
            volatility_class: iroha_data_model::block::consensus::LaneVolatilityClass::Stable,
        }),
        receipts: vec![receipt],
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    }
}
fn sample_lane_commitment_fixture_without_metadata() -> LaneBlockCommitment {
    LaneBlockCommitment {
        block_height: 8_643,
        lane_id: LaneId::new(2),
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: DataSpaceId::new(9),
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    }
}
fn write_lane_commitment_json_fixture(
    fixtures_dir: &Path,
    stem: &str,
    commitment: &LaneBlockCommitment,
) -> PathBuf {
    let path = fixtures_dir.join(format!("{stem}.json"));
    let json = norito::json::to_json_pretty(commitment).expect("serialize lane commitment fixture");
    fs::write(&path, json)
        .unwrap_or_else(|err| panic!("write lane commitment fixture {}: {err}", path.display()));
    path
}
fn assert_lane_commitment_to_fixture_matches(
    json_path: &Path,
    commitment: &LaneBlockCommitment,
    context: &str,
) {
    let to_path = json_path.with_extension("to");
    let expected_bytes = norito::to_bytes(commitment).expect("encode canonical Norito bytes");
    let actual_bytes = fs::read(&to_path).expect("read Norito fixture companion");
    assert_eq!(
        actual_bytes, expected_bytes,
        "{context}: Norito companion bytes must match canonical encoding"
    );
}
#[test]
fn lane_commitment_fixture_helper_skips_non_json_and_missing_to() {
    let dir = tempdir().expect("create temp dir");
    let fixtures_dir = dir.path();
    let commitment = sample_lane_commitment_fixture();
    let json_path = write_lane_commitment_json_fixture(fixtures_dir, "lane", &commitment);
    let ignored_path = fixtures_dir.join("notes.txt");
    fs::write(&ignored_path, "not a fixture").expect("write ignored file");
    let seen = process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Verify);
    assert_eq!(seen, 1, "only JSON fixtures should be counted");
    let to_path = json_path.with_extension("to");
    assert!(
        !to_path.exists(),
        "verify mode must not create missing Norito fixture companions"
    );
}
#[test]
fn lane_commitment_fixture_helper_returns_zero_for_empty_directory() {
    let dir = tempdir().expect("create temp dir");
    let fixtures_dir = dir.path();
    let verified =
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Verify);
    let regenerated =
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Regenerate);
    assert_eq!(
        verified, 0,
        "empty directories should not report fixtures in verify mode"
    );
    assert_eq!(
        regenerated, 0,
        "empty directories should not report fixtures in regenerate mode"
    );
}
#[test]
fn lane_commitment_fixture_helper_regenerate_overwrites_stale_to() {
    let dir = tempdir().expect("create temp dir");
    let fixtures_dir = dir.path();
    let commitment = sample_lane_commitment_fixture();
    let json_path = write_lane_commitment_json_fixture(fixtures_dir, "lane", &commitment);
    let to_path = json_path.with_extension("to");
    fs::write(&to_path, b"stale").expect("write stale Norito fixture");
    let seen =
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Regenerate);
    assert_eq!(seen, 1, "regenerate mode should process the JSON fixture");
    assert_lane_commitment_to_fixture_matches(
        &json_path,
        &commitment,
        "regenerate mode must overwrite stale Norito fixture bytes",
    );
    let verified =
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Verify);
    assert_eq!(
        verified, 1,
        "regenerated fixture should verify successfully"
    );
}
#[test]
fn lane_commitment_fixture_helper_verify_panics_on_stale_but_decodable_to() {
    let dir = tempdir().expect("create temp dir");
    let fixtures_dir = dir.path();
    let commitment = sample_lane_commitment_fixture();
    let json_path = write_lane_commitment_json_fixture(fixtures_dir, "lane", &commitment);
    let stale_commitment = sample_lane_commitment_fixture_without_metadata();
    let to_path = json_path.with_extension("to");
    let stale_bytes = norito::to_bytes(&stale_commitment).expect("encode stale Norito fixture");
    fs::write(&to_path, stale_bytes).expect("write stale decodable Norito fixture");
    let result = std::panic::catch_unwind(|| {
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Verify)
    });
    assert!(
        result.is_err(),
        "verify mode must fail when a decodable companion fixture is stale"
    );
}
#[test]
fn lane_commitment_fixture_helper_regenerate_creates_missing_to_for_multiple_fixtures() {
    let dir = tempdir().expect("create temp dir");
    let fixtures_dir = dir.path();
    let with_metadata = sample_lane_commitment_fixture();
    let without_metadata = sample_lane_commitment_fixture_without_metadata();
    let with_metadata_path =
        write_lane_commitment_json_fixture(fixtures_dir, "with_metadata", &with_metadata);
    let without_metadata_path =
        write_lane_commitment_json_fixture(fixtures_dir, "without_metadata", &without_metadata);
    let regenerated =
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Regenerate);
    assert_eq!(
        regenerated, 2,
        "regenerate mode should process every JSON lane commitment fixture"
    );
    assert_lane_commitment_to_fixture_matches(
        &with_metadata_path,
        &with_metadata,
        "regenerate mode should create a .to companion for metadata fixtures",
    );
    assert_lane_commitment_to_fixture_matches(
        &without_metadata_path,
        &without_metadata,
        "regenerate mode should create a .to companion for metadata-free fixtures",
    );
    let verified =
        process_lane_commitment_fixtures(fixtures_dir, LaneCommitmentFixtureMode::Verify);
    assert_eq!(
        verified, 2,
        "generated companions should verify for every fixture"
    );
}
#[test]
fn lane_block_commitment_roundtrips_without_metadata_or_receipts() {
    let commitment = sample_lane_commitment_fixture_without_metadata();
    let json = norito::json::to_json_pretty(&commitment).expect("serialize commitment to JSON");
    let replay: LaneBlockCommitment =
        norito::json::from_str(&json).expect("parse reserialized commitment");
    assert_eq!(
        replay, commitment,
        "JSON roundtrip must preserve commitments without optional metadata"
    );
    let norito_bytes = norito::to_bytes(&commitment).expect("encode commitment to Norito bytes");
    let archived = norito::from_bytes::<LaneBlockCommitment>(&norito_bytes)
        .expect("archive metadata-free commitment");
    let decoded =
        NoritoDeserialize::try_deserialize(archived).expect("deserialize metadata-free commitment");
    assert_eq!(
        decoded, commitment,
        "Norito roundtrip must preserve commitments without receipts"
    );
}
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root directory exists")
        .to_path_buf()
}
