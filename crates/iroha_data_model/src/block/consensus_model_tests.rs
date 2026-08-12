use std::num::NonZeroU64;

use iroha_crypto::{
    Algorithm, KeyPair, MerkleProof, MerkleTree, MerkleTreeCommitment, SignatureOf,
};
use iroha_primitives::numeric::{Numeric, Quantity};
use norito::core::DecodeFromSlice;

use crate::consensus::VALIDATOR_SET_HASH_VERSION_V1;

use super::*;

fn dummy_hash() -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::prehashed([0u8; 32]))
}

fn checked_random_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked consensus fixture keypair")
}

fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
    KeyPair::try_random_with_algorithm(algorithm)
        .expect("generate checked consensus fixture keypair")
}

#[cfg(feature = "json")]
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

fn roster_hash(roster: &[PeerId]) -> Hash {
    Hash::new(roster.to_vec().encode())
}

include!("consensus/wire_schema_tests.rs");

fn sample_qc_ref() -> QcRef {
    QcRef {
        height: 4,
        view: 1,
        epoch: 1,
        subject_block_hash: dummy_hash(),
        phase: CertPhase::Prepare,
    }
}

fn sample_consensus_header() -> ConsensusBlockHeader {
    ConsensusBlockHeader {
        parent_hash: dummy_hash(),
        tx_root: Hash::new(b"tx_root"),
        state_root: Hash::new(b"state_root"),
        proposer: 1,
        height: 6,
        view: 3,
        epoch: 1,
        highest_qc: sample_qc_ref(),
    }
}

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
        (
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
            true,
        ),
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
    assert!(!committed_lane_block_status_counts_as_progress(
        COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
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
    vrf_commit_window_blocks: u64,
    vrf_reveal_window_blocks: u64,
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
fn negative_numeric_payloads_cannot_decode_as_npos_bonds() {
    let forged = ForgedNposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(10).expect("nonzero epoch"),
        epoch_seed: [1; 32],
        vrf_commit_window_blocks: 2,
        vrf_reveal_window_blocks: 2,
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
fn npos_genesis_reveal_window_must_close_before_boundary() {
    let params = NposGenesisParams {
        epoch_length_blocks: NonZeroU64::new(4).expect("non-zero epoch"),
        epoch_seed: [1; 32],
        vrf_commit_window_blocks: 2,
        vrf_reveal_window_blocks: 2,
        max_validators: 4,
        min_self_bond: Quantity::one(),
        min_nomination_bond: Quantity::one(),
        max_nominator_concentration_pct: 100,
        seat_band_pct: 10,
        max_entity_correlation_pct: 100,
        finality_margin_blocks: 1,
        evidence_horizon_blocks: 10,
        activation_lag_blocks: 1,
        slashing_delay_blocks: 1,
    };
    assert_eq!(
        params.validate(),
        Err("VRF reveal window must close before the epoch boundary")
    );

    let mut valid = params;
    valid.epoch_length_blocks = NonZeroU64::new(5).expect("non-zero epoch");
    valid
        .validate()
        .expect("one finalized pre-boundary block is sufficient");
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

fn sample_native_amx_invariant_qc() -> NativeAmxAttestationQcV2 {
    sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        [0x81; 32],
        Hash::new(b"native-amx-validator-material-invariant"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
}

fn native_amx_qc_wire(qc: &NativeAmxAttestationQcV2) -> NativeAmxAttestationQcV2Wire {
    NativeAmxAttestationQcV2Wire {
        body: qc.body,
        validator_set_hash_version: qc.validator_set_hash_version,
        validator_set_hash: qc.validator_set_hash,
        validator_set: qc.validator_set().to_vec(),
        validator_set_pops: qc.validator_set_pops().to_vec(),
        signers_bitmap: qc.signers_bitmap.clone(),
        bls_aggregate_signature: qc.bls_aggregate_signature.clone(),
    }
}

#[test]
fn native_amx_qc_constructor_rejects_misaligned_validator_material() {
    let qc = sample_native_amx_invariant_qc();
    let validator_count = qc.validator_set().len();
    let error = NativeAmxAttestationQcV2::try_new(
        qc.body,
        qc.validator_set_hash_version,
        qc.validator_set_hash,
        qc.validator_set().to_vec(),
        Vec::new(),
        qc.signers_bitmap.clone(),
        qc.bls_aggregate_signature.clone(),
    )
    .expect_err("a validator set without one proof per validator must be rejected");

    assert_eq!(error.validator_count(), validator_count);
    assert_eq!(error.proof_count(), 0);
}

#[test]
fn native_amx_qc_binary_decode_preserves_layout_and_rejects_misalignment() {
    let qc = sample_native_amx_invariant_qc();
    let wire = native_amx_qc_wire(&qc);
    assert_eq!(
        qc.encode(),
        wire.encode(),
        "checked construction must retain the canonical flat V1 wire layout"
    );
    assert_eq!(
        NativeAmxAttestationQcV2::decode(&mut qc.encode().as_slice()).expect("aligned QC decodes"),
        qc
    );

    let mut malformed_wire = wire;
    malformed_wire
        .validator_set_pops
        .pop()
        .expect("fixture contains validator proofs");
    assert!(
        NativeAmxAttestationQcV2::decode(&mut malformed_wire.encode().as_slice()).is_err(),
        "binary decoding must not construct misaligned validator material"
    );
}

#[test]
fn native_amx_qc_json_decode_rejects_misaligned_validator_material() {
    let qc = sample_native_amx_invariant_qc();
    let mut value = norito::json::to_value(&qc).expect("serialize aligned QC");
    value
        .as_object_mut()
        .and_then(|object| object.get_mut("validator_set_pops"))
        .and_then(norito::json::Value::as_array_mut)
        .and_then(Vec::pop)
        .expect("fixture JSON contains validator proofs");

    assert!(
        norito::json::from_value::<NativeAmxAttestationQcV2>(value).is_err(),
        "JSON decoding must not construct misaligned validator material"
    );
    assert_eq!(
        norito::json::from_value::<NativeAmxAttestationQcV2>(
            norito::json::to_value(&qc).expect("serialize aligned QC")
        )
        .expect("aligned QC JSON decodes"),
        qc
    );
}

fn sample_native_amx_participant_proposal(
    body: &NativeAmxAttestationBodyV2,
    validator_set: Vec<PeerId>,
) -> LaneBlockProposalV1 {
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: body.participant_lane_id,
        dataspace_id: body.participant_dataspace_id,
        lane_incarnation: body.participant_lane_incarnation,
        proposal_height: body.authority_context_height,
        previous_lane_block_height: body.participant_previous_block_height,
        previous_lane_block_descriptor_hash: body.participant_previous_block_descriptor_hash,
        lane_block_height: body.participant_lane_block_height,
        lane_block_view: body.participant_lane_block_view,
        subject_hash: Hash::new(b"native-amx-model-participant-subject"),
        payload_ownership_hash: Hash::new(b"native-amx-model-participant-ownership"),
        rbc_instance_hash: Hash::new(b"native-amx-model-participant-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: body.participant_validator_set_hash,
        validator_set,
        validator_count: body.participant_validator_count,
        min_quorum: body.participant_min_quorum,
        qc_mode_tag: "permissioned:native-amx-model".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    proposal
}

fn sample_native_amx_leg(
    source_id: [u8; 32],
    plan_digest: Hash,
    coordinator: (LaneId, DataSpaceId),
    participant: (LaneId, DataSpaceId),
    validator_set: &[PeerId],
) -> NativeAmxLegRecordV2 {
    let prepare_qc = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        source_id,
        plan_digest,
        coordinator,
        participant,
        validator_set.to_vec(),
    );
    let commit_qc = sample_native_amx_qc(
        NativeAmxPhase::Commit,
        source_id,
        plan_digest,
        coordinator,
        participant,
        validator_set.to_vec(),
    );
    let participant_proposal = sample_native_amx_participant_proposal(
        &prepare_qc.body,
        prepare_qc.validator_set().to_vec(),
    );
    debug_assert_eq!(
        prepare_qc.body.participant_proposal_hash,
        participant_proposal.proposal_hash
    );
    debug_assert_eq!(
        commit_qc.body.participant_proposal_hash,
        participant_proposal.proposal_hash
    );
    let participant_settlement = prepare_qc
        .body
        .computed_grouped_participant_settlement(&[prepare_qc.body.source_id])
        .expect("single-source test fixture settlement is valid");
    let participant_settlement_hash =
        crate::nexus::compute_settlement_hash(&participant_settlement)
            .expect("fixture participant settlement hashes");
    NativeAmxLegRecordV2 {
        lane_id: participant.0,
        dataspace_id: participant.1,
        participant_proposal,
        participant_settlement,
        participant_settlement_hash,
        prepare_qc,
        commit_qc,
    }
}

fn grouped_native_amx_fixture_document() -> norito::json::Value {
    norito::json::from_str(include_str!(
        "../../../../fixtures/sumeragi_v2/native_amx_v2_grouped.json"
    ))
    .expect("decode Rust-owned grouped Native AMX fixture document")
}

fn grouped_native_amx_commitment_fixture() -> LaneBlockCommitment {
    let commitment = grouped_native_amx_fixture_document()
        .get("golden")
        .and_then(|golden| golden.get("receipt_group"))
        .cloned()
        .expect("grouped Native AMX fixture contains golden receipt group");
    norito::json::from_value(commitment)
        .expect("decode Rust-owned grouped Native AMX lane commitment")
}

#[expect(
    clippy::too_many_lines,
    reason = "this ordered fail-closed fixture validator follows the complete Native AMX evidence pipeline and preserves first-error intent across its canonical anchors"
)]
fn validate_grouped_native_amx_application_evidence(
    document: &norito::json::Value,
) -> Result<(), &'static str> {
    use crate::block::consensus_v2::{ExecutionCommitment, NativeAmxApplicationManifestLeafV1};

    let evidence = document
        .pointer("/golden/application_evidence")
        .ok_or("fixture is missing application evidence")?;
    let execution: ExecutionCommitment = norito::json::from_value(
        evidence
            .get("execution_commitment")
            .cloned()
            .ok_or("fixture is missing execution commitment")?,
    )
    .map_err(|_| "execution commitment is malformed")?;
    execution
        .validate()
        .map_err(|_| "execution commitment is invalid")?;
    let artifacts = evidence
        .get("manifest_artifacts")
        .and_then(norito::json::Value::as_array)
        .ok_or("manifest artifacts are malformed")?;
    if artifacts.len() != 1 || execution.native_amx_application_manifest_count != 1 {
        return Err("fixture must contain one separate-participant manifest");
    }
    let artifact = &artifacts[0];
    if artifact
        .get("version")
        .and_then(norito::json::Value::as_u64)
        != Some(1)
        || artifact
            .get("manifest_leaf_count")
            .and_then(norito::json::Value::as_u64)
            != Some(1)
        || artifact
            .get("leaf_index")
            .and_then(norito::json::Value::as_u64)
            != Some(0)
    {
        return Err("manifest artifact geometry is invalid");
    }
    let leaf: NativeAmxApplicationManifestLeafV1 = norito::json::from_value(
        artifact
            .get("leaf")
            .cloned()
            .ok_or("manifest leaf is missing")?,
    )
    .map_err(|_| "manifest leaf is malformed")?;
    leaf.validate().map_err(|_| "manifest leaf is invalid")?;
    let leaf_hash = HashOf::new(&leaf);
    let advertised_leaf_hash: Hash = norito::json::from_value(
        artifact
            .get("leaf_hash")
            .cloned()
            .ok_or("manifest leaf hash is missing")?,
    )
    .map_err(|_| "manifest leaf hash is malformed")?;
    let manifest_root: Hash = norito::json::from_value(
        artifact
            .get("manifest_root")
            .cloned()
            .ok_or("manifest root is missing")?,
    )
    .map_err(|_| "manifest root is malformed")?;
    let proof: MerkleProof<NativeAmxApplicationManifestLeafV1> = norito::json::from_value(
        artifact
            .get("proof")
            .cloned()
            .ok_or("manifest proof is missing")?,
    )
    .map_err(|_| "manifest proof is malformed")?;
    let typed_root =
        HashOf::<MerkleTree<NativeAmxApplicationManifestLeafV1>>::from_untyped_unchecked(
            manifest_root,
        );
    let manifest_leaf_count =
        NonZeroU64::new(u64::from(execution.native_amx_application_manifest_count))
            .ok_or("manifest commitment leaf count is zero")?;
    let manifest_commitment = MerkleTreeCommitment::new(typed_root, manifest_leaf_count);
    if Hash::from(leaf_hash) != advertised_leaf_hash
        || manifest_root != execution.native_amx_application_manifest_root
        || leaf.executed_block_wire_hash != execution.executed_block_wire_hash
        || !proof.verify(&leaf_hash, &manifest_commitment)
    {
        return Err("manifest proof does not authenticate the leaf");
    }

    let active = evidence
        .get("active_lane_incarnations")
        .and_then(norito::json::Value::as_array)
        .and_then(|rows| rows.first())
        .ok_or("active incarnation is missing")?;
    let active_incarnation: Hash = norito::json::from_value(
        active
            .get("lane_incarnation")
            .cloned()
            .ok_or("active incarnation hash is missing")?,
    )
    .map_err(|_| "active incarnation hash is malformed")?;
    if active.get("lane_id").and_then(norito::json::Value::as_u64)
        != Some(u64::from(leaf.lane_id.as_u32()))
        || active
            .get("dataspace_id")
            .and_then(norito::json::Value::as_u64)
            != Some(leaf.dataspace_id.as_u64())
        || active_incarnation != leaf.lane_incarnation
    {
        return Err("manifest leaf targets a stale incarnation");
    }

    let commitment: LaneBlockCommitment = norito::json::from_value(
        document
            .pointer("/golden/receipt_group")
            .cloned()
            .ok_or("receipt group is missing")?,
    )
    .map_err(|_| "receipt group is malformed")?;
    if leaf.lane_id == commitment.lane_id && leaf.dataspace_id == commitment.dataspace_id {
        return Err("same-route coordinator has separate application evidence");
    }
    let carrier_entrypoints: Vec<Hash> = norito::json::from_value(
        evidence
            .get("carrier_entrypoint_hashes")
            .cloned()
            .ok_or("carrier entrypoints are missing")?,
    )
    .map_err(|_| "carrier entrypoints are malformed")?;
    if leaf.members.len() != commitment.native_amx_receipts.len() {
        return Err("manifest source count differs from receipt group");
    }
    for (member, receipt) in leaf.members.iter().zip(&commitment.native_amx_receipts) {
        if member.source_id != receipt.source_id {
            return Err("manifest source order differs from receipt group");
        }
        let leg = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == leaf.lane_id && leg.dataspace_id == leaf.dataspace_id)
            .ok_or("manifest participant route is missing from receipt")?;
        let descriptor = &leg.participant_proposal.descriptor;
        let participant_height_matches_descriptor =
            descriptor.lane_block_height == leaf.participant_height;
        if descriptor.lane_incarnation != leaf.lane_incarnation
            || !participant_height_matches_descriptor
            || descriptor.lane_block_view != leaf.participant_view
            || descriptor.previous_lane_block_height != leaf.predecessor_height
            || descriptor.previous_lane_block_descriptor_hash != leaf.predecessor_descriptor_hash
            || descriptor.descriptor_hash != leaf.descriptor_hash
            || leg.participant_proposal.proposal_hash != leaf.proposal_hash
            || leg.participant_settlement_hash != leaf.settlement_hash
            || leg.prepare_qc.body.source_id != member.source_id
            || leg.prepare_qc.body.tx_entrypoint_hash != member.entrypoint_hash
            || !descriptor
                .accepted_transaction_hashes
                .iter()
                .all(|hash| carrier_entrypoints.contains(hash))
        {
            return Err("manifest participant identity or mixed-role anchor differs");
        }
    }

    let diagnostics: SumeragiDiagnosticsStatus = norito::json::from_value(
        document
            .pointer("/golden/expected_diagnostics")
            .cloned()
            .ok_or("diagnostics projection is missing")?,
    )
    .map_err(|_| "diagnostics projection is malformed")?;
    let row = diagnostics
        .native_amx_participant_applications
        .first()
        .ok_or("diagnostics application row is missing")?;
    if row.lane_id != leaf.lane_id
        || row.dataspace_id != leaf.dataspace_id
        || row.lane_incarnation != leaf.lane_incarnation
        || row.participant_height != leaf.participant_height
        || row.participant_view != leaf.participant_view
        || row.predecessor_height != leaf.predecessor_height
        || row.predecessor_descriptor_hash != leaf.predecessor_descriptor_hash
        || row.descriptor_hash != leaf.descriptor_hash
        || row.proposal_hash != leaf.proposal_hash
        || row.settlement_hash != leaf.settlement_hash
        || row.source_count != leaf.members.len() as u64
        || row.application_block_height != Some(leaf.application_block_height)
        || row.application_block_hash != Some(leaf.application_block_hash)
    {
        return Err("diagnostics row differs from application manifest");
    }
    Ok(())
}

fn refresh_native_amx_participant_proposal(leg: &mut NativeAmxLegRecordV2) {
    leg.participant_proposal.descriptor.descriptor_hash = leg
        .participant_proposal
        .descriptor
        .computed_descriptor_hash();
    leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
    for qc in [&mut leg.prepare_qc, &mut leg.commit_qc] {
        qc.body.participant_lane_block_view = leg.participant_proposal.descriptor.lane_block_view;
        qc.body.participant_proposal_hash = leg.participant_proposal.proposal_hash;
    }
}

fn remove_grouped_native_amx_fixture_path(
    document: &mut norito::json::Value,
    path: &str,
    control_id: &str,
) {
    let (parent_path, token) = path
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("control `{control_id}` remove path has a parent"));
    let token = token.replace("~1", "/").replace("~0", "~");
    match document
        .pointer_mut(parent_path)
        .unwrap_or_else(|| panic!("control `{control_id}` remove parent resolves"))
    {
        norito::json::Value::Object(object) => {
            assert!(
                object.remove(&token).is_some(),
                "control `{control_id}` removes an existing field"
            );
        }
        norito::json::Value::Array(array) => {
            let index = token
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("control `{control_id}` remove index is canonical"));
            assert!(
                index < array.len(),
                "control `{control_id}` removes an existing member"
            );
            array.remove(index);
        }
        _ => panic!("control `{control_id}` remove parent is a container"),
    }
}

