//! Consensus model shape, ownership and exact codec contracts.

use super::*;
use crate::block::consensus_v2::PERMISSIONED_TAG;
use crate::consensus::VALIDATOR_SET_HASH_VERSION_V1;
use iroha_crypto::{Algorithm, KeyPair, MerkleProof, MerkleTree, MerkleTreeCommitment};
use iroha_primitives::numeric::{Numeric, Quantity};
use norito::core::DecodeFromSlice;
use std::num::NonZeroU64;
#[derive(Clone, Copy, norito::codec::Encode)]
struct RetiredQcRefFixture {
    height: Height,
    view: View,
    epoch: u64,
    subject_block_hash: HashOf<BlockHeader>,
    phase: CertPhase,
}
#[derive(Clone, Copy, norito::codec::Encode)]
struct RetiredConsensusBlockHeaderFixture {
    parent_hash: HashOf<BlockHeader>,
    tx_root: Hash,
    state_root: Hash,
    proposer: ValidatorIndex,
    height: Height,
    view: View,
    epoch: u64,
    highest_qc: RetiredQcRefFixture,
}
#[derive(Clone, Copy, norito::codec::Encode)]
struct RetiredProposalFixture {
    header: RetiredConsensusBlockHeaderFixture,
    payload_hash: Hash,
}
#[derive(Clone, norito::codec::Encode)]
struct RetiredQcVoteFixture {
    phase: CertPhase,
    block_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    height: Height,
    view: View,
    epoch: u64,
    chain_order_hash: Hash,
    rechain_seq: u64,
    highest_qc: Option<RetiredQcRefFixture>,
    signer: ValidatorIndex,
    bls_sig: Vec<u8>,
}
#[derive(Clone, norito::codec::Encode)]
struct RetiredQcAggregateFixture {
    signers_bitmap: Vec<u8>,
    bls_aggregate_signature: Vec<u8>,
}
#[derive(Clone, norito::codec::Encode)]
struct RetiredQcFixture {
    phase: CertPhase,
    subject_block_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    height: Height,
    view: View,
    epoch: u64,
    chain_order_hash: Hash,
    rechain_seq: u64,
    mode_tag: String,
    highest_qc: Option<RetiredQcRefFixture>,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set_hash_version: u16,
    validator_set: Vec<PeerId>,
    aggregate: RetiredQcAggregateFixture,
}
#[expect(
    dead_code,
    reason = "all retired discriminants are retained solely to encode decode-negative fixtures"
)]
#[derive(norito::codec::Encode)]
enum RetiredEvidenceKind {
    DoublePrepare,
    DoubleCommit,
    InvalidQc,
    InvalidProposal,
    Censorship,
    SumeragiV2Equivocation,
}
#[expect(
    dead_code,
    clippy::large_enum_variant,
    reason = "all retired discriminants are retained solely to encode decode-negative fixtures"
)]
#[derive(norito::codec::Encode)]
enum RetiredEvidencePayload {
    DoubleVote {
        v1: RetiredQcVoteFixture,
        v2: RetiredQcVoteFixture,
    },
    InvalidQc {
        certificate: RetiredQcFixture,
        reason: String,
    },
    InvalidProposal {
        proposal: RetiredProposalFixture,
        reason: String,
    },
    Censorship {
        tx_hash: HashOf<crate::transaction::SignedTransaction>,
        receipts: Vec<crate::transaction::TransactionSubmissionReceipt>,
    },
    SumeragiV2Equivocation(SumeragiV2EquivocationEvidence),
}
#[derive(norito::codec::Encode)]
struct RetiredEvidence {
    kind: RetiredEvidenceKind,
    payload: RetiredEvidencePayload,
}
#[derive(norito::codec::Encode)]
struct RetiredEvidenceRecord {
    evidence: RetiredEvidence,
    recorded_at_height: Height,
    recorded_at_view: View,
    recorded_at_ms: u64,
    #[norito(default)]
    penalty_applied: bool,
    #[norito(default)]
    penalty_cancelled: bool,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    penalty_cancelled_at_height: Option<Height>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    penalty_applied_at_height: Option<Height>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    consensus_admitted_at_height: Option<Height>,
}
fn dummy_hash() -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::prehashed([0u8; 32]))
}
fn retired_chain_order_hash() -> Hash {
    Hash::new(b"iroha:sumeragi:v1:chain-order:default")
}
fn checked_random_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked consensus fixture keypair")
}
fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("generate checked consensus fixture keypair")
}

