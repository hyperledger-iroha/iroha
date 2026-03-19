//! Ensure the Norito consensus message types support encode/decode roundtrips.
use std::{
    convert::TryFrom,
    fmt::Debug,
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf, KeyPair, MerkleTree, SignatureOf};
use iroha_data_model::{
    block::{
        BlockSignature, Header as BlockHeader,
        consensus::{
            CertPhase, ConsensusBlockHeader, ConsensusGenesisParams, Evidence, EvidenceKind,
            EvidencePayload, EvidenceRecord, ExecKv, ExecWitness, ExecWitnessMsg,
            LaneBlockCommitment, LaneSettlementReceipt, NposGenesisParams, PERMISSIONED_TAG,
            Proposal, Qc, QcAggregate, QcRef, QcVote, RbcChunk, RbcDeliver, RbcInit, RbcReady,
            RbcReadySignature, Reconfig, SumeragiBlockSyncRosterStatus,
            SumeragiCommitInflightStatus, SumeragiCommitPipelineStatus, SumeragiCommitQuorumStatus,
            SumeragiConsensusCapsStatus, SumeragiConsensusMessageHandlingEntry,
            SumeragiConsensusMessageHandlingStatus, SumeragiDaGateReason,
            SumeragiDaGateSatisfaction, SumeragiDaGateStatus, SumeragiDataspaceCommitment,
            SumeragiKuraStoreStatus, SumeragiLaneCommitment, SumeragiLaneGovernance,
            SumeragiMembershipMismatchStatus, SumeragiMembershipStatus,
            SumeragiMissingBlockFetchStatus, SumeragiNposTimeoutsStatus,
            SumeragiPeerKeyPolicyStatus, SumeragiPendingRbcEntry, SumeragiPendingRbcStatus,
            SumeragiQcEntry, SumeragiQcSnapshot, SumeragiQcStatus, SumeragiRbcEvictedSession,
            SumeragiRbcMismatchEntry, SumeragiRbcMismatchStatus, SumeragiRbcStoreStatus,
            SumeragiRoundGapStatus, SumeragiRuntimeUpgradeHook, SumeragiStatusWire,
            SumeragiValidationRejectStatus, SumeragiViewChangeCauseStatus,
            SumeragiVoteValidationDropEntry, SumeragiVoteValidationDropPeerEntry,
            SumeragiVoteValidationDropReasonCount, SumeragiVoteValidationDropStatus,
            SumeragiWorkerLoopStatus, SumeragiWorkerQueueDepths, SumeragiWorkerQueueDiagnostics,
            SumeragiWorkerQueueTotals, VrfCommit, VrfReveal,
        },
    },
    da::commitment,
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
};
use norito::{
    NoritoDeserialize,
    codec::{Decode, Encode},
};

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

    fn next_u16(&mut self) -> u16 {
        let masked = self.next_u64() & u64::from(u16::MAX);
        u16::try_from(masked).expect("masked value fits into u16")
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
    let npos = if rng.next_bool() {
        Some(rng_npos_genesis_params(rng))
    } else {
        None
    };
    ConsensusGenesisParams {
        block_time_ms: rng.next_u64(),
        commit_time_ms: rng.next_u64(),
        min_finality_ms: rng.next_u64(),
        max_clock_drift_ms: rng.next_u64(),
        collectors_k: rng.next_u16(),
        redundant_send_r: rng.next_u8(),
        block_max_transactions: rng.next_u64(),
        da_enabled: rng.next_bool(),
        epoch_length_blocks: rng.next_u64(),
        bls_domain: rng_ascii_string(rng, 24),
        npos,
    }
}

fn rng_npos_genesis_params(rng: &mut DeterministicRng) -> NposGenesisParams {
    let mut epoch_seed = [0u8; 32];
    for chunk in epoch_seed.chunks_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    NposGenesisParams {
        block_time_ms: rng.next_u64(),
        timeout_propose_ms: rng.next_u64(),
        timeout_prevote_ms: rng.next_u64(),
        timeout_precommit_ms: rng.next_u64(),
        timeout_commit_ms: rng.next_u64(),
        timeout_da_ms: rng.next_u64(),
        timeout_aggregator_ms: rng.next_u64(),
        k_aggregators: rng.next_u16(),
        redundant_send_r: rng.next_u8(),
        epoch_seed,
        vrf_commit_window_blocks: rng.next_u64(),
        vrf_reveal_window_blocks: rng.next_u64(),
        max_validators: rng.next_u32(),
        min_self_bond: rng.next_u64(),
        min_nomination_bond: rng.next_u64(),
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
        validator_set.push(PeerId::new(
            KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
                .public_key()
                .clone(),
        ));
    }
    Qc {
        phase,
        subject_block_hash,
        parent_state_root,
        post_state_root,
        height,
        view: rng.next_u64(),
        epoch,
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
    }
}

fn rng_vrf_reveal(rng: &mut DeterministicRng) -> VrfReveal {
    VrfReveal {
        epoch: rng.next_u64(),
        reveal: rng.array32(),
        signer: rng.next_u32(),
    }
}

fn rng_reconfig(rng: &mut DeterministicRng) -> Reconfig {
    let roster_len = rng.range_inclusive(1, 4);
    let mut roster = Vec::with_capacity(roster_len);
    for _ in 0..roster_len {
        roster.push(PeerId::from(KeyPair::random().public_key().clone()));
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
        roster.push(PeerId::from(KeyPair::random().public_key().clone()));
    }
    roster
}

fn rng_rbc_init(rng: &mut DeterministicRng) -> RbcInit {
    let roster = rng_roster(rng);
    let roster_hash = Hash::new(roster.encode());
    let height = rng.next_u64().max(1);
    let view = rng.next_u64();
    let total_chunks = u32::try_from(rng.range_inclusive(1, 8)).expect("range bound fits u32");
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
    let leader_key = KeyPair::random();
    let (_, leader_private) = leader_key.into_parts();
    let leader_signature = BlockSignature::new(
        0,
        SignatureOf::from_hash(&leader_private, block_header.hash()),
    );
    RbcInit {
        block_hash: block_header.hash(),
        height,
        view,
        epoch: rng.next_u64(),
        roster,
        roster_hash,
        total_chunks,
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
    }
}