fn apply_grouped_native_amx_fixture_mutation(
    document: &mut norito::json::Value,
    mutation: &norito::json::Value,
    control_id: &str,
) {
    let operation = mutation
        .get("op")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("control `{control_id}` mutation has an operation"));
    let path = mutation
        .get("path")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("control `{control_id}` mutation has a path"));
    match operation {
        "replace" => {
            let replacement = mutation
                .get("value")
                .cloned()
                .unwrap_or_else(|| panic!("control `{control_id}` replace has a value"));
            *document
                .pointer_mut(path)
                .unwrap_or_else(|| panic!("control `{control_id}` replace path resolves")) =
                replacement;
        }
        "remove" => remove_grouped_native_amx_fixture_path(document, path, control_id),
        "swap" => {
            let value = mutation
                .get("value")
                .and_then(norito::json::Value::as_object)
                .unwrap_or_else(|| panic!("control `{control_id}` swap has geometry"));
            let left = value
                .get("left")
                .and_then(norito::json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` swap left index is bounded"));
            let right = value
                .get("right")
                .and_then(norito::json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` swap right index is bounded"));
            let array = document
                .pointer_mut(path)
                .and_then(norito::json::Value::as_array_mut)
                .unwrap_or_else(|| panic!("control `{control_id}` swap path is an array"));
            assert!(
                left < array.len() && right < array.len(),
                "control `{control_id}` swaps existing members"
            );
            array.swap(left, right);
        }
        "copy" => {
            let source_path = mutation
                .get("value")
                .and_then(norito::json::Value::as_object)
                .and_then(|value| value.get("from"))
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| panic!("control `{control_id}` copy has a source path"));
            let replacement = document
                .pointer(source_path)
                .cloned()
                .unwrap_or_else(|| panic!("control `{control_id}` copy source resolves"));
            *document
                .pointer_mut(path)
                .unwrap_or_else(|| panic!("control `{control_id}` copy target resolves")) =
                replacement;
        }
        "repeat" => {
            let value = mutation
                .get("value")
                .and_then(norito::json::Value::as_object)
                .unwrap_or_else(|| panic!("control `{control_id}` repeat has geometry"));
            let source_index = value
                .get("source_index")
                .and_then(norito::json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` repeat source index is bounded"));
            let count = value
                .get("count")
                .and_then(norito::json::Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` repeat count is bounded"));
            assert!(
                count <= NATIVE_AMX_GROUP_SOURCES_MAX + 1,
                "control `{control_id}` repeat remains bounded"
            );
            let array = document
                .pointer_mut(path)
                .and_then(norito::json::Value::as_array_mut)
                .unwrap_or_else(|| panic!("control `{control_id}` repeat path is an array"));
            let source = array
                .get(source_index)
                .cloned()
                .unwrap_or_else(|| panic!("control `{control_id}` repeat source exists"));
            *array = vec![source; count];
        }
        _ => panic!("control `{control_id}` uses supported mutation `{operation}`"),
    }
}

