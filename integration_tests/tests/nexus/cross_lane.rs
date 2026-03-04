#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Cross-lane manifest and relay proof tests (NX-11).

use std::{
    collections::BTreeMap,
    fs,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    time::Duration,
};

use eyre::{Result, WrapErr};
use iroha::nexus;
use iroha_config::parameters::actual::{GovernanceCatalog, GovernanceModule, LaneRegistry};
use iroha_core::governance::manifest::{GovernanceGuardReason, LaneManifestRegistry};
use iroha_crypto::{Hash, HashOf, LaneCommitmentId};
use iroha_data_model::{
    block::consensus::{LaneBlockCommitment, PERMISSIONED_TAG},
    consensus::{CertPhase, Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
    nexus::{
        DataSpaceId, LaneCatalog, LaneConfig, LaneId, LanePrivacyProof, LaneRelayEnvelope,
        LaneRelayError, LaneStorageProfile, compute_settlement_hash,
    },
    peer::PeerId,
    proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::{core as norito_core, json};
use tempfile::tempdir;

fn sample_commit_qc(header: &iroha_data_model::block::BlockHeader) -> Qc {
    let validator_set: Vec<PeerId> = Vec::new();
    Qc {
        phase: CertPhase::Commit,
        subject_block_hash: header.hash(),
        parent_state_root: Hash::new([0x22; 4]),
        post_state_root: Hash::new([0x11, 0x22, 0x33, 0x44]),
        height: header.height().get(),
        view: 1,
        epoch: 0,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![0b1010_0001],
            bls_aggregate_signature: vec![0x01; 48],
        },
    }
}

#[test]
fn commitment_only_lane_without_privacy_commitments_is_gated() -> Result<()> {
    let alias = "private-lane";
    let lane_id = LaneId::new(42);
    let fixtures = tempdir()?;
    write_manifest(fixtures.path(), alias, false)?;

    let registry = build_registry(
        fixtures.path(),
        lane_id,
        alias,
        LaneStorageProfile::CommitmentOnly,
    )?;
    let err = registry
        .ensure_lane_ready(lane_id)
        .expect_err("lane should be rejected when privacy commitments are missing");
    assert_eq!(
        err.reason(),
        GovernanceGuardReason::MissingPrivacyCommitments
    );
    assert!(
        err.message().contains("privacy commitments"),
        "expected message to mention missing commitments, got: {}",
        err.message()
    );
    Ok(())
}

#[test]
fn commitment_only_lane_with_privacy_commitments_is_ready() -> Result<()> {
    let alias = "confidential-lane";
    let lane_id = LaneId::new(7);
    let fixtures = tempdir()?;
    write_manifest(fixtures.path(), alias, true)?;

    let registry = build_registry(
        fixtures.path(),
        lane_id,
        alias,
        LaneStorageProfile::CommitmentOnly,
    )?;
    registry
        .ensure_lane_ready(lane_id)
        .expect("lane with privacy commitments should be accepted");
    let status = registry
        .status(lane_id)
        .expect("lane status should be registered after manifest load");
    assert_eq!(
        status.privacy_commitments().len(),
        1,
        "lane manifest should expose the configured privacy commitment"
    );
    Ok(())
}

#[test]
fn lane_privacy_proof_attachment_roundtrips() -> Result<()> {
    let leaf = [0xAB_u8; 32];
    let sibling = [0xCD_u8; 32];
    let privacy = LanePrivacyProof::merkle_from_raw_path(
        LaneCommitmentId::new(9),
        leaf,
        0,
        vec![Some(sibling)],
    )?;

    let mut attachment = ProofAttachment::new_inline(
        "lane/privacy".parse()?,
        ProofBox::new("lane/privacy".parse()?, vec![0x01, 0x02]),
        VerifyingKeyBox::new("lane/privacy".parse()?, vec![0xAA, 0xBB]),
    );
    attachment.lane_privacy = Some(privacy);
    let list = ProofAttachmentList(vec![attachment]);

    let norito_bytes = norito::to_bytes(&list)?;
    let archived = norito::from_bytes::<ProofAttachmentList>(&norito_bytes)?;
    let decoded: ProofAttachmentList = norito_core::NoritoDeserialize::deserialize(archived);
    assert_eq!(decoded, list);
    let decoded_privacy = decoded
        .0
        .first()
        .and_then(|entry| entry.lane_privacy.clone())
        .expect("lane privacy attachment present");
    assert_eq!(decoded_privacy.commitment_id, LaneCommitmentId::new(9));
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
fn lane_relay_envelope_must_have_consistent_qc() -> Result<()> {
    let height = NonZeroU64::new(7).expect("nonzero");
    let lane_id = LaneId::new(3);
    let dataspace_id = iroha_data_model::nexus::DataSpaceId::new(2);
    let settlement = LaneBlockCommitment {
        block_height: height.get(),
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 10,
        total_xor_due_micro: 5,
        total_xor_after_haircut_micro: 4,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let mut header =
        iroha_data_model::block::BlockHeader::new(height, None, None, None, 1_700_000_000_000, 0);
    let da_hash = Hash::new([0xAA, 0xBB, 0xCC, 0xDD]);
    let da_commitment_hash = Some(HashOf::from_untyped_unchecked(da_hash));
    header.set_da_commitments_hash(da_commitment_hash);
    let mut qc = sample_commit_qc(&header);

    // Tamper with the QC so the builder surfaces the mismatch.
    qc.subject_block_hash = HashOf::from_untyped_unchecked(Hash::new([0xFF; 4]));
    let err = nexus::CrossLaneTransferBuilder::new(
        header,
        Some(qc),
        da_commitment_hash,
        settlement.clone(),
    )
    .build()
    .expect_err("expected QC subject mismatch");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::QcSubjectMismatch)
    ));

    // Height mismatch should also be rejected.
    let mut height_mismatch_qc = sample_commit_qc(&header);
    height_mismatch_qc.height = header.height().get() + 1;
    let err = nexus::CrossLaneTransferBuilder::new(
        header,
        Some(height_mismatch_qc),
        da_commitment_hash,
        settlement.clone(),
    )
    .build()
    .expect_err("expected QC height mismatch");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::QcHeightMismatch)
    ));

    // Untampered QC should build a verifiable envelope.
    let proof = nexus::CrossLaneTransferBuilder::new(
        header,
        Some(sample_commit_qc(&header)),
        da_commitment_hash,
        settlement,
    )
    .build()
    .expect("valid envelope");
    proof.verify().expect("verification should succeed");
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
fn cross_lane_builder_rejects_settlement_height_mismatch_at_construction() -> Result<()> {
    let lane_id = LaneId::new(20);
    let dataspace_id = DataSpaceId::new(12);
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(10).expect("height"),
        None,
        None,
        None,
        1_700_000_100_000,
        0,
    );
    let settlement = LaneBlockCommitment {
        block_height: 9,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 50,
        total_xor_due_micro: 30,
        total_xor_after_haircut_micro: 28,
        total_xor_variance_micro: 2,
        swap_metadata: None,
        receipts: Vec::new(),
    };

    let err = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect_err("settlement/header height mismatch should fail");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementBlockHeightMismatch)
    ));
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
fn cross_lane_builder_rejects_da_hash_mismatch_at_construction() -> Result<()> {
    let lane_id = LaneId::new(21);
    let dataspace_id = DataSpaceId::new(13);
    let settlement = LaneBlockCommitment {
        block_height: 11,
        lane_id,
        dataspace_id,
        tx_count: 2,
        total_local_micro: 60,
        total_xor_due_micro: 40,
        total_xor_after_haircut_micro: 36,
        total_xor_variance_micro: 4,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let mut header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(11).expect("height"),
        None,
        None,
        None,
        1_700_000_110_000,
        0,
    );
    let header_da_hash = HashOf::from_untyped_unchecked(Hash::new([0x31, 0x41, 0x59, 0x26]));
    header.set_da_commitments_hash(Some(header_da_hash));
    let mismatched_da_hash = Some(HashOf::from_untyped_unchecked(Hash::new([
        0x27, 0x18, 0x28, 0x18,
    ])));

    let err = nexus::CrossLaneTransferBuilder::new(header, None, mismatched_da_hash, settlement)
        .build()
        .expect_err("da hash mismatch should fail");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::DaCommitmentHashMismatch)
    ));
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
fn duplicate_lane_relay_envelopes_are_rejected() -> Result<()> {
    let lane_id = LaneId::new(9);
    let dataspace_id = iroha_data_model::nexus::DataSpaceId::new(5);
    let settlement = LaneBlockCommitment {
        block_height: 12,
        lane_id,
        dataspace_id,
        tx_count: 0,
        total_local_micro: 0,
        total_xor_due_micro: 0,
        total_xor_after_haircut_micro: 0,
        total_xor_variance_micro: 0,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(12).expect("height"),
        None,
        None,
        None,
        1_700_000_010_000,
        0,
    );
    let proof = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect("builder should succeed");
    let envelope = proof.envelope().clone();

    // Happy-path verification.
    nexus::verify_lane_relay_envelopes(std::slice::from_ref(&envelope))
        .expect("single envelope should pass");

    // Ensure the helper rejects duplicate envelopes for the same tuple.
    let err = nexus::verify_lane_relay_envelopes(&[envelope.clone(), envelope])
        .expect_err("duplicate should be rejected");
    if let nexus::CrossLaneProofError::DuplicateProof {
        lane_id,
        dataspace_id,
        block_height,
    } = err
    {
        assert_eq!(lane_id, LaneId::new(9));
        assert_eq!(dataspace_id, iroha_data_model::nexus::DataSpaceId::new(5));
        assert_eq!(block_height, 12);
    } else {
        panic!("expected duplicate proof error, got {err:?}");
    }
    Ok(())
}

