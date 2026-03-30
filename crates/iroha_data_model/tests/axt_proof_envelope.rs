//! Roundtrip coverage for AXT proof envelopes.

use iroha_data_model::nexus::{AxtProofEnvelope, DataSpaceId};

#[test]
fn axt_proof_envelope_roundtrip() {
    let dsid = DataSpaceId::new(77);
    let envelope = AxtProofEnvelope {
        dsid,
        manifest_root: [0xAB; 32],
        da_commitment: Some([0xCD; 32]),
        proof: vec![1, 2, 3, 4],
        fastpq_binding: None,
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
