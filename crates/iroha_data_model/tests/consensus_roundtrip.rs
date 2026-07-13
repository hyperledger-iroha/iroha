//! Ensure the Norito consensus message types support encode/decode roundtrips.
use std::{
    convert::TryFrom,
    fmt::Debug,
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, MerkleTree, SignatureOf};
use iroha_data_model::{
    block::{
        BlockSignature, Header as BlockHeader,
        consensus::{
            CertPhase, ConsensusBlockHeader, ConsensusGenesisModeParams, ConsensusGenesisParams,
            Evidence, EvidenceKind, EvidencePayload, EvidenceRecord, ExecKv, ExecWitness,
            ExecWitnessMsg, LaneBlockCommitment, LaneSettlementReceipt, NposGenesisParams,
            PERMISSIONED_TAG, Proposal, Qc, QcAggregate, QcRef, QcVote, RbcChunk, RbcDeliver,
            RbcInit, RbcReady, RbcReadySignature, Reconfig, SumeragiQcEntry, SumeragiQcSnapshot,
            VrfCommit, VrfReveal,
        },
        consensus_v2::{
            ConsensusMode, DualQuorum, HeightContextId,
            PROTOCOL_VERSION as V2_PROTOCOL_VERSION, SumeragiV2BodyState,
            SumeragiV2HeightContextStatus, SumeragiV2Status, SumeragiV2StatusPhase,
        },
    },
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
};
use norito::{
    NoritoDeserialize,
    codec::{Decode, Encode},
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

fn sample_bytes(seed: u8, len: usize) -> Vec<u8> {
    assert!(u8::try_from(len).is_ok(), "len must fit in u8");
    (0..len)
        .map(|idx| {
            let idx_u8 = u8::try_from(idx).expect("iterator bound checked");
            seed.wrapping_add(idx_u8)
        })
        .collect()
}

fn checked_random_keypair() -> KeyPair {
    KeyPair::try_random().expect("test fixture random key generation should succeed")
}

fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
        panic!("{algorithm:?} consensus fixture key generation should succeed: {err}")
    })
}

fn checked_bls_keypair() -> KeyPair {
    checked_random_keypair_with_algorithm(Algorithm::BlsNormal)
}

fn checked_random_peer_id() -> PeerId {
    PeerId::from(checked_random_keypair().public_key().clone())
}

fn checked_bls_peer_id() -> PeerId {
    PeerId::new(checked_bls_keypair().public_key().clone())
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

fn rng_ascii_string(rng: &mut DeterministicRng, max_len: usize) -> String {
    let max_len = max_len.max(1);
    let len = rng.range_inclusive(1, max_len);
    (0..len)
        .map(|_| (b'a' + (rng.next_u8() % 26)) as char)
        .collect()
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

fn rng_reconfig(rng: &mut DeterministicRng) -> Reconfig {
    let roster_len = rng.range_inclusive(1, 4);
    let mut roster = Vec::with_capacity(roster_len);
    for _ in 0..roster_len {
        roster.push(checked_random_peer_id());
    }
    Reconfig {
        new_roster: roster,
        activation_height: rng.next_u64(),
    }
}

fn rng_roster(rng: &mut DeterministicRng) -> Vec<PeerId> {
    let roster_len = rng.range_inclusive(1, 4);
    let mut roster = Vec::with_capacity(roster_len);
    for _ in 0..roster_len {
        roster.push(checked_random_peer_id());
    }
    roster
}

fn rng_rbc_init(rng: &mut DeterministicRng) -> RbcInit {
    let roster = rng_roster(rng);
    let roster_hash = Hash::new(roster.encode());
    let height = rng.next_u64().max(1);
    let view = rng.next_u64();
    let total_chunks = u32::try_from(rng.range_inclusive(1, 8)).expect("range bound fits u32");
    let chunk_size_bytes = u32::try_from(rng.range_inclusive(1, 128)).expect("range bound fits");
    let payload_size_bytes = u64::from(chunk_size_bytes)
        .saturating_mul(u64::from(total_chunks.saturating_sub(1)))
        .saturating_add(
            u64::try_from(rng.range_inclusive(
                1,
                usize::try_from(chunk_size_bytes).expect("u32 fits usize"),
            ))
            .expect("range bound fits"),
        );
    let mut chunk_digests = Vec::with_capacity(total_chunks as usize);
    for _ in 0..total_chunks {
        chunk_digests.push(rng.array32());
    }
    let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone())
        .root()
        .map(Hash::from)
        .expect("chunk root");
    let block_header = BlockHeader::new(
        NonZeroU64::new(height).expect("block height must be non-zero"),
        None,
        None,
        None,
        0,
        view,
    );
    let leader_key = checked_random_keypair();
    let (_, leader_private) = leader_key.into_parts();
    let leader_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(&leader_private, block_header.hash())
            .expect("fixture block header signature must sign"),
    );
    RbcInit {
        block_hash: block_header.hash(),
        height,
        view,
        epoch: rng.next_u64(),
        roster,
        roster_hash,
        total_chunks,
        encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
        chunk_size_bytes,
        payload_size_bytes,
        data_shards: 0,
        parity_shards: 0,
        chunk_digests,
        payload_hash: rng_hash(rng),
        chunk_root,
        block_header,
        leader_signature,
    }
}