#[test]
fn native_amx_receipt_negative_corpus_fails_closed() {
    const EXPECTED_RECEIPT_CONTROLS: usize = 45;

    let canonical = grouped_native_amx_fixture_document();
    let controls = canonical
        .get("negative_controls")
        .and_then(norito::json::Value::as_array)
        .expect("fixture contains negative controls");
    let mut evaluated = 0_usize;
    for control in controls {
        if control
            .get("validator")
            .and_then(norito::json::Value::as_str)
            != Some("receipt_group")
        {
            continue;
        }
        evaluated = evaluated.saturating_add(1);
        let id = control
            .get("id")
            .and_then(norito::json::Value::as_str)
            .expect("control has id");
        let mut mutated = canonical.clone();
        for mutation in control
            .get("mutations")
            .and_then(norito::json::Value::as_array)
            .expect("control has mutations")
        {
            apply_grouped_native_amx_fixture_mutation(&mut mutated, mutation, id);
        }
        let receipt_group = mutated
            .pointer("/golden/receipt_group")
            .cloned()
            .unwrap_or_else(|| panic!("control `{id}` retains the receipt group"));
        if matches!(
            id,
            "coherent_duplicate_validator_set" | "coherent_over_quorum_requirement"
        ) {
            assert_eq!(
                mutated.pointer("/golden/expected_diagnostics/lane_settlement_commitments/0",),
                Some(&receipt_group),
                "coherent committee control `{id}` rebuilds the diagnostics projection"
            );
            validate_grouped_native_amx_application_evidence(&mutated).unwrap_or_else(|error| {
                panic!("coherent committee control `{id}` preserves application evidence: {error}")
            });
            let commitment: LaneBlockCommitment = norito::json::from_value(receipt_group.clone())
                .unwrap_or_else(|error| {
                    panic!("coherent committee control `{id}` remains decodable: {error}")
                });
            assert!(
                commitment.validate_native_amx_receipts().is_err(),
                "coherent committee control `{id}` must fail only receipt validation"
            );
            continue;
        }
        let rejected = norito::json::from_value::<LaneBlockCommitment>(receipt_group.clone())
            .map_or(true, |commitment| {
                commitment.validate_native_amx_receipts().is_err()
                    || norito::json::to_value(&commitment)
                        .map_or(true, |canonical| canonical != receipt_group)
            });
        assert!(
            rejected,
            "receipt-group negative control `{id}` must fail closed in Rust"
        );
    }
    assert_eq!(
        evaluated, EXPECTED_RECEIPT_CONTROLS,
        "Rust must execute every declared receipt-group negative control"
    );
}