#[test]
#[allow(clippy::unnecessary_wraps)]
fn lane_relay_envelope_rejects_settlement_tampering() -> Result<()> {
    let lane_id = LaneId::new(4);
    let dataspace_id = iroha_data_model::nexus::DataSpaceId::new(8);
    let settlement = LaneBlockCommitment {
        block_height: 3,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 50,
        total_xor_due_micro: 25,
        total_xor_after_haircut_micro: 20,
        total_xor_variance_micro: 5,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(3).expect("height"),
        None,
        None,
        None,
        1_700_000_020_000,
        0,
    );
    let proof = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect("builder should succeed");
    let mut envelope = proof.envelope().clone();
    envelope.settlement_hash = HashOf::from_untyped_unchecked(Hash::new([0xEE; 4]));

    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("tamper should fail");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementHashMismatch)
    ));
    Ok(())
}

#[test]
fn lane_relay_envelope_rejects_block_height_tamper() {
    let mut envelope = sample_relay_envelope();
    // Tamper the envelope height; header remains unchanged so verification must fail early.
    envelope.block_height += 1;

    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("height tamper");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::BlockHeightMismatch)
    ));
}

#[test]
fn lane_relay_envelope_rejects_settlement_height_tamper_even_when_rehashed() -> Result<()> {
    let mut envelope = sample_relay_envelope();
    envelope.settlement_commitment.block_height += 1;
    envelope.settlement_hash = compute_settlement_hash(&envelope.settlement_commitment)?;

    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("settlement height");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementBlockHeightMismatch)
    ));
    Ok(())
}