#[test]
fn manual_consensus_json_labels_have_closed_output_bounds() {
    fn assert_bounded<T: norito::json::JsonSerialize>(value: &T) {
        let expected = norito::json::to_json(value).expect("serialize ordinary JSON");
        assert_eq!(
            norito::json::to_json_bounded(value, expected.len())
                .expect("serialize at exact JSON bound"),
            expected
        );
        assert_eq!(
            norito::json::to_json_bounded(value, expected.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }
    assert_bounded(&SumeragiAutonomousLaneExecutionStage::QueueFinalized);
    assert_bounded(&SumeragiAutonomousLaneExecutionStuckReason::QueueFinalizationUnverifiable);
    assert_bounded(&SumeragiNativeAmxParticipantApplicationState::DurablyApplied);
}
fn sample_roster() -> Vec<PeerId> {
    (0..3)
        .map(|_| {
            PeerId::new(
                checked_random_keypair_with_algorithm(Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            )
        })
        .collect()
}
include!("consensus/wire_schema_tests.rs");
#[test]
fn committed_lane_block_status_progress_policy_is_fail_closed() {
    for (status, executable) in [
        (COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD, false),
        (
            COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR,
            true,
        ),
        (
            COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
            true,
        ),
        (
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION,
            true,
        ),
        (COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK, true),
    ] {
        assert!(
            committed_lane_block_status_counts_as_progress(status, executable),
            "{status} with matching availability should count as audited progress"
        );
    }
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT,
        false
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
        false
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
        true
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
        false
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
        true
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        "future_status",
        true
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
        true
    ));
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
        false
    ));
}
#[derive(Encode)]
struct ForgedNexusFeeScheduleInputs {
    tx_bytes_len: u64,
    instruction_count: u64,
    gas_used: u64,
    base_fee: Numeric,
    per_byte_fee: Numeric,
    per_instruction_fee: Numeric,
    per_gas_unit_fee: Numeric,
}
#[derive(Encode)]
struct ForgedNexusFeeReceipt {
    version: u16,
    source_id: [u8; 32],
    dataspace_id: DataSpaceId,
    lane_id: LaneId,
    block_height: u64,
    debit_source: FeeDebitSource,
    fee_asset_id: AssetDefinitionId,
    program_revision: Option<u64>,
    lease_id: Option<Hash>,
    fee_amount: Numeric,
    schedule: NexusFeeScheduleInputs,
}
#[derive(Encode)]
struct ForgedNposGenesisParams {
    epoch_length_blocks: NonZeroU64,
    epoch_seed: [u8; 32],
    max_validators: u32,
    min_self_bond: Numeric,
    min_nomination_bond: Numeric,
    max_nominator_concentration_pct: u8,
    seat_band_pct: u8,
    max_entity_correlation_pct: u8,
    finality_margin_blocks: u64,
    evidence_horizon_blocks: u64,
    activation_lag_blocks: u64,
    slashing_delay_blocks: u64,
}
#[derive(Encode)]
struct ForgedLaneSettlementReceipt {
    source_id: [u8; 32],
    local_amount: Numeric,
    xor_due: Numeric,
    xor_after_haircut: Numeric,
    xor_variance: Numeric,
    timestamp_ms: u64,
}
#[derive(Encode)]
struct ForgedLaneBlockCommitment {
    block_height: u64,
    lane_id: LaneId,
    lane_incarnation: Hash,
    dataspace_id: DataSpaceId,
    tx_count: u64,
    total_local_amount: Numeric,
    total_xor_due: Numeric,
    total_xor_after_haircut: Numeric,
    total_xor_variance: Numeric,
    swap_metadata: Option<LaneSwapMetadata>,
    receipts: Vec<LaneSettlementReceipt>,
    nexus_fee_receipts: Vec<NexusFeeReceipt>,
    native_amx_receipts: Vec<NativeAmxReceipt>,
}
fn sample_nexus_fee_receipt(source_id: [u8; 32]) -> NexusFeeReceipt {
    NexusFeeReceipt {
        version: NexusFeeReceipt::VERSION,
        source_id,
        dataspace_id: DataSpaceId::new(7),
        lane_id: LaneId::new(1),
        block_height: 42,
        debit_source: FeeDebitSource::Account(crate::account::AccountId::new(
            checked_random_keypair_with_algorithm(Algorithm::Ed25519)
                .public_key()
                .clone(),
        )),
        fee_asset_id: "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
            .parse()
            .expect("canonical asset definition id"),
        program_revision: None,
        lease_id: None,
        fee_amount: "0.001".parse().expect("quantity"),
        schedule: NexusFeeScheduleInputs {
            tx_bytes_len: 100,
            instruction_count: 1,
            gas_used: 0,
            base_fee: Quantity::zero(),
            per_byte_fee: Quantity::zero(),
            per_instruction_fee: "0.001".parse().expect("quantity"),
            per_gas_unit_fee: Quantity::zero(),
        },
    }
}
#[test]
fn negative_numeric_payloads_cannot_decode_as_nexus_fees() {
    let forged_schedule = ForgedNexusFeeScheduleInputs {
        tx_bytes_len: 1,
        instruction_count: 1,
        gas_used: 1,
        base_fee: Numeric::new(-1_i32, 0),
        per_byte_fee: Numeric::zero(),
        per_instruction_fee: Numeric::zero(),
        per_gas_unit_fee: Numeric::zero(),
    };
    let encoded = forged_schedule.encode();
    assert!(
        NexusFeeScheduleInputs::decode(&mut encoded.as_slice()).is_err(),
        "a negative signed payload must not decode as a fee schedule component"
    );
    let valid = sample_nexus_fee_receipt([0xA5; 32]);
    let forged_receipt = ForgedNexusFeeReceipt {
        version: valid.version,
        source_id: valid.source_id,
        dataspace_id: valid.dataspace_id,
        lane_id: valid.lane_id,
        block_height: valid.block_height,
        debit_source: valid.debit_source,
        fee_asset_id: valid.fee_asset_id,
        program_revision: valid.program_revision,
        lease_id: valid.lease_id,
        fee_amount: Numeric::new(-1_i32, 0),
        schedule: valid.schedule,
    };
    let encoded = forged_receipt.encode();
    assert!(
        NexusFeeReceipt::decode(&mut encoded.as_slice()).is_err(),
        "a negative signed payload must not decode as a fee receipt amount"
    );
}
#[test]
fn sponsored_nexus_fee_receipt_roundtrips_typed_source_and_asset() {
    let mut receipt = sample_nexus_fee_receipt([0x5A; 32]);
    receipt.debit_source = FeeDebitSource::SponsorProgram(crate::nexus::FeeSponsorProgramId::new(
        crate::account::AccountId::new(
            checked_random_keypair_with_algorithm(Algorithm::Ed25519)
                .public_key()
                .clone(),
        ),
        "retail".parse().expect("program name"),
    ));
    receipt.program_revision = Some(4);
    receipt.lease_id = Some(Hash::new(b"receipt-spend-lease"));
    let bytes = receipt.encode();
    assert_eq!(
        NexusFeeReceipt::decode(&mut bytes.as_slice()).expect("decode sponsored receipt"),
        receipt
    );
    let json = norito::json::to_json(&receipt).expect("serialize sponsored receipt");
    assert_eq!(
        norito::json::from_str::<NexusFeeReceipt>(&json).expect("deserialize sponsored receipt"),
        receipt
    );
}

#[test]
fn nexus_fee_receipt_rejects_pre_release_layout_without_nullable_bindings() {
    #[derive(Encode)]
    struct PreReleaseNexusFeeReceipt {
        version: u16,
        source_id: [u8; 32],
        dataspace_id: DataSpaceId,
        lane_id: LaneId,
        block_height: u64,
        debit_source: FeeDebitSource,
        fee_asset_id: crate::asset::AssetDefinitionId,
        fee_amount: Quantity,
        schedule: NexusFeeScheduleInputs,
    }

    let receipt = sample_nexus_fee_receipt([0x5C; 32]);
    let bytes = PreReleaseNexusFeeReceipt {
        version: receipt.version,
        source_id: receipt.source_id,
        dataspace_id: receipt.dataspace_id,
        lane_id: receipt.lane_id,
        block_height: receipt.block_height,
        debit_source: receipt.debit_source,
        fee_asset_id: receipt.fee_asset_id,
        fee_amount: receipt.fee_amount,
        schedule: receipt.schedule,
    }
    .encode();
    assert!(
        NexusFeeReceipt::decode_all(&mut bytes.as_slice()).is_err(),
        "the first-release fee receipt must reject the layout without revision and lease slots"
    );
}

#[test]
fn nexus_fee_receipt_json_requires_nullable_bindings_and_closed_nested_schedule() {
    let receipt = sample_nexus_fee_receipt([0x5D; 32]);
    for field in ["program_revision", "lease_id"] {
        let mut value = norito::json::to_value(&receipt).expect("serialize Nexus fee receipt");
        assert!(
            value
                .as_object_mut()
                .expect("Nexus fee receipt JSON object")
                .remove(field)
                .is_some(),
            "fixture must contain nullable field {field}"
        );
        assert!(
            norito::json::from_value::<NexusFeeReceipt>(value).is_err(),
            "the first-release Nexus fee receipt must require {field}"
        );
    }

    let mut unknown = norito::json::to_value(&receipt).expect("serialize Nexus fee receipt");
    unknown
        .as_object_mut()
        .expect("Nexus fee receipt JSON object")
        .insert("pre_release_field".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<NexusFeeReceipt>(unknown).is_err(),
        "the first-release Nexus fee receipt must reject unknown fields"
    );

    let mut unknown_schedule =
        norito::json::to_value(&receipt.schedule).expect("serialize Nexus fee schedule");
    unknown_schedule
        .as_object_mut()
        .expect("Nexus fee schedule JSON object")
        .insert("pre_release_field".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<NexusFeeScheduleInputs>(unknown_schedule).is_err(),
        "the first-release Nexus fee schedule must reject unknown fields"
    );
}