#[test]
fn native_amx_application_evidence_negative_corpus_fails_closed() {
    const EXPECTED_APPLICATION_EVIDENCE_CONTROLS: usize = 10;

    let canonical = grouped_native_amx_fixture_document();
    validate_grouped_native_amx_application_evidence(&canonical)
        .expect("the canonical application evidence must be valid before mutation");
    let controls = canonical
        .get("negative_controls")
        .and_then(norito::json::Value::as_array)
        .expect("fixture contains negative controls");
    let mut evaluated = 0_usize;
    for control in controls {
        if control
            .get("validator")
            .and_then(norito::json::Value::as_str)
            != Some("application_evidence")
        {
            continue;
        }
        evaluated = evaluated.saturating_add(1);
        let id = control
            .get("id")
            .and_then(norito::json::Value::as_str)
            .expect("control has id");
        let mut mutated = canonical.clone();
        for mutation in control
            .get("mutations")
            .and_then(norito::json::Value::as_array)
            .expect("control has mutations")
        {
            apply_grouped_native_amx_fixture_mutation(&mut mutated, mutation, id);
        }
        assert!(
            validate_grouped_native_amx_application_evidence(&mutated).is_err(),
            "application evidence negative control `{id}` must fail closed"
        );
    }
    assert_eq!(
        evaluated, EXPECTED_APPLICATION_EVIDENCE_CONTROLS,
        "Rust must execute every declared application-evidence negative control"
    );
}