fn rng_rbc_chunk_from(rng: &mut DeterministicRng, init: &RbcInit) -> RbcChunk {
    let total = init.total_chunks.max(1);
    let upper = total.saturating_sub(1) as usize;
    let idx = u32::try_from(rng.range_inclusive(0, upper)).expect("chunk index must fit into u32");
    RbcChunk {
        block_hash: init.block_hash,
        height: init.height,
        view: init.view,
        epoch: init.epoch,
        idx,
        bytes: rng.bytes(64),
    }
}

fn rng_rbc_ready_from(rng: &mut DeterministicRng, init: &RbcInit) -> RbcReady {
    RbcReady {
        block_hash: init.block_hash,
        height: init.height,
        view: init.view,
        epoch: init.epoch,
        roster_hash: init.roster_hash,
        chunk_root: init.chunk_root,
        sender: rng.next_u32(),
        signature: rng.bytes(64),
    }
}

fn rng_rbc_ready_signature_from(rng: &mut DeterministicRng) -> RbcReadySignature {
    RbcReadySignature {
        sender: rng.next_u32(),
        signature: rng.bytes(64),
    }
}

fn rng_rbc_deliver_from(rng: &mut DeterministicRng, init: &RbcInit) -> RbcDeliver {
    RbcDeliver {
        block_hash: init.block_hash,
        height: init.height,
        view: init.view,
        epoch: init.epoch,
        roster_hash: init.roster_hash,
        chunk_root: init.chunk_root,
        sender: rng.next_u32(),
        signature: rng.bytes(64),
        ready_signatures: (0..rng.up_to(2))
            .map(|_| rng_rbc_ready_signature_from(rng))
            .collect(),
    }
}

fn rng_evidence(rng: &mut DeterministicRng) -> Evidence {
    match rng.up_to(2) {
        0 => {
            let mut v1 = rng_commit_vote(rng);
            if matches!(v1.phase, CertPhase::NewView) {
                v1.phase = CertPhase::Prepare;
                v1.highest_qc = None;
            }
            let mut v2 = v1.clone();
            v2.block_hash = rng_block_hash(rng);
            let kind = match v1.phase {
                CertPhase::Commit => EvidenceKind::DoubleCommit,
                CertPhase::Prepare | CertPhase::NewView => EvidenceKind::DoublePrepare,
            };
            Evidence {
                kind,
                payload: EvidencePayload::DoubleVote { v1, v2 },
            }
        }
        1 => Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: rng_commit_qc(rng),
                reason: rng_ascii_string(rng, 32),
            },
        },
        2 => Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal: rng_proposal(rng),
                reason: rng_ascii_string(rng, 32),
            },
        },
        _ => unreachable!("rng.up_to(2) must be within 0..=2"),
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
    }
}

fn rng_sumeragi_qc_entry(rng: &mut DeterministicRng) -> SumeragiQcEntry {
    SumeragiQcEntry {
        height: rng.next_u64(),
        view: rng.next_u64(),
        subject_block_hash: if rng.next_bool() {
            Some(rng_block_hash(rng))
        } else {
            None
        },
    }
}

fn rng_sumeragi_qc_snapshot(rng: &mut DeterministicRng) -> SumeragiQcSnapshot {
    SumeragiQcSnapshot {
        highest_qc: rng_sumeragi_qc_entry(rng),
        locked_qc: rng_sumeragi_qc_entry(rng),
    }
}

