//! Lane relay envelope regression tests.

use std::num::NonZeroU64;

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{
            CertPhase, LaneBlockCommitment, LaneSettlementReceipt, PERMISSIONED_TAG, Qc,
            QcAggregate,
        },
    },
    da::commitment,
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayError, compute_settlement_hash},
    peer::PeerId,
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

fn sample_qc(block_hash: HashOf<BlockHeader>) -> Qc {
    let validator_set = vec![
        PeerId::from(KeyPair::random().public_key().clone()),
        PeerId::from(KeyPair::random().public_key().clone()),
    ];
    Qc {
        phase: CertPhase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: Hash::prehashed([0x22; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0x11; Hash::LENGTH]),
        height: 5,
        view: 3,
        epoch: 1,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![0b1010_0001],
            bls_aggregate_signature: vec![0xAB; 48],
        },
    }
}

fn sample_settlement() -> LaneBlockCommitment {
    let receipt = LaneSettlementReceipt {
        source_id: [0xAA; 32],
        local_amount_micro: 10,
        xor_due_micro: 20,
        xor_after_haircut_micro: 18,
        xor_variance_micro: 2,
        timestamp_ms: 1_700_000_100,
    };

    LaneBlockCommitment {
        block_height: 5,
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(2),
        tx_count: 1,
        total_local_micro: receipt.local_amount_micro,
        total_xor_due_micro: receipt.xor_due_micro,
        total_xor_after_haircut_micro: receipt.xor_after_haircut_micro,
        total_xor_variance_micro: receipt.xor_variance_micro,
        swap_metadata: None,
        receipts: vec![receipt],
    }
}

#[test]
fn lane_relay_envelope_roundtrips_and_verifies_hash() {
    let da_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
        [0xCC; Hash::LENGTH],
    )));
    let header = sample_block_header(da_hash);
    let settlement = sample_settlement();
    let qc = sample_qc(header.hash());
    let manifest_root = Some([0x44; 32]);
    let envelope = LaneRelayEnvelope::new(header, Some(qc.clone()), da_hash, settlement.clone(), 0)
        .expect("construct envelope")
        .with_manifest_root(manifest_root);

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

    // QC mismatch should be rejected.
    let bad_qc = Qc {
        subject_block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xEE; 32])),
        ..qc
    };
    let err = LaneRelayEnvelope::new(header, Some(bad_qc), da_hash, settlement.clone(), 0)
        .expect_err("qc mismatch should fail");
    assert!(matches!(err, LaneRelayError::QcSubjectMismatch));

    // DA hash mismatch should be rejected.
    let err = LaneRelayEnvelope::new(header, None, None, settlement.clone(), 0)
        .expect_err("da mismatch should fail");
    assert!(matches!(err, LaneRelayError::DaCommitmentHashMismatch));
}

#[test]
fn lane_relay_envelope_rejects_settlement_height_mismatch() {
    let header = sample_block_header(None);
    let mut settlement = sample_settlement();
    settlement.block_height = header.height().get() + 1;

    let err = LaneRelayEnvelope::new(header, None, None, settlement, 0)
        .expect_err("mismatched settlement height should be rejected");
    assert_eq!(err, LaneRelayError::SettlementBlockHeightMismatch);
}

#[test]
fn lane_relay_envelope_rejects_qc_height_mismatch() {
    let header = sample_block_header(None);
    let settlement = sample_settlement();
    let mut qc = sample_qc(header.hash());
    qc.height = header.height().get() + 1;

    let err = LaneRelayEnvelope::new(header, Some(qc), None, settlement.clone(), 0)
        .expect_err("mismatched execution QC height should be rejected");
    assert_eq!(err, LaneRelayError::QcHeightMismatch);

    let mut envelope =
        LaneRelayEnvelope::new(header, Some(sample_qc(header.hash())), None, settlement, 0)
            .expect("construct envelope");
    let qc_ref = envelope.qc.as_mut().expect("qc present in envelope");
    qc_ref.height += 1;
    assert_eq!(
        envelope.verify().unwrap_err(),
        LaneRelayError::QcHeightMismatch
    );
}

#[test]
fn lane_relay_envelope_detects_tampering_on_verify() {
    let da_hash = Some(HashOf::from_untyped_unchecked(Hash::prehashed(
        [0xAA; Hash::LENGTH],
    )));
    let header = sample_block_header(da_hash);
    let settlement = sample_settlement();
    let qc = sample_qc(header.hash());
    let envelope = LaneRelayEnvelope::new(header, Some(qc), da_hash, settlement, 2048)
        .expect("construct envelope");

    // Block height tamper: header height diverges from stored height.
    let mut tampered = envelope.clone();
    tampered
        .block_header
        .set_height(NonZeroU64::new(tampered.block_height + 1).unwrap());
    assert_eq!(
        tampered.verify().unwrap_err(),
        LaneRelayError::BlockHeightMismatch
    );

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