#[test]
fn native_amx_application_evidence_rejects_coherently_wrong_manifest_count() {
    let mut document = grouped_native_amx_fixture_document();
    *document
            .pointer_mut(
                "/golden/application_evidence/execution_commitment/native_amx_application_manifest_count",
            )
            .expect("execution manifest count exists") = norito::json::Value::from(2_u64);
    *document
        .pointer_mut("/golden/application_evidence/manifest_artifacts/0/manifest_leaf_count")
        .expect("artifact manifest count exists") = norito::json::Value::from(2_u64);

    assert_eq!(
        validate_grouped_native_amx_application_evidence(&document),
        Err("fixture must contain one separate-participant manifest"),
        "the same singleton root and proof must not be rebound to a coherent wrong count"
    );
}

#[test]
fn native_amx_grouped_receipts_reject_order_bounds_and_same_route_drift() {
    let mut unordered = grouped_native_amx_commitment_fixture();
    unordered.native_amx_receipts.swap(0, 1);
    assert_eq!(
        unordered.validate_native_amx_receipts(),
        Err("Native AMX receipt sources must be strictly ordered")
    );

    let mut oversized = grouped_native_amx_commitment_fixture();
    let template = oversized.native_amx_receipts[0].legs[0]
        .participant_settlement
        .receipts[0]
        .clone();
    oversized.native_amx_receipts[0].legs[0]
        .participant_settlement
        .receipts = vec![template; NATIVE_AMX_GROUP_SOURCES_MAX + 1];
    assert_eq!(
        oversized.validate_native_amx_receipts(),
        Err("Native AMX participant settlement is structurally invalid")
    );

    let mut same_route_drift = grouped_native_amx_commitment_fixture();
    let receipt = &mut same_route_drift.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == coordinator_route)
        .expect("fixture contains same-route coordinator leg");
    leg.participant_proposal.descriptor.lane_block_view = leg
        .participant_proposal
        .descriptor
        .lane_block_view
        .saturating_add(1);
    refresh_native_amx_participant_proposal(leg);
    assert_eq!(
        same_route_drift.validate_native_amx_receipts(),
        Err("Native AMX same-route leg differs from the coordinator identity")
    );
}

#[test]
fn native_amx_grouped_receipts_reject_cross_context_height_drift() {
    let mut commitment_height_drift = grouped_native_amx_commitment_fixture();
    let receipt = &mut commitment_height_drift.native_amx_receipts[0];
    receipt.lane_block_height = receipt.lane_block_height.saturating_add(1);
    assert_eq!(
        commitment_height_drift.validate_native_amx_receipts(),
        Err("Native AMX receipt coordinator identity is invalid"),
        "a receipt lane height belongs to the containing lane commitment, not an unrelated receipt field"
    );

    let mut proposal_context_drift = grouped_native_amx_commitment_fixture();
    let receipt = &mut proposal_context_drift.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == coordinator_route)
        .expect("fixture contains same-route coordinator leg");
    leg.participant_proposal.descriptor.proposal_height = leg
        .participant_proposal
        .descriptor
        .proposal_height
        .saturating_add(1);
    refresh_native_amx_participant_proposal(leg);
    assert_eq!(
        proposal_context_drift.validate_native_amx_receipts(),
        Err("Native AMX participant leg identity is internally inconsistent"),
        "a participant proposal height is bound to the coordinator authority context"
    );
}