#[test]
fn lane_relay_envelope_rejects_lane_and_dataspace_tamper_with_rehashed_payload() -> Result<()> {
    let mut envelope = sample_relay_envelope();
    envelope.settlement_commitment.lane_id = LaneId::new(77);
    envelope.settlement_hash = compute_settlement_hash(&envelope.settlement_commitment)?;
    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("lane tamper");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementLaneMismatch)
    ));

    let mut envelope = sample_relay_envelope();
    envelope.settlement_commitment.dataspace_id = DataSpaceId::new(99);
    envelope.settlement_hash = compute_settlement_hash(&envelope.settlement_commitment)?;
    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("dataspace tamper");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementDataspaceMismatch)
    ));
    Ok(())
}

#[test]
fn lane_relay_envelope_rejects_da_commitment_tamper() {
    let mut envelope = sample_relay_envelope();
    let bogus_da_hash = HashOf::from_untyped_unchecked(Hash::new([0xEE, 0xAA, 0xBB, 0xCC]));
    envelope.da_commitment_hash = Some(bogus_da_hash);

    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("da tamper");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::DaCommitmentHashMismatch)
    ));
}

#[test]
fn lane_relay_quorum_rejects_out_of_range_signer() {
    let lane_id = LaneId::new(13);
    let dataspace_id = DataSpaceId::new(6);
    let settlement = LaneBlockCommitment {
        block_height: 5,
        lane_id,
        dataspace_id,
        tx_count: 2,
        total_local_micro: 15,
        total_xor_due_micro: 10,
        total_xor_after_haircut_micro: 9,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(5).expect("height"),
        None,
        None,
        None,
        1_700_000_040_000,
        0,
    );
    let mut qc = sample_commit_qc(&header);
    qc.view = 2;
    qc.aggregate.signers_bitmap = vec![0b0010_0000]; // bit 5 set -> exceeds 5 validators
    qc.aggregate.bls_aggregate_signature = vec![0x11; 48];
    let proof = nexus::CrossLaneTransferBuilder::new(header, Some(qc), None, settlement)
        .build()
        .expect("proof");
    let quorum = nexus::LaneRelayQuorumContext::new(5, 3).expect("quorum context");

    let err = proof
        .verify_with_quorum(quorum)
        .expect_err("out-of-range signer");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::InvalidSignerIndex { .. })
    ));
}