#[test]
fn negative_numeric_payloads_cannot_decode_as_npos_bonds() {
    let forged = ForgedNposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(10).expect("nonzero epoch"),
        epoch_seed: [1; 32],
        max_validators: 4,
        min_self_bond: Numeric::new(-1_i32, 0),
        min_nomination_bond: Numeric::one(),
        max_nominator_concentration_pct: 100,
        seat_band_pct: 10,
        max_entity_correlation_pct: 100,
        finality_margin_blocks: 1,
        evidence_horizon_blocks: 10,
        activation_lag_blocks: 1,
        slashing_delay_blocks: 1,
    };
    let encoded = forged.encode();
    assert!(
        NposGenesisParams::decode(&mut encoded.as_slice()).is_err(),
        "a negative signed payload must not decode as an NPoS minimum bond"
    );
}
#[test]
fn negative_numeric_payloads_cannot_decode_as_lane_amounts() {
    let forged_receipt = ForgedLaneSettlementReceipt {
        source_id: [0xA5; 32],
        local_amount: Numeric::new(-1_i32, 0),
        xor_due: Numeric::one(),
        xor_after_haircut: Numeric::one(),
        xor_variance: Numeric::zero(),
        timestamp_ms: 1,
    };
    let encoded = forged_receipt.encode();
    assert!(
        LaneSettlementReceipt::decode(&mut encoded.as_slice()).is_err(),
        "a negative signed payload must not decode as a lane receipt amount"
    );
    let forged_commitment = ForgedLaneBlockCommitment {
        block_height: 1,
        lane_id: LaneId::SINGLE,
        lane_incarnation: Hash::new(b"negative lane quantity fixture"),
        dataspace_id: DataSpaceId::UNIVERSAL,
        tx_count: 0,
        total_local_amount: Numeric::new(-1_i32, 0),
        total_xor_due: Numeric::zero(),
        total_xor_after_haircut: Numeric::zero(),
        total_xor_variance: Numeric::zero(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let encoded = forged_commitment.encode();
    assert!(
        LaneBlockCommitment::decode(&mut encoded.as_slice()).is_err(),
        "a negative signed payload must not decode as a lane commitment total"
    );
}

#[test]
fn lane_block_commitment_rejects_pre_release_layout_without_settlement_slots() {
    #[derive(Encode)]
    struct PreReleaseLaneBlockCommitment {
        block_height: u64,
        lane_id: LaneId,
        lane_incarnation: Hash,
        dataspace_id: DataSpaceId,
        tx_count: u64,
        total_local_amount: Quantity,
        total_xor_due: Quantity,
        total_xor_after_haircut: Quantity,
        total_xor_variance: Quantity,
    }

    let bytes = PreReleaseLaneBlockCommitment {
        block_height: 1,
        lane_id: LaneId::SINGLE,
        lane_incarnation: Hash::new(b"pre-release lane commitment"),
        dataspace_id: DataSpaceId::UNIVERSAL,
        tx_count: 0,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
    }
    .encode();
    assert!(
        LaneBlockCommitment::decode_all(&mut bytes.as_slice()).is_err(),
        "the first-release lane commitment must reject the layout without settlement collections"
    );
}

#[test]
fn lane_block_commitment_json_requires_exact_settlement_shape() {
    let mut commitment = LaneBlockCommitment {
        block_height: 1,
        lane_id: LaneId::SINGLE,
        lane_incarnation: Hash::new(b"strict lane commitment"),
        dataspace_id: DataSpaceId::UNIVERSAL,
        tx_count: 0,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    for field in [
        "swap_metadata",
        "receipts",
        "nexus_fee_receipts",
        "native_amx_receipts",
    ] {
        let mut value = norito::json::to_value(&commitment).expect("serialize lane commitment");
        assert!(
            value
                .as_object_mut()
                .expect("lane commitment JSON object")
                .remove(field)
                .is_some(),
            "fixture must contain settlement field {field}"
        );
        assert!(
            norito::json::from_value::<LaneBlockCommitment>(value).is_err(),
            "the first-release lane commitment must require {field}"
        );
    }

    commitment.swap_metadata = Some(LaneSwapMetadata {
        epsilon_bps: 25,
        twap_window_seconds: 60,
        liquidity_profile: LaneLiquidityProfile::Tier1,
        twap_local_per_xor: "1".parse().expect("valid TWAP fixture"),
        volatility_class: LaneVolatilityClass::Stable,
    });
    let metadata = commitment.swap_metadata.as_ref().expect("swap metadata");
    let mut missing = norito::json::to_value(metadata).expect("serialize lane swap metadata");
    missing
        .as_object_mut()
        .expect("lane swap metadata JSON object")
        .remove("volatility_class");
    assert!(
        norito::json::from_value::<LaneSwapMetadata>(missing).is_err(),
        "the first-release swap metadata must require its volatility class"
    );

    let mut unknown = norito::json::to_value(metadata).expect("serialize lane swap metadata");
    unknown
        .as_object_mut()
        .expect("lane swap metadata JSON object")
        .insert("pre_release_field".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<LaneSwapMetadata>(unknown).is_err(),
        "the first-release swap metadata must reject unknown fields"
    );
}

#[test]
fn nexus_fee_receipts_change_lane_block_commitment_hash_inputs() {
    let base = LaneBlockCommitment {
        block_height: 42,
        lane_id: LaneId::new(1),
        lane_incarnation: Hash::new(b"commitment-hash-test-incarnation"),
        dataspace_id: DataSpaceId::new(7),
        tx_count: 1,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: vec![sample_nexus_fee_receipt([0x11; 32])],
        native_amx_receipts: Vec::new(),
    };
    let mut changed = base.clone();
    changed.nexus_fee_receipts[0].fee_amount = "0.002".parse().expect("quantity");
    assert_ne!(Hash::new(base.encode()), Hash::new(changed.encode()));
}
fn sample_lane_block_vote_body(phase: CertPhase) -> LaneBlockVoteBodyV1 {
    LaneBlockVoteBodyV1 {
        phase,
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(11),
        lane_incarnation: Hash::new(b"lane-consensus-model-fixture"),
        proposal_height: 12,
        lane_block_height: 13,
        lane_block_view: 2,
        proposal_hash: Hash::prehashed([0x21; Hash::LENGTH]),
        descriptor_hash: Hash::prehashed([0x22; Hash::LENGTH]),
        subject_hash: Hash::prehashed([0x23; Hash::LENGTH]),
        payload_ownership_hash: Hash::prehashed([0x24; Hash::LENGTH]),
        rbc_instance_hash: Hash::prehashed([0x25; Hash::LENGTH]),
        accepted_candidate_indices: vec![3, 1],
        accepted_transaction_hashes: vec![
            Hash::prehashed([0x26; Hash::LENGTH]),
            Hash::prehashed([0x27; Hash::LENGTH]),
        ],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&sample_roster()),
        validator_count: 3,
        min_quorum: 3,
        qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
    }
}
fn sample_lane_block_proposal() -> LaneBlockProposalV1 {
    let roster = sample_roster();
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(11),
        lane_incarnation: Hash::new(b"lane-consensus-model-fixture"),
        proposal_height: 12,
        previous_lane_block_height: 12,
        previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x20; Hash::LENGTH])),
        lane_block_height: 13,
        lane_block_view: 2,
        subject_hash: Hash::prehashed([0x23; Hash::LENGTH]),
        payload_ownership_hash: Hash::prehashed([0x24; Hash::LENGTH]),
        rbc_instance_hash: Hash::prehashed([0x25; Hash::LENGTH]),
        accepted_candidate_indices: vec![3, 1],
        accepted_transaction_hashes: vec![
            Hash::prehashed([0x26; Hash::LENGTH]),
            Hash::prehashed([0x27; Hash::LENGTH]),
        ],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&roster),
        validator_set: roster,
        validator_count: 3,
        min_quorum: 3,
        qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
        descriptor_hash: Hash::prehashed([0x00; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    proposal
}
fn refresh_lane_block_descriptor_hash(proposal: &mut LaneBlockProposalV1) {
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
}
#[test]
fn lane_block_vote_body_signature_preimage_binds_phase_and_descriptor() {
    let body = sample_lane_block_vote_body(CertPhase::Prepare);
    let preimage = body.signature_preimage();
    {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            body.signature_preimage(),
            preimage,
            "lane-vote signature identity must ignore the caller's ambient Norito layout"
        );
    }
    assert!(preimage.starts_with(b"iroha:lane-block-vote:v1"));
    assert!(preimage.len() > b"iroha:lane-block-vote:v1".len());
    let mut commit_body = body.clone();
    commit_body.phase = CertPhase::Commit;
    assert_ne!(
        preimage,
        commit_body.signature_preimage(),
        "prepare and commit lane votes must be domain-separated"
    );
    let mut descriptor_drift = body;
    descriptor_drift.descriptor_hash = Hash::prehashed([0x29; Hash::LENGTH]);
    assert_ne!(
        preimage,
        descriptor_drift.signature_preimage(),
        "descriptor drift must change the lane vote preimage"
    );
}
#[test]
fn lane_block_vote_body_signature_preimage_binds_replay_and_quorum_fields() {
    let body = sample_lane_block_vote_body(CertPhase::Prepare);
    let preimage = body.signature_preimage();
    let mut cases = Vec::<(&str, LaneBlockVoteBodyV1)>::new();
    let mut lane_drift = body.clone();
    lane_drift.lane_id = LaneId::new(8);
    cases.push(("lane id", lane_drift));
    let mut dataspace_drift = body.clone();
    dataspace_drift.dataspace_id = DataSpaceId::new(12);
    cases.push(("dataspace id", dataspace_drift));
    let mut proposal_height_drift = body.clone();
    proposal_height_drift.proposal_height = proposal_height_drift.proposal_height.saturating_add(1);
    cases.push(("proposal height", proposal_height_drift));
    let mut height_drift = body.clone();
    height_drift.lane_block_height = height_drift.lane_block_height.saturating_add(1);
    cases.push(("lane block height", height_drift));
    let mut view_drift = body.clone();
    view_drift.lane_block_view = view_drift.lane_block_view.saturating_add(1);
    cases.push(("lane block view", view_drift));
    let mut proposal_drift = body.clone();
    proposal_drift.proposal_hash = Hash::prehashed([0x31; Hash::LENGTH]);
    cases.push(("proposal hash", proposal_drift));
    let mut subject_drift = body.clone();
    subject_drift.subject_hash = Hash::prehashed([0x32; Hash::LENGTH]);
    cases.push(("subject hash", subject_drift));
    let mut ownership_drift = body.clone();
    ownership_drift.payload_ownership_hash = Hash::prehashed([0x33; Hash::LENGTH]);
    cases.push(("payload ownership hash", ownership_drift));
    let mut rbc_drift = body.clone();
    rbc_drift.rbc_instance_hash = Hash::prehashed([0x34; Hash::LENGTH]);
    cases.push(("rbc instance hash", rbc_drift));
    let mut candidate_indices_drift = body.clone();
    candidate_indices_drift.accepted_candidate_indices.reverse();
    cases.push(("accepted candidate indices", candidate_indices_drift));
    let mut transaction_hashes_drift = body.clone();
    transaction_hashes_drift
        .accepted_transaction_hashes
        .reverse();
    cases.push(("accepted transaction hashes", transaction_hashes_drift));
    let mut validator_hash_version_drift = body.clone();
    validator_hash_version_drift.validator_set_hash_version = validator_hash_version_drift
        .validator_set_hash_version
        .saturating_add(1);
    cases.push(("validator set hash version", validator_hash_version_drift));
    let mut validator_hash_drift = body.clone();
    validator_hash_drift.validator_set_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0x35; Hash::LENGTH]));
    cases.push(("validator set hash", validator_hash_drift));
    let mut validator_count_drift = body.clone();
    validator_count_drift.validator_count = validator_count_drift.validator_count.saturating_add(1);
    cases.push(("validator count", validator_count_drift));
    let mut quorum_drift = body.clone();
    quorum_drift.min_quorum = quorum_drift.min_quorum.saturating_sub(1);
    cases.push(("minimum quorum", quorum_drift));
    let mut qc_mode_drift = body.clone();
    qc_mode_drift.qc_mode_tag.push_str(":drift");
    cases.push(("qc mode tag", qc_mode_drift));
    for (label, drifted) in cases {
        assert_ne!(
            preimage,
            drifted.signature_preimage(),
            "{label} drift must change the lane vote preimage"
        );
    }
}
#[test]
fn lane_block_proposal_hashes_bind_predecessor_and_committee() {
    let proposal = sample_lane_block_proposal();
    assert_eq!(
        proposal.descriptor.computed_descriptor_hash(),
        proposal.descriptor.descriptor_hash
    );
    assert_eq!(proposal.computed_proposal_hash(), proposal.proposal_hash);
    let mut predecessor_drift = proposal.clone();
    predecessor_drift
        .descriptor
        .previous_lane_block_descriptor_hash = Some(Hash::prehashed([0x31; Hash::LENGTH]));
    assert_ne!(
        predecessor_drift.descriptor.computed_descriptor_hash(),
        proposal.descriptor.descriptor_hash,
        "predecessor descriptor drift must change descriptor identity"
    );
    let mut committee_drift = proposal.clone();
    committee_drift.descriptor.validator_set.reverse();
    assert_ne!(
        committee_drift.descriptor.computed_descriptor_hash(),
        proposal.descriptor.descriptor_hash,
        "committee order drift must change descriptor identity"
    );
}
#[test]
fn lane_block_and_replay_hashes_ignore_ambient_norito_layout() {
    let proposal = sample_lane_block_proposal();
    let descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    let proposal_hash = proposal.computed_proposal_hash();
    let ownership = sample_lane_payload_ownership_with_replay_material();
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("compute canonical replay hashes");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    assert_eq!(
        proposal.descriptor.computed_descriptor_hash(),
        descriptor_hash
    );
    assert_eq!(proposal.computed_proposal_hash(), proposal_hash);
    assert_eq!(
        ownership
            .compute_replay_hashes()
            .expect("compute replay hashes under alternate ambient layout"),
        replay_hashes
    );
}
#[test]
fn lane_block_descriptor_hash_binds_replay_and_quorum_fields() {
    let descriptor = sample_lane_block_proposal().descriptor;
    let mut cases = Vec::<(&str, LaneBlockDescriptorV1)>::new();
    let mut lane_drift = descriptor.clone();
    lane_drift.lane_id = LaneId::new(8);
    cases.push(("lane id", lane_drift));
    let mut dataspace_drift = descriptor.clone();
    dataspace_drift.dataspace_id = DataSpaceId::new(12);
    cases.push(("dataspace id", dataspace_drift));
    let mut proposal_height_drift = descriptor.clone();
    proposal_height_drift.proposal_height = proposal_height_drift.proposal_height.saturating_add(1);
    cases.push(("proposal height", proposal_height_drift));
    let mut previous_height_drift = descriptor.clone();
    previous_height_drift.previous_lane_block_height = previous_height_drift
        .previous_lane_block_height
        .saturating_sub(1);
    cases.push(("previous lane block height", previous_height_drift));
    let mut predecessor_drift = descriptor.clone();
    predecessor_drift.previous_lane_block_descriptor_hash = None;
    cases.push(("previous descriptor hash", predecessor_drift));
    let mut height_drift = descriptor.clone();
    height_drift.lane_block_height = height_drift.lane_block_height.saturating_add(1);
    cases.push(("lane block height", height_drift));
    let mut view_drift = descriptor.clone();
    view_drift.lane_block_view = view_drift.lane_block_view.saturating_add(1);
    cases.push(("lane block view", view_drift));
    let mut subject_drift = descriptor.clone();
    subject_drift.subject_hash = Hash::prehashed([0x31; Hash::LENGTH]);
    cases.push(("subject hash", subject_drift));
    let mut ownership_drift = descriptor.clone();
    ownership_drift.payload_ownership_hash = Hash::prehashed([0x32; Hash::LENGTH]);
    cases.push(("payload ownership hash", ownership_drift));
    let mut rbc_drift = descriptor.clone();
    rbc_drift.rbc_instance_hash = Hash::prehashed([0x33; Hash::LENGTH]);
    cases.push(("rbc instance hash", rbc_drift));
    let mut candidate_indices_drift = descriptor.clone();
    candidate_indices_drift.accepted_candidate_indices.reverse();
    cases.push(("accepted candidate indices", candidate_indices_drift));
    let mut transaction_hashes_drift = descriptor.clone();
    transaction_hashes_drift
        .accepted_transaction_hashes
        .reverse();
    cases.push(("accepted transaction hashes", transaction_hashes_drift));
    let mut validator_hash_version_drift = descriptor.clone();
    validator_hash_version_drift.validator_set_hash_version = validator_hash_version_drift
        .validator_set_hash_version
        .saturating_add(1);
    cases.push(("validator set hash version", validator_hash_version_drift));
    let mut validator_hash_drift = descriptor.clone();
    validator_hash_drift.validator_set_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0x34; Hash::LENGTH]));
    cases.push(("validator set hash", validator_hash_drift));
    let mut validator_set_drift = descriptor.clone();
    validator_set_drift.validator_set.reverse();
    cases.push(("validator set order", validator_set_drift));
    let mut validator_count_drift = descriptor.clone();
    validator_count_drift.validator_count = validator_count_drift.validator_count.saturating_add(1);
    cases.push(("validator count", validator_count_drift));
    let mut quorum_drift = descriptor.clone();
    quorum_drift.min_quorum = quorum_drift.min_quorum.saturating_sub(1);
    cases.push(("minimum quorum", quorum_drift));
    let mut qc_mode_drift = descriptor.clone();
    qc_mode_drift.qc_mode_tag.push_str(":drift");
    cases.push(("qc mode tag", qc_mode_drift));
    for (label, drifted) in cases {
        assert_ne!(
            drifted.computed_descriptor_hash(),
            descriptor.descriptor_hash,
            "{label} drift must change descriptor identity"
        );
    }
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the complete proposal mutation matrix documents every canonical descriptor, replay, quorum, and proposal-preimage binding in one protocol vector"
)]
fn lane_block_proposal_hash_binds_descriptor_replay_and_quorum_fields() {
    let proposal = sample_lane_block_proposal();
    let mut cases = Vec::<(&str, LaneBlockProposalV1)>::new();
    let mut descriptor_hash_drift = proposal.clone();
    descriptor_hash_drift.descriptor.descriptor_hash = Hash::prehashed([0x31; Hash::LENGTH]);
    cases.push(("descriptor hash", descriptor_hash_drift));
    let mut lane_drift = proposal.clone();
    lane_drift.descriptor.lane_id = LaneId::new(8);
    refresh_lane_block_descriptor_hash(&mut lane_drift);
    cases.push(("lane id", lane_drift));
    let mut dataspace_drift = proposal.clone();
    dataspace_drift.descriptor.dataspace_id = DataSpaceId::new(12);
    refresh_lane_block_descriptor_hash(&mut dataspace_drift);
    cases.push(("dataspace id", dataspace_drift));
    let mut proposal_height_drift = proposal.clone();
    proposal_height_drift.descriptor.proposal_height = proposal_height_drift
        .descriptor
        .proposal_height
        .saturating_add(1);
    refresh_lane_block_descriptor_hash(&mut proposal_height_drift);
    cases.push(("proposal height", proposal_height_drift));
    let mut previous_height_drift = proposal.clone();
    previous_height_drift.descriptor.previous_lane_block_height = previous_height_drift
        .descriptor
        .previous_lane_block_height
        .saturating_sub(1);
    refresh_lane_block_descriptor_hash(&mut previous_height_drift);
    cases.push(("previous lane block height", previous_height_drift));
    let mut predecessor_drift = proposal.clone();
    predecessor_drift
        .descriptor
        .previous_lane_block_descriptor_hash = None;
    refresh_lane_block_descriptor_hash(&mut predecessor_drift);
    cases.push(("previous descriptor hash", predecessor_drift));
    let mut height_drift = proposal.clone();
    height_drift.descriptor.lane_block_height =
        height_drift.descriptor.lane_block_height.saturating_add(1);
    refresh_lane_block_descriptor_hash(&mut height_drift);
    cases.push(("lane block height", height_drift));
    let mut view_drift = proposal.clone();
    view_drift.descriptor.lane_block_view = view_drift.descriptor.lane_block_view.saturating_add(1);
    refresh_lane_block_descriptor_hash(&mut view_drift);
    cases.push(("lane block view", view_drift));
    let mut subject_drift = proposal.clone();
    subject_drift.descriptor.subject_hash = Hash::prehashed([0x32; Hash::LENGTH]);
    refresh_lane_block_descriptor_hash(&mut subject_drift);
    cases.push(("subject hash", subject_drift));
    let mut ownership_drift = proposal.clone();
    ownership_drift.descriptor.payload_ownership_hash = Hash::prehashed([0x33; Hash::LENGTH]);
    refresh_lane_block_descriptor_hash(&mut ownership_drift);
    cases.push(("payload ownership hash", ownership_drift));
    let mut rbc_drift = proposal.clone();
    rbc_drift.descriptor.rbc_instance_hash = Hash::prehashed([0x34; Hash::LENGTH]);
    refresh_lane_block_descriptor_hash(&mut rbc_drift);
    cases.push(("rbc instance hash", rbc_drift));
    let mut candidate_indices_drift = proposal.clone();
    candidate_indices_drift
        .descriptor
        .accepted_candidate_indices
        .reverse();
    refresh_lane_block_descriptor_hash(&mut candidate_indices_drift);
    cases.push(("accepted candidate indices", candidate_indices_drift));
    let mut transaction_hashes_drift = proposal.clone();
    transaction_hashes_drift
        .descriptor
        .accepted_transaction_hashes
        .reverse();
    refresh_lane_block_descriptor_hash(&mut transaction_hashes_drift);
    cases.push(("accepted transaction hashes", transaction_hashes_drift));
    let mut validator_hash_version_drift = proposal.clone();
    validator_hash_version_drift
        .descriptor
        .validator_set_hash_version = validator_hash_version_drift
        .descriptor
        .validator_set_hash_version
        .saturating_add(1);
    refresh_lane_block_descriptor_hash(&mut validator_hash_version_drift);
    cases.push(("validator set hash version", validator_hash_version_drift));
    let mut validator_hash_drift = proposal.clone();
    validator_hash_drift.descriptor.validator_set_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0x35; Hash::LENGTH]));
    refresh_lane_block_descriptor_hash(&mut validator_hash_drift);
    cases.push(("validator set hash", validator_hash_drift));
    let mut validator_set_drift = proposal.clone();
    validator_set_drift.descriptor.validator_set.reverse();
    refresh_lane_block_descriptor_hash(&mut validator_set_drift);
    cases.push(("validator set order", validator_set_drift));
    let mut validator_count_drift = proposal.clone();
    validator_count_drift.descriptor.validator_count = validator_count_drift
        .descriptor
        .validator_count
        .saturating_add(1);
    refresh_lane_block_descriptor_hash(&mut validator_count_drift);
    cases.push(("validator count", validator_count_drift));
    let mut quorum_drift = proposal.clone();
    quorum_drift.descriptor.min_quorum = quorum_drift.descriptor.min_quorum.saturating_sub(1);
    refresh_lane_block_descriptor_hash(&mut quorum_drift);
    cases.push(("minimum quorum", quorum_drift));
    let mut qc_mode_drift = proposal.clone();
    qc_mode_drift.descriptor.qc_mode_tag.push_str(":drift");
    refresh_lane_block_descriptor_hash(&mut qc_mode_drift);
    cases.push(("qc mode tag", qc_mode_drift));
    for (label, drifted) in cases {
        assert_ne!(
            drifted.computed_proposal_hash(),
            proposal.proposal_hash,
            "{label} drift must change proposal identity"
        );
    }
}
#[test]
fn lane_block_proposal_roundtrips_and_derives_vote_body() {
    let proposal = sample_lane_block_proposal();
    let encoded = norito::to_bytes(&proposal).expect("lane proposal encodes");
    let decoded: LaneBlockProposalV1 =
        norito::decode_from_bytes(&encoded).expect("lane proposal decodes");
    assert_eq!(decoded, proposal);
    let body = decoded.vote_body(CertPhase::Prepare);
    assert_eq!(body.proposal_hash, decoded.proposal_hash);
    assert_eq!(body.descriptor_hash, decoded.descriptor.descriptor_hash);
    assert_eq!(body.proposal_height, decoded.descriptor.proposal_height);
    assert_eq!(
        body.validator_set_hash,
        decoded.descriptor.computed_validator_set_hash()
    );
    assert_eq!(
        body.accepted_transaction_hashes,
        decoded.descriptor.accepted_transaction_hashes
    );
}