#[test]
fn native_amx_mixed_role_marker_defers_only_separate_participant_anchor() {
    let mut mixed_role = grouped_native_amx_commitment_fixture();
    let receipt = &mut mixed_role.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) != coordinator_route)
        .expect("fixture contains a separate participant leg");
    let current_entrypoint = Hash::from(leg.prepare_qc.body.tx_entrypoint_hash);
    let position = leg
        .participant_proposal
        .descriptor
        .accepted_transaction_hashes
        .iter()
        .position(|hash| *hash == current_entrypoint)
        .expect("fixture participant contains current entrypoint");
    leg.participant_proposal
        .descriptor
        .accepted_transaction_hashes[position] = Hash::new(b"mixed-role executable anchor member");
    refresh_native_amx_participant_proposal(leg);
    assert!(leg.requires_mixed_role_anchor_validation());
    mixed_role
        .validate_native_amx_receipts()
        .expect("separate participant may defer exact block-wide anchor validation");

    let mut same_route = grouped_native_amx_commitment_fixture();
    let receipt = &mut same_route.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == coordinator_route)
        .expect("fixture contains same-route coordinator leg");
    let current_entrypoint = Hash::from(leg.prepare_qc.body.tx_entrypoint_hash);
    let position = leg
        .participant_proposal
        .descriptor
        .accepted_transaction_hashes
        .iter()
        .position(|hash| *hash == current_entrypoint)
        .expect("fixture coordinator contains current entrypoint");
    leg.participant_proposal
        .descriptor
        .accepted_transaction_hashes[position] = Hash::new(b"invalid same-route mixed-role member");
    refresh_native_amx_participant_proposal(leg);
    assert!(leg.requires_mixed_role_anchor_validation());
    assert_eq!(
        same_route.validate_native_amx_receipts(),
        Err("Native AMX same-route leg differs from the coordinator identity")
    );
}

#[test]
fn native_amx_grouped_receipts_reject_qc_and_group_membership_drift() {
    let mut malformed_bitmap = grouped_native_amx_commitment_fixture();
    malformed_bitmap.native_amx_receipts[0].legs[0]
        .prepare_qc
        .signers_bitmap = vec![0b0000_0011];
    assert_eq!(
        malformed_bitmap.validate_native_amx_receipts(),
        Err("Native AMX participant QC is structurally invalid")
    );

    let mut duplicate_leg = grouped_native_amx_commitment_fixture();
    duplicate_leg.native_amx_receipts[0].legs[1] =
        duplicate_leg.native_amx_receipts[0].legs[0].clone();
    assert_eq!(
        duplicate_leg.validate_native_amx_receipts(),
        Err("Native AMX receipt contains duplicate participant routes")
    );

    let mut group_drift = grouped_native_amx_commitment_fixture();
    group_drift.native_amx_receipts[0].legs[0]
        .participant_settlement
        .receipts
        .swap(0, 1);
    assert_eq!(
        group_drift.validate_native_amx_receipts(),
        Err("Native AMX participant settlement is structurally invalid")
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

#[test]
fn native_amx_receipts_change_lane_block_commitment_hash_inputs() {
    let plan_digest = Hash::new(b"test-native-amx-plan");
    let source_id = [0xAB; 32];
    let coordinator_lane_id = LaneId::new(0);
    let coordinator_dataspace_id = DataSpaceId::UNIVERSAL;
    let validators = sample_roster();
    let base = LaneBlockCommitment {
        block_height: 42,
        lane_id: coordinator_lane_id,
        lane_incarnation: Hash::new(b"amx-commitment-test-incarnation"),
        dataspace_id: coordinator_dataspace_id,
        tx_count: 1,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: vec![NativeAmxReceipt {
            version: 2,
            source_id,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"native-amx-model-genesis",
            ))),
            plan_digest,
            lane_id: coordinator_lane_id,
            dataspace_id: coordinator_dataspace_id,
            lane_incarnation: Hash::new(b"native-amx-model-coordinator"),
            authority_context_height: 42,
            lane_block_height: 7,
            lane_block_view: 2,
            coordinator_proposal_hash: Hash::new(b"native-amx-model-proposal"),
            legs: vec![
                sample_native_amx_leg(
                    source_id,
                    plan_digest,
                    (coordinator_lane_id, coordinator_dataspace_id),
                    (LaneId::new(7), DataSpaceId::new(7)),
                    &validators,
                ),
                sample_native_amx_leg(
                    source_id,
                    plan_digest,
                    (coordinator_lane_id, coordinator_dataspace_id),
                    (LaneId::new(8), DataSpaceId::new(8)),
                    &validators,
                ),
            ],
        }],
    };
    let mut changed = base.clone();
    changed.native_amx_receipts[0].legs[1].commit_qc.body.phase = NativeAmxPhase::Prepare;

    assert_ne!(Hash::new(base.encode()), Hash::new(changed.encode()));
}