#[test]
fn lane_relay_quorum_rejects_zero_signature() {
    let lane_id = LaneId::new(15);
    let dataspace_id = DataSpaceId::new(7);
    let settlement = LaneBlockCommitment {
        block_height: 6,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 20,
        total_xor_due_micro: 12,
        total_xor_after_haircut_micro: 10,
        total_xor_variance_micro: 2,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(6).expect("height"),
        None,
        None,
        None,
        1_700_000_050_000,
        0,
    );
    let mut qc = sample_commit_qc(&header);
    qc.aggregate.signers_bitmap = vec![0b0000_0011];
    qc.aggregate.bls_aggregate_signature = vec![0; 48];
    let proof = nexus::CrossLaneTransferBuilder::new(header, Some(qc), None, settlement)
        .build()
        .expect("proof");
    let quorum = nexus::LaneRelayQuorumContext::new(4, 2).expect("quorum context");

    let err = proof
        .verify_with_quorum(quorum)
        .expect_err("zero signature should fail");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::AggregateSignatureInvalid)
    ));
}

#[test]
fn lane_relay_quorum_requires_quorum_bitmap() {
    let lane_id = LaneId::new(17);
    let dataspace_id = DataSpaceId::new(8);
    let settlement = LaneBlockCommitment {
        block_height: 7,
        lane_id,
        dataspace_id,
        tx_count: 3,
        total_local_micro: 30,
        total_xor_due_micro: 18,
        total_xor_after_haircut_micro: 16,
        total_xor_variance_micro: 2,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(7).expect("height"),
        None,
        None,
        None,
        1_700_000_060_000,
        0,
    );
    let mut qc = sample_commit_qc(&header);
    qc.view = 2;
    qc.aggregate.signers_bitmap = vec![0b0000_0010]; // single signer
    qc.aggregate.bls_aggregate_signature = vec![0x22; 48];
    let proof = nexus::CrossLaneTransferBuilder::new(header, Some(qc), None, settlement)
        .build()
        .expect("proof");
    let quorum = nexus::LaneRelayQuorumContext::new(5, 3).expect("quorum context");

    let err = proof
        .verify_with_quorum(quorum)
        .expect_err("quorum should fail");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::InsufficientQuorum { .. })
    ));
}

