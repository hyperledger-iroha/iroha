//! Roundtrip coverage for AXT proof envelopes.

use iroha_data_model::nexus::{AxtFastpqBinding, AxtProofEnvelope, DataSpaceId};

fn sample_fastpq_binding(dsid: DataSpaceId) -> AxtFastpqBinding {
    AxtFastpqBinding {
        parameter: "fastpq-lane-balanced".to_string(),
        source_dsid: dsid.as_u64(),
        source_dataspace: "proof-envelope-test".to_string(),
        source_receipt_id: format!("receipt-{}", dsid.as_u64()),
        source_tx_commitment: "aa".repeat(32),
        claim_type: "authorization".to_string(),
        claim_digest: "bb".repeat(32),
        witness_commitment: "cc".repeat(32),
        policy_commitment: "dd".repeat(32),
        verified_effect_type: "test_effect".to_string(),
        corridor: "proof-envelope-test".to_string(),
        verifier_id: "fastpq".to_string(),
        verifier_version: "v1".to_string(),
        target_dsids: vec![dsid.as_u64()],
        effect_binding: None,
    }
}

#[test]
fn axt_proof_envelope_roundtrip() {
    let dsid = DataSpaceId::new(77);
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root: [0xAB; 32],
        da_commitment: Some([0xCD; 32]),
        proof: vec![1, 2, 3, 4],
        fastpq_binding: Some(sample_fastpq_binding(dsid)),
        committed_amount: None,
        amount_commitment: None,
    };

    let encoded = norito::to_bytes(&envelope).expect("encode proof envelope");
    let decoded: AxtProofEnvelope =
        norito::decode_from_bytes(&encoded).expect("decode proof envelope");

    assert_eq!(decoded, envelope);
    assert_eq!(decoded.dsid, dsid);
    assert_eq!(decoded.manifest_root, [0xAB; 32]);
}