#[test]
fn lane_consensus_artifacts_reject_pre_release_omitted_nullable_slots() {
    #[derive(Encode)]
    struct PreReleaseLaneBlockProposal {
        descriptor: LaneBlockDescriptorV1,
        proposal_hash: Hash,
    }
    #[derive(Encode)]
    struct PreReleaseLaneBlockQc {
        body: LaneBlockVoteBodyV1,
        validator_set_hash_version: u16,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Vec<PeerId>,
        signers_bitmap: Vec<u8>,
        bls_aggregate_signature: Vec<u8>,
    }

    let proposal = sample_lane_block_proposal();
    let bytes = PreReleaseLaneBlockProposal {
        descriptor: proposal.descriptor.clone(),
        proposal_hash: proposal.proposal_hash,
    }
    .encode();
    assert!(
        LaneBlockProposalV1::decode_all(&mut bytes.as_slice()).is_err(),
        "the first-release lane proposal must reject the layout without its carrier-hint slot"
    );

    let bytes = PreReleaseLaneBlockQc {
        body: proposal.vote_body(CertPhase::Prepare),
        validator_set_hash_version: proposal.descriptor.validator_set_hash_version,
        validator_set_hash: proposal.descriptor.validator_set_hash,
        validator_set: proposal.descriptor.validator_set.clone(),
        signers_bitmap: vec![0b0000_0111],
        bls_aggregate_signature: vec![0xA5; 96],
    }
    .encode();
    assert!(
        LaneBlockQcV1::decode_all(&mut bytes.as_slice()).is_err(),
        "the first-release lane QC must reject the layout without its availability-QC slot"
    );
}