#[allow(clippy::too_many_lines)]
fn rng_sumeragi_status(rng: &mut DeterministicRng) -> SumeragiStatusWire {
    SumeragiStatusWire {
        mode_tag: rng_ascii_string(rng, 24),
        staged_mode_tag: rng.next_bool().then(|| rng_ascii_string(rng, 24)),
        staged_mode_activation_height: rng.next_bool().then(|| rng.next_u64()),
        mode_activation_lag_blocks: rng.next_bool().then(|| rng.next_u64()),
        mode_flip_kill_switch: rng.next_bool(),
        mode_flip_blocked: rng.next_bool(),
        mode_flip_success_total: rng.next_u64(),
        mode_flip_fail_total: rng.next_u64(),
        mode_flip_blocked_total: rng.next_u64(),
        last_mode_flip_timestamp_ms: rng.next_bool().then(|| rng.next_u64()),
        last_mode_flip_error: rng.next_bool().then(|| rng_ascii_string(rng, 8)),
        consensus_caps: rng.next_bool().then(|| rng_consensus_caps_status(rng)),
        effective_min_finality_ms: rng.next_u64(),
        effective_block_time_ms: rng.next_u64(),
        effective_commit_time_ms: rng.next_u64(),
        effective_pacing_factor_bps: rng.next_u64(),
        effective_commit_quorum_timeout_ms: rng.next_u64(),
        effective_availability_timeout_ms: rng.next_u64(),
        effective_pacemaker_interval_ms: rng.next_u64(),
        effective_npos_timeouts: rng.next_bool().then(|| SumeragiNposTimeoutsStatus {
            propose_ms: rng.next_u64(),
            prevote_ms: rng.next_u64(),
            precommit_ms: rng.next_u64(),
            commit_ms: rng.next_u64(),
            da_ms: rng.next_u64(),
            aggregator_ms: rng.next_u64(),
            exec_ms: rng.next_u64(),
            witness_ms: rng.next_u64(),
        }),
        effective_collectors_k: rng.next_u64(),
        effective_redundant_send_r: rng.next_u64(),
        leader_index: rng.next_u64(),
        highest_qc_height: rng.next_u64(),
        highest_qc_view: rng.next_u64(),
        highest_qc_subject: if rng.next_bool() {
            Some(rng_block_hash(rng))
        } else {
            None
        },
        locked_qc_height: rng.next_u64(),
        locked_qc_view: rng.next_u64(),
        locked_qc_subject: if rng.next_bool() {
            Some(rng_block_hash(rng))
        } else {
            None
        },
        commit_qc: rng_commit_qc_status(rng),
        commit_quorum: rng_commit_quorum_status(rng),
        view_change_proof_accepted_total: rng.next_u64(),
        view_change_proof_stale_total: rng.next_u64(),
        view_change_proof_rejected_total: rng.next_u64(),
        view_change_suggest_total: rng.next_u64(),
        view_change_install_total: rng.next_u64(),
        view_change_causes: SumeragiViewChangeCauseStatus {
            commit_failure_total: rng.next_u64(),
            quorum_timeout_total: rng.next_u64(),
            stake_quorum_timeout_total: rng.next_u64(),
            da_gate_total: rng.next_u64(),
            censorship_evidence_total: rng.next_u64(),
            missing_payload_total: rng.next_u64(),
            missing_qc_total: rng.next_u64(),
            validation_reject_total: rng.next_u64(),
            last_cause: if rng.next_bool() {
                Some("quorum_timeout".to_string())
            } else {
                None
            },
            last_cause_timestamp_ms: rng.next_u64(),
            last_commit_failure_timestamp_ms: rng.next_u64(),
            last_quorum_timeout_timestamp_ms: rng.next_u64(),
            last_stake_quorum_timeout_timestamp_ms: rng.next_u64(),
            last_da_gate_timestamp_ms: rng.next_u64(),
            last_censorship_evidence_timestamp_ms: rng.next_u64(),
            last_missing_payload_timestamp_ms: rng.next_u64(),
            last_missing_qc_timestamp_ms: rng.next_u64(),
            last_validation_reject_timestamp_ms: rng.next_u64(),
        },
        gossip_fallback_total: rng.next_u64(),
        block_created_dropped_by_lock_total: rng.next_u64(),
        block_created_hint_mismatch_total: rng.next_u64(),
        block_created_proposal_mismatch_total: rng.next_u64(),
        consensus_message_handling: rng_consensus_message_handling_status(rng),
        vote_validation_drops: rng_vote_validation_drop_status(rng),
        validation_reject_total: rng.next_u64(),
        validation_reject_reason: if rng.next_bool() {
            Some("stateless".to_string())
        } else {
            None
        },
        validation_rejects: SumeragiValidationRejectStatus {
            total: rng.next_u64(),
            stateless_total: rng.next_u64(),
            execution_total: rng.next_u64(),
            prev_hash_total: rng.next_u64(),
            prev_height_total: rng.next_u64(),
            topology_total: rng.next_u64(),
            last_reason: if rng.next_bool() {
                Some("execution".to_string())
            } else {
                None
            },
            last_height: if rng.next_bool() {
                Some(rng.next_u64())
            } else {
                None
            },
            last_view: if rng.next_bool() {
                Some(rng.next_u64())
            } else {
                None
            },
            last_block: if rng.next_bool() {
                Some(rng_block_hash(rng))
            } else {
                None
            },
            last_timestamp_ms: rng.next_u64(),
        },
        peer_key_policy: SumeragiPeerKeyPolicyStatus {
            total: rng.next_u64(),
            missing_hsm_total: rng.next_u64(),
            disallowed_algorithm_total: rng.next_u64(),
            disallowed_provider_total: rng.next_u64(),
            lead_time_violation_total: rng.next_u64(),
            activation_in_past_total: rng.next_u64(),
            expiry_before_activation_total: rng.next_u64(),
            identifier_collision_total: rng.next_u64(),
            last_reason: if rng.next_bool() {
                Some("identifier_collision".to_string())
            } else {
                None
            },
            last_timestamp_ms: rng.next_u64(),
        },
        block_sync_roster: SumeragiBlockSyncRosterStatus {
            commit_qc_hint_total: rng.next_u64(),
            checkpoint_hint_total: rng.next_u64(),
            commit_qc_history_total: rng.next_u64(),
            checkpoint_history_total: rng.next_u64(),
            roster_sidecar_total: rng.next_u64(),
            commit_roster_journal_total: rng.next_u64(),
            drop_missing_total: rng.next_u64(),
            drop_unsolicited_share_blocks_total: rng.next_u64(),
        },
        pacemaker_backpressure_deferrals_total: rng.next_u64(),
        commit_pipeline_tick_total: rng.next_u64(),
        da_reschedule_total: rng.next_u64(),
        missing_block_fetch: rng_missing_block_fetch(rng),
        committed_edge_conflict_obsolete_total: rng.next_u64(),
        roster_sidecar_mismatch_obsolete_total: rng.next_u64(),
        da_gate: rng_da_gate(rng),
        kura_store: rng_kura_store(rng),
        rbc_store: rng_rbc_store(rng),
        rbc_mismatch: rng_rbc_mismatch_status(rng),
        pending_rbc: SumeragiPendingRbcStatus::default(),
        tx_queue_depth: rng.next_u64(),
        tx_queue_capacity: rng.next_u64(),
        tx_queue_saturated: rng.next_bool(),
        epoch_length_blocks: rng.next_u64(),
        epoch_commit_deadline_offset: rng.next_u64(),
        epoch_reveal_deadline_offset: rng.next_u64(),
        prf_epoch_seed: if rng.next_bool() {
            Some(rng.array32())
        } else {
            None
        },
        prf_height: rng.next_u64(),
        prf_view: rng.next_u64(),
        vrf_penalty_epoch: rng.next_u64(),
        vrf_committed_no_reveal_total: rng.next_u64(),
        vrf_no_participation_total: rng.next_u64(),
        vrf_late_reveals_total: rng.next_u64(),
        consensus_penalties_applied_total: rng.next_u64(),
        consensus_penalties_pending: rng.next_u64(),
        vrf_penalties_applied_total: rng.next_u64(),
        vrf_penalties_pending: rng.next_u64(),
        membership: rng_sumeragi_membership_status(rng),
        membership_mismatch: rng_sumeragi_membership_mismatch_status(rng),
        lane_commitments: vec![rng_lane_commitment(rng)],
        dataspace_commitments: vec![rng_dataspace_commitment(rng)],
        lane_settlement_commitments: Vec::new(),
        lane_relay_envelopes: vec![rng_lane_relay_envelope(rng)],
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: vec![rng_lane_governance(rng)],
        worker_loop: rng_worker_loop_status(rng),
        commit_inflight: rng_commit_inflight_status(rng),
        commit_pipeline: rng_commit_pipeline_status(rng),
        round_gap: rng_round_gap_status(rng),
    }
}

