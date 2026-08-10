//! Lane relay envelope regression tests.

use std::num::NonZeroU64;

use iroha_crypto::{Hash, HashOf, MerkleProof};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        consensus_v2::finality::V2FinalityArtifact,
    },
    da::commitment,
    nexus::{
        DataSpaceId, LaneFinalityAuthorityV1, LaneId, LaneRelayEnvelope, LaneRelayError,
        compute_settlement_hash,
    },
};
use norito::core::NoritoDeserialize;

fn sample_block_header(da_hash: Option<HashOf<commitment::DaCommitmentBundle>>) -> BlockHeader {
    let mut header = BlockHeader::new(
        NonZeroU64::new(5).expect("non-zero height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    );
    header.set_da_commitments_hash(da_hash);
    header
}

fn sample_settlement() -> LaneBlockCommitment {
    let receipt = LaneSettlementReceipt {
        source_id: [0xAA; 32],
        local_amount: "0.00001".parse().expect("valid settlement quantity"),
        xor_due: "0.00002".parse().expect("valid settlement quantity"),
        xor_after_haircut: "0.000018".parse().expect("valid settlement quantity"),
        xor_variance: "0.000002".parse().expect("valid settlement quantity"),
        timestamp_ms: 1_700_000_100,
    };

    LaneBlockCommitment {
        block_height: 5,
        lane_id: LaneId::new(1),
        lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: DataSpaceId::new(2),
        tx_count: 1,
        total_local_amount: receipt.local_amount.clone(),
        total_xor_due: receipt.xor_due.clone(),
        total_xor_after_haircut: receipt.xor_after_haircut.clone(),
        total_xor_variance: receipt.xor_variance.clone(),
        swap_metadata: None,
        receipts: vec![receipt],
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    }
}

#[test]
fn lane_relay_envelope_roundtrips_and_verifies_hash() {
    let da_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
        [0xCC; Hash::LENGTH],
    )));
    let header = sample_block_header(da_hash);
    let settlement = sample_settlement();
    let manifest_root = Some([0x44; 32]);
    let envelope = LaneRelayEnvelope::new(header, da_hash, settlement.clone(), 0)
        .expect("construct envelope")
        .with_manifest_root(manifest_root)
        .with_lane_block_descriptor_hash(Some(Hash::new(b"roundtrip-lane-descriptor")))
        .with_finality_authority(Some(LaneFinalityAuthorityV1 {
            version: 1,
            global_block_height: header.height().get(),
            finality_artifact_hash: HashOf::<V2FinalityArtifact>::from_untyped_unchecked(
                Hash::new(b"roundtrip-finality-artifact"),
            ),
            statement_proof: MerkleProof::from_audit_path(0, Vec::new()),
        }));

    let bytes = norito::to_bytes(&envelope).expect("encode envelope");
    let archived = norito::from_bytes::<LaneRelayEnvelope>(&bytes).expect("archive envelope");
    let decoded: LaneRelayEnvelope =
        NoritoDeserialize::try_deserialize(archived).expect("deserialize envelope");
    assert_eq!(envelope, decoded);
    assert_eq!(header.hash(), decoded.block_header.hash());
    assert_eq!(settlement, decoded.settlement_commitment);
    assert_eq!(
        compute_settlement_hash(&settlement).expect("hash"),
        decoded.settlement_hash
    );
    assert_eq!(manifest_root, decoded.manifest_root);
    decoded.verify().expect("envelope should verify");

    // DA hash mismatch should be rejected.
    let err = LaneRelayEnvelope::new(header, None, settlement.clone(), 0)
        .expect_err("da mismatch should fail");
    assert!(matches!(err, LaneRelayError::DaCommitmentHashMismatch));
}

#[test]
fn lane_relay_envelope_distinguishes_lane_local_and_global_heights() {
    let header = sample_block_header(None);
    let mut settlement = sample_settlement();
    settlement.block_height = 1;
    assert_ne!(settlement.block_height, header.height().get());

    let envelope = LaneRelayEnvelope::new(header, None, settlement, 0)
        .expect("lane-local settlement height may differ from global proposal height");
    assert_eq!(envelope.block_height, 1);
    assert_eq!(envelope.block_header.height().get(), header.height().get());
    envelope.verify().expect("separate height domains verify");
}

#[test]
fn lane_relay_envelope_rejects_finality_authority_height_mismatch() {
    let header = sample_block_header(None);
    let settlement = sample_settlement();
    let mut envelope = LaneRelayEnvelope::new(header, None, settlement, 0)
        .expect("construct envelope")
        .with_finality_authority(Some(LaneFinalityAuthorityV1 {
            version: 1,
            global_block_height: header.height().get() + 1,
            finality_artifact_hash: HashOf::<V2FinalityArtifact>::from_untyped_unchecked(
                Hash::new(b"wrong-height-finality-artifact"),
            ),
            statement_proof: MerkleProof::from_audit_path(0, Vec::new()),
        }));
    assert_eq!(
        envelope.validate_finality_authority_ref().unwrap_err(),
        LaneRelayError::FinalityAuthorityHeightMismatch
    );
    envelope
        .finality_authority
        .as_mut()
        .expect("authority")
        .version = 2;
    assert_eq!(
        envelope.validate_finality_authority_ref().unwrap_err(),
        LaneRelayError::UnsupportedFinalityAuthorityVersion(2)
    );
}

#[test]
fn lane_relay_envelope_detects_tampering_on_verify() {
    let da_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
        [0xAA; Hash::LENGTH],
    )));
    let header = sample_block_header(da_hash);
    let settlement = sample_settlement();
    let envelope =
        LaneRelayEnvelope::new(header, da_hash, settlement, 2048).expect("construct envelope");

    // Settlement height tamper.
    let mut tampered = envelope.clone();
    tampered.settlement_commitment.block_height = tampered.block_height + 2;
    assert_eq!(
        tampered.verify().unwrap_err(),
        LaneRelayError::SettlementBlockHeightMismatch
    );

    // Lane/dataspace tamper.
    let mut tampered = envelope.clone();
    tampered.lane_id = LaneId::new(tampered.lane_id.as_u32() + 1);
    assert_eq!(
        tampered.verify().unwrap_err(),
        LaneRelayError::SettlementLaneMismatch
    );

    let mut tampered = envelope.clone();
    tampered.dataspace_id = DataSpaceId::new(tampered.dataspace_id.as_u64() + 1);
    assert_eq!(
        tampered.verify().unwrap_err(),
        LaneRelayError::SettlementDataspaceMismatch
    );

    // Settlement payload tamper (hash mismatch).
    let mut tampered = envelope;
    tampered.settlement_commitment.tx_count += 1;
    assert_eq!(
        tampered.verify().unwrap_err(),
        LaneRelayError::SettlementHashMismatch
    );
}