#[test]
fn lane_relay_quorum_accepts_exact_min_quorum() {
    let lane_id = LaneId::new(19);
    let dataspace_id = DataSpaceId::new(11);
    let settlement = LaneBlockCommitment {
        block_height: 9,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 12,
        total_xor_due_micro: 8,
        total_xor_after_haircut_micro: 7,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(9).expect("height"),
        None,
        None,
        None,
        1_700_000_065_000,
        0,
    );
    let mut qc = sample_commit_qc(&header);
    qc.aggregate.signers_bitmap = vec![0b0000_0111];
    qc.aggregate.bls_aggregate_signature = vec![0x66; 48];
    let proof = nexus::CrossLaneTransferBuilder::new(header, Some(qc), None, settlement)
        .build()
        .expect("proof");
    let quorum = nexus::LaneRelayQuorumContext::new(5, 3).expect("quorum context");

    proof
        .verify_with_quorum(quorum)
        .expect("exact min quorum should pass");
}

#[test]
fn lane_relay_quorum_rejects_signer_bitmap_length_mismatch() {
    let lane_id = LaneId::new(18);
    let dataspace_id = DataSpaceId::new(10);
    let settlement = LaneBlockCommitment {
        block_height: 8,
        lane_id,
        dataspace_id,
        tx_count: 1,
        total_local_micro: 11,
        total_xor_due_micro: 7,
        total_xor_after_haircut_micro: 6,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(8).expect("height"),
        None,
        None,
        None,
        1_700_000_070_000,
        0,
    );
    let mut qc = sample_commit_qc(&header);
    qc.aggregate.signers_bitmap = vec![0b0000_0011, 0b0000_0001];
    qc.aggregate.bls_aggregate_signature = vec![0x55; 48];
    let proof = nexus::CrossLaneTransferBuilder::new(header, Some(qc), None, settlement)
        .build()
        .expect("proof");
    let quorum = nexus::LaneRelayQuorumContext::new(4, 2).expect("quorum context");

    let err = proof
        .verify_with_quorum(quorum)
        .expect_err("bitmap length mismatch should fail");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SignerBitmapLengthMismatch {
            expected: 1,
            actual: 2
        })
    ));
}