#[test]
fn native_amx_v2_grouped_participant_settlement_is_exact_zero_effect_evidence() {
    let source_id = [0xC7; 32];
    let ordered_sources = [[0xC6; 32], source_id];
    let body = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        source_id,
        Hash::new(b"v2-zero-effect-settlement-plan"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
    .body;
    let settlement = body
        .computed_grouped_participant_settlement(&ordered_sources)
        .expect("ordered grouped participant settlement");

    assert_eq!(settlement.block_height, body.participant_lane_block_height);
    assert_eq!(settlement.lane_id, body.participant_lane_id);
    assert_eq!(
        settlement.lane_incarnation,
        body.participant_lane_incarnation
    );
    assert_eq!(settlement.dataspace_id, body.participant_dataspace_id);
    assert_eq!(settlement.tx_count, 2);
    assert!(settlement.total_local_amount.is_zero());
    assert!(settlement.total_xor_due.is_zero());
    assert!(settlement.total_xor_after_haircut.is_zero());
    assert!(settlement.total_xor_variance.is_zero());
    assert!(settlement.swap_metadata.is_none());
    assert!(settlement.nexus_fee_receipts.is_empty());
    assert!(settlement.native_amx_receipts.is_empty());
    assert_eq!(
        settlement
            .receipts
            .iter()
            .map(|receipt| receipt.source_id)
            .collect::<Vec<_>>(),
        ordered_sources
    );
    assert!(settlement.receipts.iter().all(|receipt| {
        receipt.local_amount.is_zero()
            && receipt.xor_due.is_zero()
            && receipt.xor_after_haircut.is_zero()
            && receipt.xor_variance.is_zero()
            && receipt.timestamp_ms == body.authority_context_height
    }));
    assert_eq!(
        Hash::from(
            crate::nexus::compute_settlement_hash(&settlement)
                .expect("computed participant settlement must hash")
        ),
        body.computed_grouped_participant_settlement_commitment(&ordered_sources)
            .expect("ordered grouped participant commitment")
    );

    let encoded = norito::to_bytes(&settlement).expect("encode participant settlement");
    let decoded = norito::decode_from_bytes::<LaneBlockCommitment>(&encoded)
        .expect("decode participant settlement");
    assert_eq!(decoded, settlement);
}

#[test]
fn native_amx_v2_grouped_participant_settlement_rejects_invalid_source_groups() {
    let body = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        [0x31; 32],
        Hash::new(b"v2-invalid-source-group-plan"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
    .body;

    assert!(body.computed_grouped_participant_settlement(&[]).is_err());
    assert!(
        body.computed_grouped_participant_settlement(&[[0x32; 32]])
            .is_err()
    );
    assert!(
        body.computed_grouped_participant_settlement(&[body.source_id, body.source_id])
            .is_err()
    );
    assert!(
        body.computed_grouped_participant_settlement(&[[0x32; 32], body.source_id])
            .is_err()
    );
    assert!(
        body.computed_grouped_participant_settlement(&vec![
            body.source_id;
            NATIVE_AMX_GROUP_SOURCES_MAX + 1
        ])
        .is_err()
    );
}

#[test]
fn native_amx_v2_attestation_preimage_binds_round_and_epoch() {
    let body = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        [0x31; 32],
        Hash::new(b"v2-context-bound-plan"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
    .body;
    let preimage = body.signature_preimage();
    {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            body.signature_preimage(),
            preimage,
            "Native AMX signature identity must ignore the caller's ambient Norito layout"
        );
    }
    let mut another_view = body;
    another_view.round.view = another_view.round.view.saturating_add(1);
    let mut another_epoch = body;
    another_epoch.epoch = another_epoch.epoch.saturating_add(1);

    assert!(preimage.starts_with(b"iroha:native-amx:v2"));
    assert_ne!(preimage, another_view.signature_preimage());
    assert_ne!(preimage, another_epoch.signature_preimage());
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

fn sample_proposal() -> Proposal {
    Proposal {
        header: sample_consensus_header(),
        payload_hash: Hash::new(b"payload"),
    }
}

fn sample_reconfig() -> Reconfig {
    let peers = (0..2)
        .map(|_| PeerId::new(checked_random_keypair().public_key().clone()))
        .collect();
    Reconfig {
        new_roster: peers,
        activation_height: 42,
    }
}

fn sample_rbc_init() -> RbcInit {
    let roster = sample_roster();
    let roster_hash = roster_hash(&roster);
    let chunk_digests = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
    let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone())
        .root()
        .map(Hash::from)
        .expect("chunk root");
    let block_header = BlockHeader::new(
        NonZeroU64::new(6).expect("block height must be non-zero"),
        None,
        None,
        None,
        0,
        3,
    );
    let leader_key = checked_random_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, leader_private) = leader_key.into_parts();
    let leader_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(&leader_private, block_header.hash())
            .expect("checked RBC init leader fixture signature"),
    );
    RbcInit {
        block_hash: block_header.hash(),
        height: 6,
        view: 3,
        epoch: 1,
        roster,
        roster_hash,
        total_chunks: 3,
        encoding: RbcEncoding::Plain,
        chunk_size_bytes: 128,
        payload_size_bytes: 257,
        data_shards: 0,
        parity_shards: 0,
        chunk_digests,
        payload_hash: Hash::new(b"payload_hash"),
        chunk_root,
        block_header,
        leader_signature,
    }
}

fn sample_rbc_chunk() -> RbcChunk {
    RbcChunk {
        block_hash: dummy_hash(),
        height: 6,
        view: 3,
        epoch: 1,
        idx: 1,
        bytes: vec![1, 2, 3, 4],
    }
}

fn sample_rbc_init_request() -> RbcInitRequest {
    RbcInitRequest {
        block_hash: dummy_hash(),
        height: 6,
        view: 3,
    }
}

fn sample_rbc_chunk_request() -> RbcChunkRequest {
    RbcChunkRequest {
        block_hash: dummy_hash(),
        height: 6,
        view: 3,
        missing_indices: vec![0, 2, 5],
    }
}

fn sample_rbc_ready() -> RbcReady {
    let roster = sample_roster();
    RbcReady {
        block_hash: dummy_hash(),
        height: 6,
        view: 3,
        epoch: 1,
        roster_hash: roster_hash(&roster),
        chunk_root: Hash::prehashed([0xAA; Hash::LENGTH]),
        sender: 2,
        signature: vec![0x10, 0x11],
    }
}

fn sample_rbc_deliver() -> RbcDeliver {
    let roster = sample_roster();
    RbcDeliver {
        block_hash: dummy_hash(),
        height: 6,
        view: 3,
        epoch: 1,
        roster_hash: roster_hash(&roster),
        chunk_root: Hash::prehashed([0xAA; Hash::LENGTH]),
        sender: 2,
        signature: vec![0x21, 0x22],
        ready_signatures: vec![RbcReadySignature {
            sender: 1,
            signature: vec![0x31, 0x32],
        }],
    }
}