fn rng_commit_pipeline_status(rng: &mut DeterministicRng) -> SumeragiCommitPipelineStatus {
    SumeragiCommitPipelineStatus {
        last_total_ms: rng.next_u64(),
        last_validation_ms: rng.next_u64(),
        last_qc_rebuild_ms: rng.next_u64(),
        last_gate_ms: rng.next_u64(),
        last_finalize_ms: rng.next_u64(),
        last_drain_results_ms: rng.next_u64(),
        last_drain_qc_verify_ms: rng.next_u64(),
        last_drain_persist_ms: rng.next_u64(),
        last_drain_kura_store_ms: rng.next_u64(),
        last_drain_state_apply_ms: rng.next_u64(),
        last_drain_state_commit_ms: rng.next_u64(),
        ema_total_ms: rng.next_u64(),
        ema_validation_ms: rng.next_u64(),
        ema_gate_ms: rng.next_u64(),
        ema_finalize_ms: rng.next_u64(),
    }
}

fn rng_round_gap_status(rng: &mut DeterministicRng) -> SumeragiRoundGapStatus {
    SumeragiRoundGapStatus {
        last_deliver_to_state_commit_ms: rng.next_u64(),
        last_state_commit_to_next_propose_ms: rng.next_u64(),
        last_deliver_to_next_propose_ms: rng.next_u64(),
        ema_deliver_to_state_commit_ms: rng.next_u64(),
        ema_state_commit_to_next_propose_ms: rng.next_u64(),
        ema_deliver_to_next_propose_ms: rng.next_u64(),
    }
}

fn rng_sumeragi_membership_status(rng: &mut DeterministicRng) -> SumeragiMembershipStatus {
    SumeragiMembershipStatus {
        height: rng.next_u64(),
        view: rng.next_u64(),
        epoch: rng.next_u64(),
        view_hash: if rng.next_bool() {
            Some(rng.array32())
        } else {
            None
        },
    }
}

fn rng_sumeragi_membership_mismatch_status(
    rng: &mut DeterministicRng,
) -> SumeragiMembershipMismatchStatus {
    let active_len = (rng.next_u32() % 3) as usize;
    let active_peers = (0..active_len)
        .map(|_| PeerId::from(KeyPair::random().public_key().clone()))
        .collect();
    SumeragiMembershipMismatchStatus {
        active_peers,
        last_peer: rng
            .next_bool()
            .then(|| PeerId::from(KeyPair::random().public_key().clone())),
        last_height: rng.next_u64(),
        last_view: rng.next_u64(),
        last_epoch: rng.next_u64(),
        last_local_hash: rng.next_bool().then(|| rng.array32()),
        last_remote_hash: rng.next_bool().then(|| rng.array32()),
        last_timestamp_ms: rng.next_u64(),
    }
}

fn rng_lane_commitment(rng: &mut DeterministicRng) -> SumeragiLaneCommitment {
    SumeragiLaneCommitment {
        block_height: rng.next_u64(),
        lane_id: LaneId::new(rng.next_u32()),
        tx_count: rng.next_u64(),
        total_chunks: rng.next_u64(),
        rbc_bytes_total: rng.next_u64(),
        teu_total: rng.next_u64(),
        block_hash: rng_block_hash(rng),
    }
}

fn rng_dataspace_commitment(rng: &mut DeterministicRng) -> SumeragiDataspaceCommitment {
    SumeragiDataspaceCommitment {
        block_height: rng.next_u64(),
        lane_id: LaneId::new(rng.next_u32()),
        dataspace_id: DataSpaceId::new(rng.next_u64()),
        tx_count: rng.next_u64(),
        total_chunks: rng.next_u64(),
        rbc_bytes_total: rng.next_u64(),
        teu_total: rng.next_u64(),
        block_hash: rng_block_hash(rng),
    }
}

fn rng_lane_relay_envelope(rng: &mut DeterministicRng) -> LaneRelayEnvelope {
    let height = NonZeroU64::new(rng.next_u64().saturating_add(1))
        .unwrap_or_else(|| NonZeroU64::new(1).expect("nonzero height"));
    let mut header = BlockHeader::new(height, None, None, None, rng.next_u64(), rng.next_u64());
    let da_hash = rng
        .next_bool()
        .then(|| HashOf::<commitment::DaCommitmentBundle>::from_untyped_unchecked(rng_hash(rng)));
    header.set_da_commitments_hash(da_hash);

    let receipt = LaneSettlementReceipt {
        source_id: rng.array32(),
        local_amount_micro: u128::from(rng.next_u64()),
        xor_due_micro: u128::from(rng.next_u64()),
        xor_after_haircut_micro: u128::from(rng.next_u64()),
        xor_variance_micro: u128::from(rng.next_u64()),
        timestamp_ms: rng.next_u64(),
    };
    let settlement = LaneBlockCommitment {
        block_height: header.height().get(),
        lane_id: LaneId::new(rng.next_u32()),
        dataspace_id: DataSpaceId::new(rng.next_u64()),
        tx_count: 1,
        total_local_micro: receipt.local_amount_micro,
        total_xor_due_micro: receipt.xor_due_micro,
        total_xor_after_haircut_micro: receipt.xor_after_haircut_micro,
        total_xor_variance_micro: receipt.xor_variance_micro,
        swap_metadata: None,
        receipts: vec![receipt],
    };
    let qc = if rng.next_bool() {
        let validator_set: Vec<PeerId> = Vec::new();
        Some(Qc {
            phase: CertPhase::Commit,
            subject_block_hash: header.hash(),
            parent_state_root: rng_hash(rng),
            post_state_root: rng_hash(rng),
            height: header.height().get(),
            view: rng.next_u64(),
            epoch: rng.next_u64(),
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: 1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![rng.next_u8()],
                bls_aggregate_signature: sample_bytes(rng.next_u8(), 48),
            },
        })
    } else {
        None
    };

    LaneRelayEnvelope::new(header, qc, da_hash, settlement, rng.next_u64())
        .expect("construct lane relay envelope")
}

fn rng_runtime_upgrade_hook(rng: &mut DeterministicRng) -> Option<SumeragiRuntimeUpgradeHook> {
    if !rng.next_bool() {
        return None;
    }
    let allowed_ids = (0..rng.up_to(4))
        .map(|_| format!("upgrade-{}", rng.next_u32()))
        .collect();
    Some(SumeragiRuntimeUpgradeHook {
        allow: rng.next_bool(),
        require_metadata: rng.next_bool(),
        metadata_key: if rng.next_bool() {
            Some(format!("key{}", rng.next_u32()))
        } else {
            None
        },
        allowed_ids,
    })
}