#[test]
fn consensus_genesis_norito_roundtrip() {
    let npos = NposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(120).unwrap(),
        epoch_seed: [0x11; 32],
        vrf_commit_window_blocks: 8,
        vrf_reveal_window_blocks: 5,
        max_validators: 20,
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
    let double_prepare = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote {
            v1: prepare_vote.clone(),
            v2: other_prepare_vote.clone(),
        },
    };
    let invalid_cert = Evidence {
        kind: EvidenceKind::InvalidQc,
        payload: EvidencePayload::InvalidQc {
            certificate: commit_cert.clone(),
            reason: "aggregate mismatch".to_owned(),
        },
    };
    let invalid_proposal = Evidence {
        kind: EvidenceKind::InvalidProposal,
        payload: EvidencePayload::InvalidProposal {
            proposal,
            reason: "payload commitment mismatch".to_owned(),
        },
    };
    let evidence_record = EvidenceRecord {
        evidence: invalid_cert.clone(),
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
    let peer_ids = vec![checked_random_peer_id(), checked_random_peer_id()];
    let reconfig = Reconfig {
        new_roster: peer_ids,
        activation_height: 100,
    };
    let roster = vec![checked_random_peer_id(), checked_random_peer_id()];
    let roster_hash = Hash::new(roster.encode());
    let chunk_digests = vec![[0x31; 32], [0x32; 32], [0x33; 32]];
    let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone())
        .root()
        .map(Hash::from)
        .expect("chunk root");
    let block_header = BlockHeader::new(
        NonZeroU64::new(44).expect("block height must be non-zero"),
        None,
        None,
        None,
        0,
        7,
    );
    let leader_key = checked_random_keypair();
    let (_, leader_private) = leader_key.into_parts();
    let leader_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(&leader_private, block_header.hash())
            .expect("fixture block header signature must sign"),
    );
    let rbc_init = RbcInit {
        block_hash: block_header.hash(),
        height: 44,
        view: 7,
        epoch: 2,
        roster,
        roster_hash,
        total_chunks: 3,
        encoding: iroha_data_model::block::consensus::RbcEncoding::Plain,
        chunk_size_bytes: 128,
        payload_size_bytes: 257,
        data_shards: 0,
        parity_shards: 0,
        chunk_digests,
        payload_hash: sample_hash(0x31),
        chunk_root,
        block_header,
        leader_signature,
    };
    let rbc_chunk = RbcChunk {
        block_hash: rbc_init.block_hash,
        height: rbc_init.height,
        view: rbc_init.view,
        epoch: rbc_init.epoch,
        idx: 1,
        bytes: sample_bytes(0x40, 32),
    };
    let rbc_ready = RbcReady {
        block_hash: rbc_init.block_hash,
        height: rbc_init.height,
        view: rbc_init.view,
        epoch: rbc_init.epoch,
        roster_hash,
        chunk_root: rbc_init.chunk_root,
        sender: 15,
        signature: sample_bytes(0x50, 64),
    };
    let rbc_deliver = RbcDeliver {
        block_hash: rbc_init.block_hash,
        height: rbc_init.height,
        view: rbc_init.view,
        epoch: rbc_init.epoch,
        roster_hash,
        chunk_root: rbc_init.chunk_root,
        sender: 16,
        signature: sample_bytes(0x60, 64),
        ready_signatures: vec![RbcReadySignature {
            sender: 3,
            signature: sample_bytes(0x61, 64),
        }],
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
    assert_roundtrip(&double_prepare);
    assert_roundtrip(&invalid_cert);
    assert_roundtrip(&invalid_proposal);
    assert_roundtrip(&evidence_record);
    assert_roundtrip(&exec_witness);
    assert_roundtrip(&exec_witness_msg);
    assert_roundtrip(&vrf_commit);
    assert_roundtrip(&vrf_reveal);
    assert_roundtrip(&reconfig);
    assert_roundtrip(&rbc_init);
    assert_roundtrip(&rbc_chunk);
    assert_roundtrip(&rbc_ready);
    assert_roundtrip(&rbc_deliver);
}

#[test]
fn consensus_roundtrip_deterministic_fuzz() {
    let mut rng = DeterministicRng::new(0xD4E5_F607_89AB_CDEF);
    for _ in 0..64 {
        let status = rng_sumeragi_v2_status(&mut rng);
        assert_roundtrip(&status);

        let qc_snapshot = rng_sumeragi_qc_snapshot(&mut rng);
        assert_roundtrip(&qc_snapshot);

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

        let reconfig = rng_reconfig(&mut rng);
        assert_roundtrip(&reconfig);

        let evidence = rng_evidence(&mut rng);
        assert_roundtrip(&evidence);

        let evidence_record = rng_evidence_record(&mut rng, evidence);
        assert_roundtrip(&evidence_record);

        let rbc_init = rng_rbc_init(&mut rng);
        assert_roundtrip(&rbc_init);

        let rbc_chunk = rng_rbc_chunk_from(&mut rng, &rbc_init);
        assert_roundtrip(&rbc_chunk);

        let rbc_ready = rng_rbc_ready_from(&mut rng, &rbc_init);
        assert_roundtrip(&rbc_ready);

        let rbc_deliver = rng_rbc_deliver_from(&mut rng, &rbc_init);
        assert_roundtrip(&rbc_deliver);
    }
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