#[test]
fn lane_consensus_json_requires_nullable_slots_and_rejects_unknown_fields() {
    let proposal = sample_lane_block_proposal();
    let mut descriptor =
        norito::json::to_value(&proposal.descriptor).expect("serialize lane descriptor");
    descriptor
        .as_object_mut()
        .expect("lane descriptor JSON object")
        .remove("previous_lane_block_descriptor_hash");
    assert!(
        norito::json::from_value::<LaneBlockDescriptorV1>(descriptor).is_err(),
        "the first-release lane descriptor must require its nullable predecessor slot"
    );

    let mut proposal_json = norito::json::to_value(&proposal).expect("serialize lane proposal");
    proposal_json
        .as_object_mut()
        .expect("lane proposal JSON object")
        .remove("payload_block_hint");
    assert!(
        norito::json::from_value::<LaneBlockProposalV1>(proposal_json).is_err(),
        "the first-release lane proposal must require its nullable carrier-hint slot"
    );

    let qc = LaneBlockQcV1 {
        body: proposal.vote_body(CertPhase::Prepare),
        validator_set_hash_version: proposal.descriptor.validator_set_hash_version,
        validator_set_hash: proposal.descriptor.validator_set_hash,
        validator_set: proposal.descriptor.validator_set.clone(),
        signers_bitmap: vec![0b0000_0111],
        bls_aggregate_signature: vec![0xA5; 96],
        payload_availability_qc: None,
    };
    let mut qc_json = norito::json::to_value(&qc).expect("serialize lane QC");
    qc_json
        .as_object_mut()
        .expect("lane QC JSON object")
        .remove("payload_availability_qc");
    assert!(
        norito::json::from_value::<LaneBlockQcV1>(qc_json).is_err(),
        "the first-release lane QC must require its nullable availability slot"
    );

    let ownership = sample_lane_payload_ownership_with_replay_material();
    for field in [
        "previous_lane_block_descriptor_hash",
        "lane_block_descriptor_hash",
    ] {
        let mut value =
            norito::json::to_value(&ownership).expect("serialize lane payload ownership");
        value
            .as_object_mut()
            .expect("lane payload ownership JSON object")
            .remove(field);
        assert!(
            norito::json::from_value::<SumeragiLanePayloadOwnership>(value).is_err(),
            "the first-release lane payload ownership must require {field}"
        );
    }

    let mut vote_body = norito::json::to_value(&qc.body).expect("serialize lane vote body");
    vote_body
        .as_object_mut()
        .expect("lane vote body JSON object")
        .insert("pre_release_field".to_owned(), norito::json::Value::Null);
    assert!(
        norito::json::from_value::<LaneBlockVoteBodyV1>(vote_body).is_err(),
        "the first-release lane vote body must reject unknown fields"
    );
}