fn rng_lane_governance(rng: &mut DeterministicRng) -> SumeragiLaneGovernance {
    let manifest_required = rng.next_bool();
    let manifest_ready = manifest_required && rng.next_bool();
    SumeragiLaneGovernance {
        lane_id: LaneId::new(rng.next_u32()),
        alias: format!("lane-{}", rng.next_u32()),
        governance: if rng.next_bool() {
            Some(format!("module-{}", rng.next_u32()))
        } else {
            None
        },
        manifest_required,
        manifest_ready,
        manifest_path: if manifest_ready && rng.next_bool() {
            Some(format!("/var/lanes/lane-{}.json", rng.next_u32()))
        } else {
            None
        },
        validator_ids: (0..rng.up_to(4))
            .map(|_| format!("validator{}@test", rng.next_u32()))
            .collect(),
        quorum: if rng.next_bool() {
            Some(rng.next_u32())
        } else {
            None
        },
        protected_namespaces: (0..rng.up_to(3))
            .map(|_| format!("ns{}", rng.next_u32()))
            .collect(),
        runtime_upgrade: rng_runtime_upgrade_hook(rng),
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

fn rng_consensus_caps_status(rng: &mut DeterministicRng) -> SumeragiConsensusCapsStatus {
    SumeragiConsensusCapsStatus {
        collectors_k: rng.next_u16(),
        redundant_send_r: rng.next_u8(),
        da_enabled: rng.next_bool(),
        rbc_chunk_max_bytes: rng.next_u64(),
        rbc_session_ttl_ms: rng.next_u64(),
        rbc_store_max_sessions: rng.next_u32(),
        rbc_store_soft_sessions: rng.next_u32(),
        rbc_store_max_bytes: rng.next_u64(),
        rbc_store_soft_bytes: rng.next_u64(),
    }
}

fn rng_missing_block_fetch(rng: &mut DeterministicRng) -> SumeragiMissingBlockFetchStatus {
    SumeragiMissingBlockFetchStatus {
        total: rng.next_u64(),
        last_targets: rng.next_u64(),
        last_dwell_ms: rng.next_u64(),
    }
}

fn rng_da_gate(rng: &mut DeterministicRng) -> SumeragiDaGateStatus {
    let reason = match rng.range_inclusive(0, 5) {
        0 => SumeragiDaGateReason::None,
        1 => SumeragiDaGateReason::MissingLocalData,
        2 => SumeragiDaGateReason::ManifestMissing,
        3 => SumeragiDaGateReason::ManifestHashMismatch,
        4 => SumeragiDaGateReason::ManifestReadFailed,
        _ => SumeragiDaGateReason::ManifestSpoolScan,
    };
    let last_satisfied = match rng.range_inclusive(0, 1) {
        0 => SumeragiDaGateSatisfaction::None,
        _ => SumeragiDaGateSatisfaction::MissingDataRecovered,
    };
    SumeragiDaGateStatus {
        reason,
        last_satisfied,
        missing_local_data_total: rng.next_u64(),
        manifest_guard_total: rng.next_u64(),
    }
}

fn rng_kura_store(rng: &mut DeterministicRng) -> SumeragiKuraStoreStatus {
    SumeragiKuraStoreStatus {
        failures_total: rng.next_u64(),
        abort_total: rng.next_u64(),
        stage_total: rng.next_u64(),
        rollback_total: rng.next_u64(),
        stage_last_height: rng.next_u64(),
        stage_last_view: rng.next_u64(),
        stage_last_hash: rng
            .next_bool()
            .then(|| HashOf::<BlockHeader>::from_untyped_unchecked(rng_hash(rng))),
        rollback_last_height: rng.next_u64(),
        rollback_last_view: rng.next_u64(),
        rollback_last_hash: rng
            .next_bool()
            .then(|| HashOf::<BlockHeader>::from_untyped_unchecked(rng_hash(rng))),
        rollback_last_reason: rng.next_bool().then(|| String::from("store_failure")),
        lock_reset_total: rng.next_u64(),
        lock_reset_last_height: rng.next_u64(),
        lock_reset_last_view: rng.next_u64(),
        lock_reset_last_hash: rng
            .next_bool()
            .then(|| HashOf::<BlockHeader>::from_untyped_unchecked(rng_hash(rng))),
        lock_reset_last_reason: rng.next_bool().then(|| String::from("kura_abort")),
        last_retry_attempt: rng.next_u64(),
        last_retry_backoff_ms: rng.next_u64(),
        last_height: rng.next_u64(),
        last_view: rng.next_u64(),
        last_hash: rng
            .next_bool()
            .then(|| HashOf::<BlockHeader>::from_untyped_unchecked(rng_hash(rng))),
    }
}

fn rng_rbc_evicted_session(rng: &mut DeterministicRng) -> SumeragiRbcEvictedSession {
    SumeragiRbcEvictedSession {
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(rng_hash(rng)),
        height: rng.next_u64(),
        view: rng.next_u64(),
    }
}

fn rng_rbc_store(rng: &mut DeterministicRng) -> SumeragiRbcStoreStatus {
    let eviction_len = rng.up_to(3);
    let recent_evictions = (0..eviction_len)
        .map(|_| rng_rbc_evicted_session(rng))
        .collect();
    SumeragiRbcStoreStatus {
        sessions: rng.next_u64(),
        bytes: rng.next_u64(),
        pressure_level: rng.next_u8(),
        backpressure_deferrals_total: rng.next_u64(),
        persist_drops_total: rng.next_u64(),
        evictions_total: rng.next_u64(),
        recent_evictions,
    }
}

fn rng_rbc_mismatch_entry(rng: &mut DeterministicRng) -> SumeragiRbcMismatchEntry {
    SumeragiRbcMismatchEntry {
        peer_id: PeerId::from(KeyPair::random().public_key().clone()),
        chunk_digest_mismatch_total: rng.next_u64(),
        payload_hash_mismatch_total: rng.next_u64(),
        chunk_root_mismatch_total: rng.next_u64(),
        last_timestamp_ms: rng.next_u64(),
    }
}

fn rng_rbc_mismatch_status(rng: &mut DeterministicRng) -> SumeragiRbcMismatchStatus {
    let entry_len = rng.up_to(3);
    let entries = (0..entry_len)
        .map(|_| rng_rbc_mismatch_entry(rng))
        .collect();
    SumeragiRbcMismatchStatus { entries }
}

fn rng_consensus_message_handling_status(
    rng: &mut DeterministicRng,
) -> SumeragiConsensusMessageHandlingStatus {
    let entry_len = rng.up_to(3);
    let entries = (0..entry_len)
        .map(|_| SumeragiConsensusMessageHandlingEntry {
            kind: rng_ascii_string(rng, 16),
            outcome: rng_ascii_string(rng, 12),
            reason: rng_ascii_string(rng, 24),
            total: rng.next_u64(),
        })
        .collect();
    SumeragiConsensusMessageHandlingStatus { entries }
}

fn rng_vote_validation_drop_entry(rng: &mut DeterministicRng) -> SumeragiVoteValidationDropEntry {
    SumeragiVoteValidationDropEntry {
        reason: rng_ascii_string(rng, 24),
        height: rng.next_u64(),
        view: rng.next_u64(),
        epoch: rng.next_u64(),
        signer_index: rng.next_u32(),
        peer_id: rng
            .next_bool()
            .then(|| PeerId::from(KeyPair::random().public_key().clone())),
        roster_hash: rng
            .next_bool()
            .then(|| HashOf::<Vec<PeerId>>::from_untyped_unchecked(rng_hash(rng))),
        roster_len: rng.next_u32(),
        block_hash: rng_block_hash(rng),
        timestamp_ms: rng.next_u64(),
    }
}

fn rng_vote_validation_drop_reason_count(
    rng: &mut DeterministicRng,
) -> SumeragiVoteValidationDropReasonCount {
    SumeragiVoteValidationDropReasonCount {
        reason: rng_ascii_string(rng, 12),
        total: rng.next_u64(),
    }
}

fn rng_vote_validation_drop_peer_entry(
    rng: &mut DeterministicRng,
) -> SumeragiVoteValidationDropPeerEntry {
    let reason_len = rng.up_to(3);
    let reasons = (0..reason_len)
        .map(|_| rng_vote_validation_drop_reason_count(rng))
        .collect();
    SumeragiVoteValidationDropPeerEntry {
        peer_id: PeerId::from(KeyPair::random().public_key().clone()),
        roster_hash: rng
            .next_bool()
            .then(|| HashOf::<Vec<PeerId>>::from_untyped_unchecked(rng_hash(rng))),
        roster_len: rng.next_u32(),
        total: rng.next_u64(),
        reasons,
        last_height: rng.next_u64(),
        last_view: rng.next_u64(),
        last_epoch: rng.next_u64(),
        last_timestamp_ms: rng.next_u64(),
    }
}

fn rng_vote_validation_drop_status(rng: &mut DeterministicRng) -> SumeragiVoteValidationDropStatus {
    let entry_len = rng.up_to(3);
    let entries = (0..entry_len)
        .map(|_| rng_vote_validation_drop_entry(rng))
        .collect();
    let peer_entry_len = rng.up_to(3);
    let peer_entries = (0..peer_entry_len)
        .map(|_| rng_vote_validation_drop_peer_entry(rng))
        .collect();
    SumeragiVoteValidationDropStatus {
        total: rng.next_u64(),
        entries,
        peer_entries,
    }
}

fn rng_worker_queue_depths(rng: &mut DeterministicRng) -> SumeragiWorkerQueueDepths {
    SumeragiWorkerQueueDepths {
        vote_rx: rng.next_u64(),
        block_payload_rx: rng.next_u64(),
        rbc_chunk_rx: rng.next_u64(),
        block_rx: rng.next_u64(),
        consensus_rx: rng.next_u64(),
        lane_relay_rx: rng.next_u64(),
        background_rx: rng.next_u64(),
    }
}

fn rng_worker_queue_totals(rng: &mut DeterministicRng) -> SumeragiWorkerQueueTotals {
    SumeragiWorkerQueueTotals {
        vote_rx: rng.next_u64(),
        block_payload_rx: rng.next_u64(),
        rbc_chunk_rx: rng.next_u64(),
        block_rx: rng.next_u64(),
        consensus_rx: rng.next_u64(),
        lane_relay_rx: rng.next_u64(),
        background_rx: rng.next_u64(),
    }
}

fn rng_worker_queue_diagnostics(rng: &mut DeterministicRng) -> SumeragiWorkerQueueDiagnostics {
    SumeragiWorkerQueueDiagnostics {
        blocked_total: rng_worker_queue_totals(rng),
        blocked_ms_total: rng_worker_queue_totals(rng),
        blocked_max_ms: rng_worker_queue_totals(rng),
        dropped_total: rng_worker_queue_totals(rng),
    }
}

fn rng_worker_loop_status(rng: &mut DeterministicRng) -> SumeragiWorkerLoopStatus {
    let stage = match rng.up_to(9) {
        0 => "idle",
        1 => "drain_votes",
        2 => "drain_rbc_chunks",
        3 => "drain_block_payloads",
        4 => "drain_blocks",
        5 => "tick",
        6 => "drain_consensus",
        7 => "drain_lane_relay",
        _ => "drain_background",
    };
    SumeragiWorkerLoopStatus {
        stage: stage.to_string(),
        stage_started_ms: rng.next_u64(),
        last_iteration_ms: rng.next_u64(),
        queue_depths: rng_worker_queue_depths(rng),
        queue_diagnostics: rng_worker_queue_diagnostics(rng),
    }
}

fn rng_commit_inflight_status(rng: &mut DeterministicRng) -> SumeragiCommitInflightStatus {
    SumeragiCommitInflightStatus {
        active: rng.next_bool(),
        id: rng.next_u64(),
        height: rng.next_u64(),
        view: rng.next_u64(),
        block_hash: rng.next_bool().then(|| sample_block_hash(rng.next_u8())),
        started_ms: rng.next_u64(),
        elapsed_ms: rng.next_u64(),
        timeout_ms: rng.next_u64(),
        timeout_total: rng.next_u64(),
        last_timeout_timestamp_ms: rng.next_u64(),
        last_timeout_elapsed_ms: rng.next_u64(),
        last_timeout_height: rng.next_u64(),
        last_timeout_view: rng.next_u64(),
        last_timeout_block_hash: rng.next_bool().then(|| sample_block_hash(rng.next_u8())),
        pause_total: rng.next_u64(),
        resume_total: rng.next_u64(),
        paused_since_ms: rng.next_u64(),
        pause_queue_depths: rng_worker_queue_depths(rng),
        resume_queue_depths: rng_worker_queue_depths(rng),
    }
}

fn rng_commit_quorum_status(rng: &mut DeterministicRng) -> SumeragiCommitQuorumStatus {
    SumeragiCommitQuorumStatus {
        height: rng.next_u64(),
        view: rng.next_u64(),
        block_hash: rng.next_bool().then(|| rng_block_hash(rng)),
        signatures_present: rng.next_u64(),
        signatures_counted: rng.next_u64(),
        signatures_set_b: rng.next_u64(),
        signatures_required: rng.next_u64(),
        last_updated_ms: rng.next_u64(),
    }
}

fn rng_commit_qc_status(rng: &mut DeterministicRng) -> SumeragiQcStatus {
    SumeragiQcStatus {
        height: rng.next_u64(),
        view: rng.next_u64(),
        epoch: rng.next_u64(),
        block_hash: rng.next_bool().then(|| rng_block_hash(rng)),
        validator_set_hash: rng
            .next_bool()
            .then(|| HashOf::<Vec<PeerId>>::from_untyped_unchecked(rng_hash(rng))),
        validator_set_len: rng.next_u64(),
        signatures_total: rng.next_u64(),
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn sumeragi_wire_status_roundtrip() {
    let mut rng = DeterministicRng::new(0xABCD_DCBA);
    let relay = rng_lane_relay_envelope(&mut rng);
    let status = SumeragiStatusWire {
        mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_string(),
        staged_mode_tag: Some("iroha2-consensus::npos-sumeragi@v1".to_string()),
        staged_mode_activation_height: Some(42),
        mode_activation_lag_blocks: Some(2),
        mode_flip_kill_switch: true,
        mode_flip_blocked: false,
        mode_flip_success_total: 2,
        mode_flip_fail_total: 1,
        mode_flip_blocked_total: 0,
        last_mode_flip_timestamp_ms: Some(123),
        last_mode_flip_error: None,
        consensus_caps: Some(SumeragiConsensusCapsStatus {
            collectors_k: 1,
            redundant_send_r: 1,
            da_enabled: true,
            rbc_chunk_max_bytes: 65536,
            rbc_session_ttl_ms: 120_000,
            rbc_store_max_sessions: 1024,
            rbc_store_soft_sessions: 512,
            rbc_store_max_bytes: 256 * 1024 * 1024,
            rbc_store_soft_bytes: 128 * 1024 * 1024,
        }),
        effective_min_finality_ms: 100,
        effective_block_time_ms: 1_000,
        effective_commit_time_ms: 1_500,
        effective_pacing_factor_bps: 12_500,
        effective_commit_quorum_timeout_ms: 3_000,
        effective_availability_timeout_ms: 2_500,
        effective_pacemaker_interval_ms: 900,
        effective_npos_timeouts: Some(SumeragiNposTimeoutsStatus {
            propose_ms: 200,
            prevote_ms: 210,
            precommit_ms: 220,
            commit_ms: 230,
            da_ms: 240,
            aggregator_ms: 250,
            exec_ms: 260,
            witness_ms: 270,
        }),
        effective_collectors_k: 1,
        effective_redundant_send_r: 1,
        leader_index: 3,
        highest_qc_height: 15,
        highest_qc_view: 6,
        highest_qc_subject: Some(sample_block_hash(0x90)),
        locked_qc_height: 14,
        locked_qc_view: 5,
        locked_qc_subject: None,
        commit_qc: SumeragiQcStatus {
            height: 12,
            view: 4,
            epoch: 1,
            block_hash: Some(sample_block_hash(0x91)),
            validator_set_hash: Some(HashOf::<Vec<PeerId>>::from_untyped_unchecked(
                Hash::prehashed([0x92; Hash::LENGTH]),
            )),
            validator_set_len: 4,
            signatures_total: 3,
        },
        commit_quorum: SumeragiCommitQuorumStatus {
            height: 12,
            view: 4,
            block_hash: Some(sample_block_hash(0x93)),
            signatures_present: 4,
            signatures_counted: 3,
            signatures_set_b: 1,
            signatures_required: 4,
            last_updated_ms: 456,
        },
        view_change_proof_accepted_total: 9,
        view_change_proof_stale_total: 10,
        view_change_proof_rejected_total: 11,
        view_change_suggest_total: 12,
        view_change_install_total: 13,
        view_change_causes: SumeragiViewChangeCauseStatus {
            commit_failure_total: 1,
            quorum_timeout_total: 2,
            stake_quorum_timeout_total: 0,
            da_gate_total: 3,
            censorship_evidence_total: 7,
            missing_payload_total: 4,
            missing_qc_total: 5,
            validation_reject_total: 6,
            last_cause: Some("quorum_timeout".to_string()),
            last_cause_timestamp_ms: 42,
            last_commit_failure_timestamp_ms: 10,
            last_quorum_timeout_timestamp_ms: 11,
            last_stake_quorum_timeout_timestamp_ms: 0,
            last_da_gate_timestamp_ms: 12,
            last_censorship_evidence_timestamp_ms: 16,
            last_missing_payload_timestamp_ms: 13,
            last_missing_qc_timestamp_ms: 14,
            last_validation_reject_timestamp_ms: 15,
        },
        gossip_fallback_total: 7,
        block_created_dropped_by_lock_total: 2,
        block_created_hint_mismatch_total: 1,
        block_created_proposal_mismatch_total: 4,
        consensus_message_handling: SumeragiConsensusMessageHandlingStatus {
            entries: vec![SumeragiConsensusMessageHandlingEntry {
                kind: "block_created".to_string(),
                outcome: "dropped".to_string(),
                reason: "hint_mismatch".to_string(),
                total: 3,
            }],
        },
        vote_validation_drops: SumeragiVoteValidationDropStatus {
            total: 2,
            entries: vec![SumeragiVoteValidationDropEntry {
                reason: "invalid_signature".to_string(),
                height: 9,
                view: 2,
                epoch: 0,
                signer_index: 1,
                peer_id: Some(PeerId::from(KeyPair::random().public_key().clone())),
                roster_hash: Some(HashOf::<Vec<PeerId>>::from_untyped_unchecked(
                    Hash::prehashed([0xAE; Hash::LENGTH]),
                )),
                roster_len: 4,
                block_hash: sample_block_hash(0x94),
                timestamp_ms: 456,
            }],
            peer_entries: vec![SumeragiVoteValidationDropPeerEntry {
                peer_id: PeerId::from(KeyPair::random().public_key().clone()),
                roster_hash: Some(HashOf::<Vec<PeerId>>::from_untyped_unchecked(
                    Hash::prehashed([0xAF; Hash::LENGTH]),
                )),
                roster_len: 4,
                total: 2,
                reasons: vec![SumeragiVoteValidationDropReasonCount {
                    reason: "invalid_signature".to_string(),
                    total: 2,
                }],
                last_height: 9,
                last_view: 2,
                last_epoch: 0,
                last_timestamp_ms: 789,
            }],
        },
        validation_reject_total: 1,
        validation_reject_reason: Some("stateless".to_string()),
        validation_rejects: SumeragiValidationRejectStatus {
            total: 1,
            stateless_total: 1,
            execution_total: 0,
            prev_hash_total: 0,
            prev_height_total: 0,
            topology_total: 0,
            last_reason: Some("stateless".to_string()),
            last_height: Some(5),
            last_view: Some(3),
            last_block: Some(sample_block_hash(0x91)),
            last_timestamp_ms: 123,
        },
        peer_key_policy: SumeragiPeerKeyPolicyStatus {
            total: 2,
            missing_hsm_total: 1,
            disallowed_algorithm_total: 0,
            disallowed_provider_total: 0,
            lead_time_violation_total: 0,
            activation_in_past_total: 0,
            expiry_before_activation_total: 0,
            identifier_collision_total: 1,
            last_reason: Some("identifier_collision".to_string()),
            last_timestamp_ms: 456,
        },
        block_sync_roster: SumeragiBlockSyncRosterStatus {
            commit_qc_hint_total: 2,
            checkpoint_hint_total: 3,
            commit_qc_history_total: 4,
            checkpoint_history_total: 5,
            roster_sidecar_total: 6,
            commit_roster_journal_total: 7,
            drop_missing_total: 10,
            drop_unsolicited_share_blocks_total: 11,
        },
        pacemaker_backpressure_deferrals_total: 8,
        commit_pipeline_tick_total: 0,
        da_reschedule_total: 0,
        missing_block_fetch: SumeragiMissingBlockFetchStatus {
            total: 3,
            last_targets: 2,
            last_dwell_ms: 7,
        },
        committed_edge_conflict_obsolete_total: 9,
        roster_sidecar_mismatch_obsolete_total: 4,
        da_gate: SumeragiDaGateStatus {
            reason: SumeragiDaGateReason::MissingLocalData,
            last_satisfied: SumeragiDaGateSatisfaction::MissingDataRecovered,
            missing_local_data_total: 1,
            manifest_guard_total: 5,
        },
        kura_store: SumeragiKuraStoreStatus {
            failures_total: 1,
            abort_total: 2,
            last_retry_attempt: 3,
            last_retry_backoff_ms: 15,
            last_height: 5,
            last_view: 6,
            last_hash: Some(sample_block_hash(0x55)),
            ..Default::default()
        },
        rbc_store: SumeragiRbcStoreStatus {
            sessions: 4,
            bytes: 1024,
            pressure_level: 2,
            backpressure_deferrals_total: 1,
            persist_drops_total: 2,
            evictions_total: 3,
            recent_evictions: vec![SumeragiRbcEvictedSession {
                block_hash: sample_block_hash(0x77),
                height: 10,
                view: 2,
            }],
        },
        rbc_mismatch: SumeragiRbcMismatchStatus {
            entries: vec![SumeragiRbcMismatchEntry {
                peer_id: PeerId::from(KeyPair::random().public_key().clone()),
                chunk_digest_mismatch_total: 2,
                payload_hash_mismatch_total: 1,
                chunk_root_mismatch_total: 3,
                last_timestamp_ms: 42,
            }],
        },
        pending_rbc: SumeragiPendingRbcStatus {
            sessions: 2,
            session_cap: 256,
            chunks: 3,
            bytes: 512,
            max_chunks_per_session: 128,
            max_bytes_per_session: 8_192,
            ttl_ms: 30_000,
            drops_total: 4,
            drops_cap_total: 3,
            drops_cap_bytes_total: 12,
            drops_ttl_total: 1,
            drops_ttl_bytes_total: 4,
            drops_bytes_total: 16,
            evicted_total: 2,
            stash_ready_total: 3,
            stash_ready_init_missing_total: 1,
            stash_ready_roster_missing_total: 1,
            stash_ready_roster_hash_mismatch_total: 0,
            stash_ready_roster_unverified_total: 1,
            stash_deliver_total: 2,
            stash_deliver_init_missing_total: 1,
            stash_deliver_roster_missing_total: 0,
            stash_deliver_roster_hash_mismatch_total: 1,
            stash_deliver_roster_unverified_total: 0,
            stash_chunk_total: 5,
            entries: vec![SumeragiPendingRbcEntry {
                block_hash: sample_block_hash(0x80),
                height: 5,
                view: 1,
                chunks: 2,
                bytes: 256,
                ready: 1,
                deliver: 0,
                dropped_chunks: 1,
                dropped_bytes: 128,
                dropped_ready: 0,
                dropped_deliver: 0,
                age_ms: 250,
            }],
        },
        tx_queue_depth: 11,
        tx_queue_capacity: 64,
        tx_queue_saturated: true,
        epoch_length_blocks: 3600,
        epoch_commit_deadline_offset: 120,
        epoch_reveal_deadline_offset: 160,
        prf_epoch_seed: Some([0x11; 32]),
        prf_height: 15,
        prf_view: 6,
        vrf_penalty_epoch: 4,
        vrf_committed_no_reveal_total: 2,
        vrf_no_participation_total: 1,
        vrf_late_reveals_total: 3,
        consensus_penalties_applied_total: 1,
        consensus_penalties_pending: 0,
        vrf_penalties_applied_total: 2,
        vrf_penalties_pending: 0,
        membership: SumeragiMembershipStatus {
            height: 15,
            view: 6,
            epoch: 4,
            view_hash: Some([0xAB; 32]),
        },
        membership_mismatch: SumeragiMembershipMismatchStatus {
            active_peers: vec![PeerId::from(KeyPair::random().public_key().clone())],
            last_peer: Some(PeerId::from(KeyPair::random().public_key().clone())),
            last_height: 14,
            last_view: 5,
            last_epoch: 4,
            last_local_hash: Some([0x10; 32]),
            last_remote_hash: Some([0x20; 32]),
            last_timestamp_ms: 1_700_000_000_000,
        },
        lane_commitments: vec![SumeragiLaneCommitment {
            block_height: 15,
            lane_id: LaneId::new(2),
            tx_count: 5,
            total_chunks: 3,
            rbc_bytes_total: 256,
            teu_total: 99,
            block_hash: sample_block_hash(0x91),
        }],
        dataspace_commitments: vec![SumeragiDataspaceCommitment {
            block_height: 15,
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(9),
            tx_count: 2,
            total_chunks: 1,
            rbc_bytes_total: 96,
            teu_total: 55,
            block_hash: sample_block_hash(0x92),
        }],
        lane_settlement_commitments: Vec::new(),
        lane_relay_envelopes: vec![relay],
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: vec![SumeragiLaneGovernance {
            lane_id: LaneId::new(2),
            alias: "governance-lane".to_owned(),
            governance: Some("parliament".to_owned()),
            manifest_required: true,
            manifest_ready: true,
            manifest_path: Some("/etc/iroha/lanes/governance-lane.json".to_owned()),
            validator_ids: vec!["validator@test".to_owned()],
            quorum: Some(2),
            protected_namespaces: vec!["finance".to_owned()],
            runtime_upgrade: Some(SumeragiRuntimeUpgradeHook {
                allow: true,
                require_metadata: true,
                metadata_key: Some("gov_upgrade_id".to_owned()),
                allowed_ids: vec!["upgrade-2026Q1".to_owned()],
            }),
        }],
        worker_loop: SumeragiWorkerLoopStatus {
            stage: "tick".to_string(),
            stage_started_ms: 1_700_000_000_000,
            last_iteration_ms: 250,
            queue_depths: SumeragiWorkerQueueDepths {
                vote_rx: 1,
                block_payload_rx: 2,
                rbc_chunk_rx: 3,
                block_rx: 4,
                consensus_rx: 5,
                lane_relay_rx: 6,
                background_rx: 7,
            },
            queue_diagnostics: SumeragiWorkerQueueDiagnostics {
                blocked_total: SumeragiWorkerQueueTotals {
                    vote_rx: 1,
                    block_payload_rx: 2,
                    rbc_chunk_rx: 3,
                    block_rx: 4,
                    consensus_rx: 5,
                    lane_relay_rx: 6,
                    background_rx: 7,
                },
                blocked_ms_total: SumeragiWorkerQueueTotals {
                    vote_rx: 10,
                    block_payload_rx: 20,
                    rbc_chunk_rx: 30,
                    block_rx: 40,
                    consensus_rx: 50,
                    lane_relay_rx: 60,
                    background_rx: 70,
                },
                blocked_max_ms: SumeragiWorkerQueueTotals {
                    vote_rx: 3,
                    block_payload_rx: 4,
                    rbc_chunk_rx: 5,
                    block_rx: 6,
                    consensus_rx: 7,
                    lane_relay_rx: 8,
                    background_rx: 9,
                },
                dropped_total: SumeragiWorkerQueueTotals {
                    vote_rx: 4,
                    block_payload_rx: 5,
                    rbc_chunk_rx: 6,
                    block_rx: 7,
                    consensus_rx: 8,
                    lane_relay_rx: 9,
                    background_rx: 10,
                },
            },
        },
        commit_inflight: SumeragiCommitInflightStatus {
            active: true,
            id: 12,
            height: 19,
            view: 3,
            block_hash: Some(sample_block_hash(0xAC)),
            started_ms: 1_700_000_123_456,
            elapsed_ms: 1_234,
            timeout_ms: 5_000,
            timeout_total: 2,
            last_timeout_timestamp_ms: 1_700_000_124_000,
            last_timeout_elapsed_ms: 4_321,
            last_timeout_height: 17,
            last_timeout_view: 2,
            last_timeout_block_hash: Some(sample_block_hash(0xAD)),
            pause_total: 4,
            resume_total: 3,
            paused_since_ms: 1_700_000_123_900,
            pause_queue_depths: SumeragiWorkerQueueDepths {
                vote_rx: 5,
                block_payload_rx: 4,
                rbc_chunk_rx: 3,
                block_rx: 2,
                consensus_rx: 1,
                lane_relay_rx: 0,
                background_rx: 9,
            },
            resume_queue_depths: SumeragiWorkerQueueDepths {
                vote_rx: 6,
                block_payload_rx: 7,
                rbc_chunk_rx: 8,
                block_rx: 9,
                consensus_rx: 10,
                lane_relay_rx: 11,
                background_rx: 12,
            },
        },
        commit_pipeline: SumeragiCommitPipelineStatus {
            last_total_ms: 84,
            last_validation_ms: 23,
            last_qc_rebuild_ms: 7,
            last_gate_ms: 11,
            last_finalize_ms: 19,
            last_drain_results_ms: 13,
            last_drain_qc_verify_ms: 2,
            last_drain_persist_ms: 3,
            last_drain_kura_store_ms: 4,
            last_drain_state_apply_ms: 5,
            last_drain_state_commit_ms: 6,
            ema_total_ms: 80,
            ema_validation_ms: 20,
            ema_gate_ms: 10,
            ema_finalize_ms: 18,
        },
        round_gap: SumeragiRoundGapStatus {
            last_deliver_to_state_commit_ms: 33,
            last_state_commit_to_next_propose_ms: 12,
            last_deliver_to_next_propose_ms: 45,
            ema_deliver_to_state_commit_ms: 31,
            ema_state_commit_to_next_propose_ms: 10,
            ema_deliver_to_next_propose_ms: 42,
        },
    };
    assert_roundtrip(&status);

    let qc_snapshot = SumeragiQcSnapshot {
        highest_qc: SumeragiQcEntry {
            height: 16,
            view: 7,
            subject_block_hash: Some(sample_block_hash(0xA0)),
        },
        locked_qc: SumeragiQcEntry {
            height: 15,
            view: 6,
            subject_block_hash: None,
        },
    };
    assert_roundtrip(&qc_snapshot);
}

#[test]
fn consensus_genesis_norito_roundtrip() {
    let npos = NposGenesisParams {
        block_time_ms: 1_000,
        timeout_propose_ms: 300,
        timeout_prevote_ms: 300,
        timeout_precommit_ms: 250,
        timeout_commit_ms: 150,
        timeout_da_ms: 320,
        timeout_aggregator_ms: 120,
        k_aggregators: 3,
        redundant_send_r: 2,
        epoch_seed: [0x11; 32],
        vrf_commit_window_blocks: 8,
        vrf_reveal_window_blocks: 5,
        max_validators: 20,
        min_self_bond: 10,
        min_nomination_bond: 2,
        max_nominator_concentration_pct: 35,
        seat_band_pct: 15,
        max_entity_correlation_pct: 25,
        finality_margin_blocks: 9,
        evidence_horizon_blocks: 1_024,
        activation_lag_blocks: 12,
        slashing_delay_blocks: 17,
    };
    let with_npos = ConsensusGenesisParams {
        block_time_ms: 750,
        commit_time_ms: 1_500,
        min_finality_ms: 100,
        max_clock_drift_ms: 250,
        collectors_k: 4,
        redundant_send_r: 2,
        block_max_transactions: 512,
        da_enabled: true,
        epoch_length_blocks: 120,
        bls_domain: "bls-iroha2:npos-sumeragi:v1".to_owned(),
        npos: Some(npos),
    };
    let without_npos = ConsensusGenesisParams {
        npos: None,
        ..with_npos.clone()
    };

    assert_roundtrip(&npos);
    assert_roundtrip(&with_npos);
    assert_roundtrip(&without_npos);
}

#[allow(clippy::too_many_lines)]
#[test]
fn consensus_messages_norito_roundtrip() {
    let validator_set = vec![
        PeerId::new(
            KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
                .public_key()
                .clone(),
        ),
        PeerId::new(
            KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal)
                .public_key()
                .clone(),
        ),
    ];
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
    };
    let vrf_reveal = VrfReveal {
        epoch: 3,
        reveal: [0x44; 32],
        signer: 5,
    };
    let peer_ids = vec![
        PeerId::from(KeyPair::random().public_key().clone()),
        PeerId::from(KeyPair::random().public_key().clone()),
    ];
    let reconfig = Reconfig {
        new_roster: peer_ids,
        activation_height: 100,
    };
    let roster = vec![
        PeerId::from(KeyPair::random().public_key().clone()),
        PeerId::from(KeyPair::random().public_key().clone()),
    ];
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
    let leader_key = KeyPair::random();
    let (_, leader_private) = leader_key.into_parts();
    let leader_signature = BlockSignature::new(
        0,
        SignatureOf::from_hash(&leader_private, block_header.hash()),
    );
    let rbc_init = RbcInit {
        block_hash: block_header.hash(),
        height: 44,
        view: 7,
        epoch: 2,
        roster,
        roster_hash,
        total_chunks: 3,
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
        let status = rng_sumeragi_status(&mut rng);
        assert_roundtrip(&status);

        let qc_snapshot = rng_sumeragi_qc_snapshot(&mut rng);
        assert_roundtrip(&qc_snapshot);

        let genesis = rng_consensus_genesis_params(&mut rng);
        if let Some(ref npos) = genesis.npos {
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

    let mut seen = 0usize;
    for entry in fs::read_dir(&fixtures_dir).expect("read lane commitment fixtures") {
        let entry = entry.expect("fixture entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read lane commitment fixture {path:?}: {err}"));
        let commitment: LaneBlockCommitment = norito::json::from_str(&raw)
            .unwrap_or_else(|err| panic!("parse lane commitment fixture {path:?}: {err}"));
        let reserialized =
            norito::json::to_json_pretty(&commitment).expect("serialize commitment to JSON");
        let replay: LaneBlockCommitment =
            norito::json::from_str(&reserialized).expect("parse reserialized commitment");
        assert_eq!(
            commitment, replay,
            "JSON roundtrip mismatch for fixture {path:?}"
        );
        let norito_bytes =
            norito::to_bytes(&commitment).expect("encode commitment to Norito bytes");
        let archived =
            norito::from_bytes::<LaneBlockCommitment>(&norito_bytes).expect("archive commitment");
        let decoded = NoritoDeserialize::try_deserialize(archived)
            .expect("deserialize commitment from Norito bytes");
        assert_eq!(
            commitment, decoded,
            "Norito roundtrip mismatch for fixture {path:?}"
        );
        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("fixture stem");
        let to_path = fixtures_dir.join(format!("{stem}.to"));
        if to_path.is_file() {
            let fixture_bytes = fs::read(&to_path)
                .unwrap_or_else(|err| panic!("read Norito bytes {to_path:?}: {err}"));
            let archived_file =
                norito::from_bytes::<LaneBlockCommitment>(&fixture_bytes).expect("archive fixture");
            let decoded_from_file = NoritoDeserialize::try_deserialize(archived_file)
                .expect("deserialize fixture Norito bytes");
            assert_eq!(
                commitment, decoded_from_file,
                "Norito fixture bytes mismatch for {to_path:?}"
            );
            assert_eq!(
                norito_bytes, fixture_bytes,
                "canonical Norito bytes do not match fixture {to_path:?}"
            );
        }
        seen += 1;
    }

    assert!(
        seen > 0,
        "expected at least one lane commitment fixture under {fixtures_dir:?}"
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root directory exists")
        .to_path_buf()
}