fn sample_vrf_commit() -> VrfCommit {
    VrfCommit {
        epoch: 7,
        commitment: [0xAB; 32],
        signer: 5,
        bls_sig: Vec::new(),
    }
}

fn sample_vrf_reveal() -> VrfReveal {
    VrfReveal {
        epoch: 7,
        reveal: [0xCD; 32],
        signer: 5,
        bls_sig: Vec::new(),
    }
}

#[test]
fn qc_roundtrip_encode_decode() {
    let roster = sample_roster();
    let highest = sample_qc_ref();
    let cert = Qc {
        phase: CertPhase::NewView,
        subject_block_hash: highest.subject_block_hash,
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        height: highest.height,
        view: 7,
        epoch: 0,
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: Some(highest),
        validator_set_hash: HashOf::new(&roster),
        validator_set_hash_version: 1,
        validator_set: roster,
        aggregate: QcAggregate {
            signers_bitmap: vec![0xAA, 0x01],
            bls_aggregate_signature: vec![1, 2, 3],
        },
    };
    let bytes = cert.encode();
    let dec = Qc::decode(&mut &bytes[..]).expect("decode certificate");
    assert_eq!(cert, dec);
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
fn rbc_repair_requests_roundtrip_codec() {
    let init_request = sample_rbc_init_request();
    let init_bytes = init_request.encode();
    let init_decoded =
        RbcInitRequest::decode(&mut &init_bytes[..]).expect("decode RBC init request");
    assert_eq!(init_request, init_decoded);

    let chunk_request = sample_rbc_chunk_request();
    let chunk_bytes = chunk_request.encode();
    let chunk_decoded =
        RbcChunkRequest::decode(&mut &chunk_bytes[..]).expect("decode RBC chunk request");
    assert_eq!(chunk_request, chunk_decoded);
}

#[test]
fn evidence_roundtrip_codec() {
    let roster = sample_roster();
    let ev = Evidence {
        kind: EvidenceKind::InvalidQc,
        payload: EvidencePayload::InvalidQc {
            certificate: Qc {
                phase: CertPhase::Commit,
                subject_block_hash: dummy_hash(),
                parent_state_root: Hash::new(b"parent_root"),
                post_state_root: Hash::new(b"post_root"),
                height: 12,
                view: 3,
                epoch: 0,
                chain_order_hash: default_chain_order_hash(),
                rechain_seq: 0,
                mode_tag: PERMISSIONED_TAG.to_string(),
                highest_qc: None,
                validator_set_hash: HashOf::new(&roster),
                validator_set_hash_version: 1,
                validator_set: roster,
                aggregate: QcAggregate {
                    signers_bitmap: vec![0xFF],
                    bls_aggregate_signature: vec![4, 5, 6],
                },
            },
            reason: "test".to_string(),
        },
    };
    let bytes = ev.encode();
    let dec = Evidence::decode(&mut &bytes[..]).expect("decode evidence");
    assert_eq!(ev, dec);
}

#[test]
fn censorship_evidence_roundtrip_codec() {
    let key_pair = checked_random_keypair();
    let payload = crate::transaction::TransactionSubmissionReceiptPayload {
        tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
        entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
        signed_transaction_hash: None,
        submitted_at_ms: 10,
        submitted_at_height: 2,
        signer: key_pair.public_key().clone(),
    };
    let receipt = crate::transaction::TransactionSubmissionReceipt::try_sign(payload, &key_pair)
        .expect("checked censorship evidence receipt fixture signature");
    let tx_hash = receipt.payload.tx_hash;
    let ev = Evidence {
        kind: EvidenceKind::Censorship,
        payload: EvidencePayload::Censorship {
            tx_hash,
            receipts: vec![receipt],
        },
    };
    let bytes = ev.encode();
    let dec = Evidence::decode(&mut &bytes[..]).expect("decode censorship evidence");
    assert_eq!(ev, dec);
}

#[test]
fn evidence_record_roundtrip() {
    let ev = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote {
            v1: QcVote {
                phase: CertPhase::Prepare,
                block_hash: dummy_hash(),
                parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                height: 10,
                view: 1,
                epoch: 0,
                chain_order_hash: default_chain_order_hash(),
                rechain_seq: 0,
                highest_qc: None,
                signer: 2,
                bls_sig: vec![],
            },
            v2: QcVote {
                phase: CertPhase::Prepare,
                block_hash: dummy_hash(),
                parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                height: 10,
                view: 1,
                epoch: 0,
                chain_order_hash: default_chain_order_hash(),
                rechain_seq: 0,
                highest_qc: None,
                signer: 2,
                bls_sig: vec![],
            },
        },
    };
    let rec = EvidenceRecord {
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
    let dec = EvidenceRecord::decode(&mut &bytes[..]).expect("decode evidence record");
    assert_eq!(rec, dec);
}

#[test]
fn rbc_ready_decode_from_slice_matches_encode() {
    let ready = RbcReady {
        block_hash: dummy_hash(),
        height: 5,
        view: 1,
        epoch: 0,
        roster_hash: Hash::prehashed([0xAA; Hash::LENGTH]),
        chunk_root: Hash::prehashed([0u8; Hash::LENGTH]),
        sender: 2,
        signature: vec![9, 9, 9],
    };
    let canonical = ready.encode();
    let (decoded, used) = RbcReady::decode_from_slice(&canonical).expect("decode_from_slice ready");
    assert_eq!(ready, decoded);
    assert_eq!(used, canonical.len());
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

#[test]
fn proposal_roundtrip_codec() {
    let prop = sample_proposal();
    let bytes = prop.encode();
    let dec = Proposal::decode(&mut &bytes[..]).expect("decode proposal");
    assert_eq!(prop, dec);
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

include!("consensus/quorum_policy_tests.rs");
include!("consensus/rbc_roundtrip_tail_tests.rs");
include!("consensus/runtime_diagnostics_tests.rs");
include!("consensus/npos_diagnostics_tests.rs");
