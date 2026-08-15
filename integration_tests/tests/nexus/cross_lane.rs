#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Cross-lane manifest and relay proof tests (NX-11).
use eyre::{Result, WrapErr};
use iroha::nexus;
use iroha_config::parameters::actual::{GovernanceCatalog, GovernanceModule, LaneRegistry};
use iroha_core::governance::manifest::{GovernanceGuardReason, LaneManifestRegistry};
use iroha_crypto::{Hash, HashOf, LaneCommitmentId, MerkleProof};
use iroha_data_model::{
    block::{consensus::LaneBlockCommitment, consensus_v2::finality::V2FinalityArtifact},
    nexus::{
        DataSpaceId, LaneCatalog, LaneConfig, LaneFinalityAuthorityV1, LaneId, LanePrivacyProof,
        LaneRelayEnvelope, LaneRelayError, LaneStorageProfile, compute_settlement_hash,
    },
    peer::PeerId,
    proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use norito::{core as norito_core, json};
use std::{
    collections::BTreeMap,
    fs,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::tempdir;
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
    let mut attachment = ProofAttachment::new_ref(
        "lane/privacy".parse()?,
        ProofBox::new("lane/privacy".parse()?, vec![0x01, 0x02]),
        VerifyingKeyId::new("lane/privacy", "lane_privacy_vk"),
    );
    attachment.lane_privacy = Some(privacy);
    let list = ProofAttachmentList::try_from(vec![attachment])
        .expect("one attachment is a valid bounded proof list");
    let norito_bytes = norito::to_bytes(&list)?;
    let archived = norito::from_bytes::<ProofAttachmentList>(&norito_bytes)?;
    let decoded: ProofAttachmentList = norito_core::NoritoDeserialize::deserialize(archived);
    assert_eq!(decoded, list);
    let decoded_privacy = decoded
        .as_slice()
        .first()
        .and_then(|entry| entry.lane_privacy.clone())
        .expect("lane privacy attachment present");
    assert_eq!(decoded_privacy.commitment_id, LaneCommitmentId::new(9));
    Ok(())
}
#[test]
fn lane_relay_envelope_must_have_consistent_finality_authority_reference() {
    let mut envelope = sample_relay_envelope();
    let global_block_height = envelope.block_header.height().get();
    envelope = envelope.with_finality_authority(Some(LaneFinalityAuthorityV1 {
        version: 1,
        global_block_height,
        finality_artifact_hash: HashOf::<V2FinalityArtifact>::from_untyped_unchecked(Hash::new(
            b"cross-lane-finality-artifact",
        )),
        statement_proof: MerkleProof::from_audit_path(0, Vec::new()),
    }));
    envelope
        .validate_finality_authority_ref()
        .expect("matching compact finality authority reference");
    envelope
        .finality_authority
        .as_mut()
        .expect("fixture carries finality authority")
        .global_block_height += 1;
    assert!(matches!(
        envelope.validate_finality_authority_ref(),
        Err(LaneRelayError::FinalityAuthorityHeightMismatch)
    ));
    let authority = envelope
        .finality_authority
        .as_mut()
        .expect("fixture carries finality authority");
    authority.global_block_height -= 1;
    authority.version = 2;
    assert!(matches!(
        envelope.validate_finality_authority_ref(),
        Err(LaneRelayError::UnsupportedFinalityAuthorityVersion(2))
    ));
}
#[test]
#[allow(clippy::unnecessary_wraps)]
fn cross_lane_builder_accepts_independent_lane_local_settlement_height() -> Result<()> {
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 1,
        total_local_amount: "0.00005".parse().expect("valid settlement quantity"),
        total_xor_due: "0.00003".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.000028".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000002".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let proof = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
        .build()
        .expect("lane-local settlement height may differ from global proposal height");
    assert_eq!(proof.envelope().block_height, 9);
    assert_eq!(proof.envelope().block_header.height().get(), 10);
    proof
        .verify()
        .expect("independent lane-local and global heights should verify");
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 2,
        total_local_amount: "0.00006".parse().expect("valid settlement quantity"),
        total_xor_due: "0.00004".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.000036".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000004".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
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
    let err = nexus::CrossLaneTransferBuilder::new(header, mismatched_da_hash, settlement)
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(12).expect("height"),
        None,
        None,
        None,
        1_700_000_010_000,
        0,
    );
    let proof = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 1,
        total_local_amount: "0.00005".parse().expect("valid settlement quantity"),
        total_xor_due: "0.000025".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.00002".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000005".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(3).expect("height"),
        None,
        None,
        None,
        1_700_000_020_000,
        0,
    );
    let proof = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
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
fn lane_relay_envelope_rejects_zero_lane_local_height() {
    let mut envelope = sample_relay_envelope();
    envelope.block_height = 0;
    let err = nexus::verify_lane_relay_envelopes(&[envelope]).expect_err("zero lane-local height");
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
fn verify_lane_relay_envelopes_allows_distinct_lanes_on_same_height() {
    let first = sample_relay_envelope();
    let settlement = LaneBlockCommitment {
        block_height: first.block_height,
        lane_id: LaneId::new(first.lane_id.as_u32() + 1),
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: DataSpaceId::new(first.dataspace_id.as_u64() + 1),
        tx_count: 1,
        total_local_amount: "0.000009".parse().expect("valid settlement quantity"),
        total_xor_due: "0.000005".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.000004".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(first.block_height).expect("height"),
        None,
        None,
        None,
        1_700_000_080_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: first.dataspace_id,
        tx_count: 2,
        total_local_amount: "0.000016".parse().expect("valid settlement quantity"),
        total_xor_due: "0.000009".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.000008".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(first.block_height).expect("height"),
        None,
        None,
        None,
        1_700_000_082_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: DataSpaceId::new(first.dataspace_id.as_u64() + 1),
        tx_count: 1,
        total_local_amount: "0.000013".parse().expect("valid settlement quantity"),
        total_xor_due: "0.000008".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.000007".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(first.block_height).expect("height"),
        None,
        None,
        None,
        1_700_000_085_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: first.dataspace_id,
        tx_count: 3,
        total_local_amount: "0.000014".parse().expect("valid settlement quantity"),
        total_xor_due: "0.000009".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.000008".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(next_height).expect("height"),
        None,
        None,
        None,
        1_700_000_090_000,
        0,
    );
    let second = nexus::CrossLaneTransferBuilder::new(header, None, settlement)
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
    let alice_peer = PeerId::from(ALICE_ID.expect_single_signatory().clone()).to_string();
    let bob_peer = PeerId::from(BOB_ID.expect_single_signatory().clone()).to_string();
    let mut alice_binding = norito::json::native::Map::new();
    alice_binding.insert("validator".into(), ALICE_ID.to_string().into());
    alice_binding.insert("peer_id".into(), alice_peer.into());
    let mut bob_binding = norito::json::native::Map::new();
    bob_binding.insert("validator".into(), BOB_ID.to_string().into());
    bob_binding.insert("peer_id".into(), bob_peer.into());
    let mut manifest = norito::json::native::Map::new();
    manifest.insert("lane".into(), norito::json!(alias));
    manifest.insert("governance".into(), norito::json!("council"));
    manifest.insert("version".into(), norito::json!(1));
    manifest.insert(
        "validators".into(),
        norito::json::native::Value::Array(vec![
            norito::json::native::Value::Object(alice_binding),
            norito::json::native::Value::Object(bob_binding),
        ]),
    );
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
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id,
        tx_count: 2,
        total_local_amount: "0.000075".parse().expect("valid settlement quantity"),
        total_xor_due: "0.000025".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0.00002".parse().expect("valid settlement quantity"),
        total_xor_variance: "0.000005".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
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
    nexus::CrossLaneTransferBuilder::new(header, Some(da_hash), settlement)
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
