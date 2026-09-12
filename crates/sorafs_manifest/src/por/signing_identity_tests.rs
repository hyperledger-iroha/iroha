//! Capture serialize-only PoR projections without adding owned or borrowed decoders.

use super::*;
use crate::signing_identity_test_support::shapes_encode_only;
use ed25519_dalek::SigningKey;
use norito::json::Value;

pub(crate) fn record(rows: &mut Vec<Value>) {
    let key = SigningKey::from_bytes(&[0x31; 32]);
    let mut proof = tests::proof_fixture();
    tests::sign_proof(&mut proof, &key);
    proof.validate().expect("populated proof fixture");
    proof.verify_signature().expect("signed proof fixture");
    shapes_encode_only(
        rows,
        "por/proof/populated",
        || PorProofSigningPayloadViewV1::from(&proof),
        PorProofSigningPayloadV1::from(&proof),
    );
    let mut success = tests::verdict_fixture();
    tests::add_verdict_signature(&mut success, &key);
    let mut failed = tests::verdict_fixture();
    failed.proof_digest = None;
    failed.outcome = AuditOutcomeV1::Failed;
    failed.failure_reason = Some("provider missed the challenge deadline".to_owned());
    failed.metadata = vec![CapacityMetadataEntry {
        key: "repair.ticket".to_owned(),
        value: "ticket-7".to_owned(),
    }];
    tests::add_verdict_signature(&mut failed, &key);
    for (case, verdict) in [("success", &success), ("failed", &failed)] {
        verdict.validate().expect("complete verdict fixture");
        verdict.verify_signatures().expect("signed verdict fixture");
        shapes_encode_only(
            rows,
            &format!("por/verdict/{case}"),
            || AuditVerdictSigningPayloadViewV1::from(verdict),
            AuditVerdictSigningPayloadV1::from(verdict),
        );
    }
}