#[test]
fn lane_block_certificate_decodes_exactly_and_rejects_trailing_bytes() {
    let proposal = sample_lane_block_proposal();
    let qc = |phase| LaneBlockQcV1 {
        body: proposal.vote_body(phase),
        validator_set_hash_version: proposal.descriptor.validator_set_hash_version,
        validator_set_hash: proposal.descriptor.validator_set_hash,
        validator_set: proposal.descriptor.validator_set.clone(),
        signers_bitmap: vec![0b0000_0111],
        bls_aggregate_signature: vec![0xA5; 96],
        payload_availability_qc: None,
    };
    let prepare_qc = qc(CertPhase::Prepare);
    let commit_qc = qc(CertPhase::Commit);
    let certificate = LaneBlockCertificateV1 {
        proposal,
        prepare_qc,
        commit_qc,
    };
    let encoded = certificate.encode();
    let (decoded, used) = norito::core::decode_field_canonical::<LaneBlockCertificateV1>(&encoded)
        .expect("canonical lane certificate decodes exactly");
    assert_eq!(decoded, certificate);
    assert_eq!(used, encoded.len());
    let mut tailed = encoded;
    tailed.extend_from_slice(b"next-frame");
    norito::core::decode_field_canonical::<LaneBlockCertificateV1>(&tailed)
        .expect_err("unframed trailing bytes must be rejected");
}
#[test]
fn exec_witness_roundtrip_codec() {
    let w = ExecWitness {
        reads: vec![ExecKv {
            key: b"key:read".to_vec(),
            value: b"value-pre".to_vec(),
        }],
        writes: vec![ExecKv {
            key: b"key:write".to_vec(),
            value: b"value-post".to_vec(),
        }],
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    };
    let bytes = w.encode();
    let dec = ExecWitness::decode(&mut &bytes[..]).expect("decode witness");
    assert_eq!(w, dec);
}
#[test]
fn retired_invalid_qc_evidence_fails_decode() {
    let roster = sample_roster();
    let ev = RetiredEvidence {
        kind: RetiredEvidenceKind::InvalidQc,
        payload: RetiredEvidencePayload::InvalidQc {
            certificate: RetiredQcFixture {
                phase: CertPhase::Commit,
                subject_block_hash: dummy_hash(),
                parent_state_root: Hash::new(b"parent_root"),
                post_state_root: Hash::new(b"post_root"),
                height: 12,
                view: 3,
                epoch: 0,
                chain_order_hash: retired_chain_order_hash(),
                rechain_seq: 0,
                mode_tag: PERMISSIONED_TAG.to_string(),
                highest_qc: None,
                validator_set_hash: HashOf::new(&roster),
                validator_set_hash_version: 1,
                validator_set: roster,
                aggregate: RetiredQcAggregateFixture {
                    signers_bitmap: vec![0xFF],
                    bls_aggregate_signature: vec![4, 5, 6],
                },
            },
            reason: "test".to_string(),
        },
    };
    let bytes = ev.encode();
    Evidence::decode(&mut &bytes[..]).expect_err("retired invalid-QC evidence must fail decode");
}
#[test]
fn retired_censorship_evidence_fails_decode() {
    let key_pair = checked_random_keypair();
    let payload = crate::transaction::TransactionSubmissionReceiptPayload {
        entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
        signed_transaction_hash: None,
        submitted_at_ms: 10,
        submitted_at_height: 2,
        signer: key_pair.public_key().clone(),
    };
    let receipt = crate::transaction::TransactionSubmissionReceipt::try_sign(payload, &key_pair)
        .expect("checked censorship evidence receipt fixture signature");
    let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32]));
    let ev = RetiredEvidence {
        kind: RetiredEvidenceKind::Censorship,
        payload: RetiredEvidencePayload::Censorship {
            tx_hash,
            receipts: vec![receipt],
        },
    };
    let bytes = ev.encode();
    Evidence::decode(&mut &bytes[..]).expect_err("retired censorship evidence must fail decode");
}
#[test]
fn retired_evidence_record_fails_decode() {
    let ev = RetiredEvidence {
        kind: RetiredEvidenceKind::DoublePrepare,
        payload: RetiredEvidencePayload::DoubleVote {
            v1: RetiredQcVoteFixture {
                phase: CertPhase::Prepare,
                block_hash: dummy_hash(),
                parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                height: 10,
                view: 1,
                epoch: 0,
                chain_order_hash: retired_chain_order_hash(),
                rechain_seq: 0,
                highest_qc: None,
                signer: 2,
                bls_sig: vec![],
            },
            v2: RetiredQcVoteFixture {
                phase: CertPhase::Prepare,
                block_hash: dummy_hash(),
                parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                height: 10,
                view: 1,
                epoch: 0,
                chain_order_hash: retired_chain_order_hash(),
                rechain_seq: 0,
                highest_qc: None,
                signer: 2,
                bls_sig: vec![],
            },
        },
    };
    let rec = RetiredEvidenceRecord {
        evidence: ev,
        recorded_at_height: 11,
        recorded_at_view: 2,
        recorded_at_ms: 1_689_000,
        penalty_applied: false,
        penalty_cancelled: false,
        penalty_cancelled_at_height: None,
        penalty_applied_at_height: None,
        consensus_admitted_at_height: Some(11),
    };
    let bytes = rec.encode();
    EvidenceRecord::decode(&mut &bytes[..])
        .expect_err("records carrying retired evidence must fail decode");
}
#[test]
fn lane_settlement_receipt_decode_from_slice_requires_canonical_bare_prefix() {
    let receipt = LaneSettlementReceipt {
        source_id: [0xA5; 32],
        local_amount: "1.25".parse().expect("quantity"),
        xor_due: "2.5".parse().expect("quantity"),
        xor_after_haircut: "2.0".parse().expect("quantity"),
        xor_variance: "0.5".parse().expect("quantity"),
        timestamp_ms: 1_689_000,
    };
    let canonical = receipt.encode();
    let mut followed_by_next_field = canonical.clone();
    followed_by_next_field.extend_from_slice(b"next-field");
    let (decoded, used) = LaneSettlementReceipt::decode_from_slice(&followed_by_next_field)
        .expect("decode canonical lane settlement receipt prefix");
    assert_eq!(decoded, receipt);
    assert_eq!(used, canonical.len());
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::codec::encode_with_header_flags(&receipt).0
    };
    assert_ne!(
        alternate, canonical,
        "alternate layout must differ for the canonicality probe"
    );
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    let (decoded, used) = LaneSettlementReceipt::decode_from_slice(&followed_by_next_field)
        .expect("canonical receipt prefix must ignore the ambient layout");
    assert_eq!(decoded, receipt);
    assert_eq!(used, canonical.len());
    LaneSettlementReceipt::decode_from_slice(&alternate)
        .expect_err("alternate bare layout must be rejected");
}
fn checked_seeded_peer_id(seed: u8) -> PeerId {
    PeerId::new(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed must produce a keypair")
            .public_key()
            .clone(),
    )
}
fn sample_lane_payload_ownership_with_replay_material() -> SumeragiLanePayloadOwnership {
    let mut validator_set = vec![checked_seeded_peer_id(1), checked_seeded_peer_id(2)];
    validator_set.sort();
    let validator_count = u32::try_from(validator_set.len()).expect("validator count fits u32");
    let mut ownership = SumeragiLanePayloadOwnership {
        proposal_height: 12,
        proposal_view: 3,
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(42),
        lane_incarnation: Hash::new(b"lane-ownership-model-fixture"),
        lane_block_height: 2,
        lane_block_view: 1,
        subject_hash: Hash::new(b"lane subject placeholder"),
        qc_mode_tag: "test-lane-qc-mode".to_string(),
        accepted_candidate_indices: vec![0, 2],
        accepted_transaction_hashes: vec![
            Hash::new(b"lane accepted tx 0"),
            Hash::new(b"lane accepted tx 2"),
        ],
        previous_lane_block_height: 1,
        previous_lane_block_descriptor_hash: Some(Hash::new(b"lane predecessor descriptor")),
        lane_block_descriptor_hash: Some(Hash::new(b"lane block descriptor placeholder")),
        lane_block_descriptor_validator_set: validator_set,
        lane_block_descriptor_validator_count: validator_count,
        lane_block_descriptor_min_quorum: validator_count,
        payload_ownership_hash: Hash::new(b"lane payload ownership placeholder"),
        rbc_instance_hash: Hash::new(b"lane rbc instance placeholder"),
    };
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("replay hashes compute for canonical lane ownership");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    ownership
}
#[test]
fn lane_payload_ownership_replay_material_validates_canonical_hashes() {
    let ownership = sample_lane_payload_ownership_with_replay_material();
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("canonical replay material should hash");
    assert_eq!(ownership.subject_hash, replay_hashes.subject_hash);
    assert_eq!(
        ownership.payload_ownership_hash,
        replay_hashes.payload_ownership_hash
    );
    assert_eq!(ownership.rbc_instance_hash, replay_hashes.rbc_instance_hash);
    assert_eq!(
        ownership.lane_block_descriptor_hash,
        Some(replay_hashes.lane_block_descriptor_hash)
    );
    ownership
        .validate_replay_material()
        .expect("canonical replay material should validate");
}
#[test]
fn lane_payload_ownership_replay_material_rejects_accepted_hash_drift() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.accepted_transaction_hashes[0] = Hash::new(b"forged accepted tx 0");
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::SubjectHashMismatch)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_proposal_height_drift() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.proposal_height = ownership.proposal_height.saturating_add(1);
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::DescriptorHashMismatch)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_defaulted_candidate_hashes() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.accepted_transaction_hashes.clear();
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::CandidateHashCountMismatch)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_defaulted_predecessor_height() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.previous_lane_block_height = 0;
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::PreviousLaneBlockHeightMismatch)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_missing_non_genesis_predecessor_hash() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    assert!(ownership.previous_lane_block_height > 0);
    ownership.previous_lane_block_descriptor_hash = None;
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_missing_descriptor_hash() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.lane_block_descriptor_hash = None;
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_empty_validator_set() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.lane_block_descriptor_validator_set.clear();
    ownership.lane_block_descriptor_validator_count = 0;
    ownership.lane_block_descriptor_min_quorum = 0;
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::EmptyValidatorSet)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_validator_count_drift() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.lane_block_descriptor_validator_count = ownership
        .lane_block_descriptor_validator_count
        .saturating_add(1);
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::ValidatorCountMismatch)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_noncanonical_validator_set() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.lane_block_descriptor_validator_set.reverse();
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::ValidatorSetNotCanonical)
    );
}
#[test]
fn lane_payload_ownership_replay_material_rejects_genesis_predecessor_descriptor() {
    let mut ownership = sample_lane_payload_ownership_with_replay_material();
    ownership.lane_block_height = 1;
    ownership.previous_lane_block_height = 0;
    ownership.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"unexpected genesis predecessor descriptor"));
    assert_eq!(
        ownership.validate_replay_material(),
        Err(SumeragiLanePayloadOwnershipReplayError::UnexpectedGenesisPredecessorDescriptorHash)
    );
}
#[test]
fn lane_payload_ownership_status_roundtrip_codec() {
    let ownership = SumeragiLanePayloadOwnership {
        proposal_height: 12,
        proposal_view: 3,
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(42),
        lane_incarnation: Hash::new(b"lane-ownership-model-fixture"),
        lane_block_height: 2,
        lane_block_view: 1,
        subject_hash: Hash::new(b"lane subject"),
        qc_mode_tag: "test-lane-qc-mode".to_string(),
        accepted_candidate_indices: vec![0, 2],
        accepted_transaction_hashes: vec![
            Hash::new(b"lane accepted tx 0"),
            Hash::new(b"lane accepted tx 2"),
        ],
        previous_lane_block_height: 1,
        previous_lane_block_descriptor_hash: Some(Hash::new(b"lane predecessor descriptor")),
        lane_block_descriptor_hash: Some(Hash::new(b"lane block descriptor")),
        lane_block_descriptor_validator_set: Vec::new(),
        lane_block_descriptor_validator_count: 0,
        lane_block_descriptor_min_quorum: 0,
        payload_ownership_hash: Hash::new(b"lane payload ownership"),
        rbc_instance_hash: Hash::new(b"lane rbc instance"),
    };
    let encoded = ownership.encode();
    let decoded = SumeragiLanePayloadOwnership::decode(&mut &encoded[..])
        .expect("lane payload ownership decodes");
    assert_eq!(decoded, ownership);
    let (decoded_from_slice, used) = SumeragiLanePayloadOwnership::decode_from_slice(&encoded)
        .expect("lane payload ownership decodes from slice");
    assert_eq!(decoded_from_slice, ownership);
    assert_eq!(used, encoded.len());
}
include!("consensus/runtime_diagnostics_tests.rs");
include!("consensus/npos_diagnostics_tests.rs");

#[path = "native_amx_settlement_tests.rs"]
mod native_amx;