#[test]
fn verify_lane_relay_envelopes_allows_distinct_lanes_on_same_height() {
    let first = sample_relay_envelope();
    let settlement = LaneBlockCommitment {
        block_height: first.block_height,
        lane_id: LaneId::new(first.lane_id.as_u32() + 1),
        dataspace_id: DataSpaceId::new(first.dataspace_id.as_u64() + 1),
        tx_count: 1,
        total_local_micro: 9,
        total_xor_due_micro: 5,
        total_xor_after_haircut_micro: 4,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(first.block_height).expect("height"),
        None,
        None,
        None,
        1_700_000_080_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect("valid second envelope")
        .envelope()
        .clone();

    nexus::verify_lane_relay_envelopes(&[first, second])
        .expect("distinct lane tuples must not be treated as duplicates");
}

#[test]
fn verify_lane_relay_envelopes_allows_distinct_lanes_on_same_dataspace_and_height() {
    let first = sample_relay_envelope();
    let settlement = LaneBlockCommitment {
        block_height: first.block_height,
        lane_id: LaneId::new(first.lane_id.as_u32() + 2),
        dataspace_id: first.dataspace_id,
        tx_count: 2,
        total_local_micro: 16,
        total_xor_due_micro: 9,
        total_xor_after_haircut_micro: 8,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(first.block_height).expect("height"),
        None,
        None,
        None,
        1_700_000_082_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect("valid second envelope")
        .envelope()
        .clone();

    nexus::verify_lane_relay_envelopes(&[first, second])
        .expect("distinct lanes on same dataspace/height must not be duplicates");
}

#[test]
fn verify_lane_relay_envelopes_allows_distinct_dataspaces_on_same_lane_and_height() {
    let first = sample_relay_envelope();
    let settlement = LaneBlockCommitment {
        block_height: first.block_height,
        lane_id: first.lane_id,
        dataspace_id: DataSpaceId::new(first.dataspace_id.as_u64() + 1),
        tx_count: 1,
        total_local_micro: 13,
        total_xor_due_micro: 8,
        total_xor_after_haircut_micro: 7,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(first.block_height).expect("height"),
        None,
        None,
        None,
        1_700_000_085_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect("valid second envelope")
        .envelope()
        .clone();

    nexus::verify_lane_relay_envelopes(&[first, second])
        .expect("distinct dataspaces on the same lane/height must not be duplicates");
}

#[test]
fn verify_lane_relay_envelopes_allows_same_lane_across_heights() {
    let first = sample_relay_envelope();
    let next_height = first.block_height + 1;
    let settlement = LaneBlockCommitment {
        block_height: next_height,
        lane_id: first.lane_id,
        dataspace_id: first.dataspace_id,
        tx_count: 3,
        total_local_micro: 14,
        total_xor_due_micro: 9,
        total_xor_after_haircut_micro: 8,
        total_xor_variance_micro: 1,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(next_height).expect("height"),
        None,
        None,
        None,
        1_700_000_090_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, None, settlement)
        .build()
        .expect("valid second envelope")
        .envelope()
        .clone();

    nexus::verify_lane_relay_envelopes(&[first, second])
        .expect("same lane should be accepted when block heights differ");
}

#[test]
fn verify_lane_relay_envelopes_reports_relay_error_before_duplicate_tuple_check() {
    let valid = sample_relay_envelope();
    let mut tampered_duplicate = valid.clone();
    tampered_duplicate.settlement_hash = HashOf::from_untyped_unchecked(Hash::new([0xDD; 4]));

    let err = nexus::verify_lane_relay_envelopes(&[valid, tampered_duplicate])
        .expect_err("invalid duplicate should fail relay verification first");
    assert!(matches!(
        err,
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementHashMismatch)
    ));
}

#[test]
fn dataspace_tamper_does_not_taint_valid_envelopes() -> Result<()> {
    let baseline = sample_relay_envelope();
    nexus::verify_lane_relay_envelopes(std::slice::from_ref(&baseline))
        .expect("baseline envelope should validate");

    let mut tampered = baseline.clone();
    tampered.settlement_commitment.dataspace_id = DataSpaceId::new(99);
    tampered.settlement_hash = compute_settlement_hash(&tampered.settlement_commitment)?;

    let err = nexus::verify_lane_relay_envelopes(std::slice::from_ref(&tampered))
        .expect_err("tampered dataspace should be rejected");
    nexus::verify_lane_relay_envelopes(std::slice::from_ref(&baseline))
        .expect("valid envelope should remain usable after tamper rejection");

    let mut summary = json::native::Map::new();
    summary.insert(
        "scenario".to_string(),
        json::native::Value::String("dataspace_tamper_isolation".to_string()),
    );
    summary.insert(
        "baseline_dataspace".to_string(),
        json::native::Value::Number(json::native::Number::from(
            baseline.settlement_commitment.dataspace_id.as_u64(),
        )),
    );
    summary.insert(
        "tampered_dataspace".to_string(),
        json::native::Value::Number(json::native::Number::from(
            tampered.settlement_commitment.dataspace_id.as_u64(),
        )),
    );
    summary.insert(
        "tampered_error".to_string(),
        json::native::Value::String(relay_error_code(&err).to_string()),
    );
    emit_adversarial_summary(
        "dataspace_tamper_isolation",
        &json::native::Value::Object(summary),
    )?;
    Ok(())
}

fn build_registry(
    manifest_dir: &Path,
    lane_id: LaneId,
    alias: &str,
    storage: LaneStorageProfile,
) -> Result<LaneManifestRegistry> {
    let lane_count = NonZeroU32::new(lane_id.as_u32() + 1).expect("lane count must be nonzero");
    let lane_catalog = LaneCatalog::new(
        lane_count,
        vec![LaneConfig {
            id: lane_id,
            alias: alias.to_string(),
            governance: Some("council".to_string()),
            storage,
            ..LaneConfig::default()
        }],
    )?;

    let mut governance_catalog = GovernanceCatalog::default();
    governance_catalog.modules.insert(
        "council".to_string(),
        GovernanceModule {
            module_type: Some("council".to_string()),
            params: BTreeMap::new(),
        },
    );

    let registry_cfg = LaneRegistry {
        manifest_directory: Some(manifest_dir.to_path_buf()),
        cache_directory: None,
        poll_interval: Duration::ZERO,
    };

    Ok(LaneManifestRegistry::from_config(
        &lane_catalog,
        &governance_catalog,
        &registry_cfg,
    ))
}

fn write_manifest(dir: &Path, alias: &str, include_privacy: bool) -> Result<()> {
    fs::create_dir_all(dir)?;
    let validators = vec![
        format!("{}@{}", ALICE_ID.signatory(), ALICE_ID.domain()),
        format!("{}@{}", BOB_ID.signatory(), BOB_ID.domain()),
    ];
    let mut manifest = norito::json::native::Map::new();
    manifest.insert("lane".into(), norito::json!(alias));
    manifest.insert("governance".into(), norito::json!("council"));
    manifest.insert("version".into(), norito::json!(1));
    manifest.insert("validators".into(), norito::json!(validators));
    manifest.insert("quorum".into(), norito::json!(1));
    manifest.insert(
        "protected_namespaces".into(),
        norito::json!(["confidential"]),
    );

    if include_privacy {
        manifest.insert(
            "privacy_commitments".into(),
            norito::json!([{
                "id": 1,
                "scheme": "merkle",
                "merkle": {
                    "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    "max_depth": 8
                }
            }]),
        );
    }
    let manifest = norito::json::native::Value::Object(manifest);

    let path = dir.join(format!("{alias}.manifest.json"));
    fs::write(&path, format!("{}\n", json::to_string_pretty(&manifest)?))?;
    Ok(())
}

fn sample_relay_envelope() -> LaneRelayEnvelope {
    let lane_id = LaneId::new(12);
    let dataspace_id = DataSpaceId::new(9);
    let settlement = LaneBlockCommitment {
        block_height: 4,
        lane_id,
        dataspace_id,
        tx_count: 2,
        total_local_micro: 75,
        total_xor_due_micro: 25,
        total_xor_after_haircut_micro: 20,
        total_xor_variance_micro: 5,
        swap_metadata: None,
        receipts: Vec::new(),
    };
    let mut header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(4).expect("height"),
        None,
        None,
        None,
        1_700_000_030_000,
        0,
    );
    let da_hash = HashOf::from_untyped_unchecked(Hash::new([0x22, 0x33, 0x44, 0x55]));
    header.set_da_commitments_hash(Some(da_hash));

    nexus::CrossLaneTransferBuilder::new(header, None, Some(da_hash), settlement)
        .build()
        .expect("valid envelope")
        .envelope()
        .clone()
}

fn relay_error_code(err: &nexus::CrossLaneProofError) -> &'static str {
    match err {
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementDataspaceMismatch) => {
            "settlement_dataspace_mismatch"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementBlockHeightMismatch) => {
            "settlement_block_height_mismatch"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::BlockHeightMismatch) => {
            "block_height_mismatch"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::InvalidSignerIndex { .. }) => {
            "invalid_signer_index"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::InsufficientQuorum { .. }) => {
            "insufficient_quorum"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::AggregateSignatureInvalid) => {
            "aggregate_signature_invalid"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::DaCommitmentHashMismatch) => {
            "da_commitment_hash_mismatch"
        }
        nexus::CrossLaneProofError::Relay(LaneRelayError::SettlementLaneMismatch) => {
            "settlement_lane_mismatch"
        }
        _ => "unexpected",
    }
}

fn emit_adversarial_summary(scenario: &str, summary: &norito::json::native::Value) -> Result<()> {
    let pretty = json::to_json_pretty(summary).wrap_err("serialize summary")?;
    println!("dataspace_adversarial::{scenario}::{pretty}");

    if let Ok(dir) = std::env::var("DATASPACE_ADVERSARIAL_ARTIFACT_DIR") {
        let root = PathBuf::from(dir);
        fs::create_dir_all(&root).wrap_err("create dataspace artifact dir")?;
        let path = root.join(format!("{scenario}.summary.json"));
        fs::write(path, format!("{pretty}\n")).wrap_err("write dataspace summary")?;
    }
    Ok(())
}
