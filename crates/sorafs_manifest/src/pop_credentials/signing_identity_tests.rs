//! Capture PoP signing projections without changing their signature or proof contracts.

use super::*;
use crate::signing_identity_test_support::shapes_decodable;
use norito::json::Value;

pub(crate) fn record(rows: &mut Vec<Value>) {
    let fixture = tests::fixture();
    verify_pop_credential_signature_v1(&fixture.credential).expect("signed credential fixture");
    let mut unsigned = fixture.credential.clone();
    unsigned.issuer_signature.signature.clear();
    shapes_decodable(
        rows,
        "pop/credential/populated",
        || PopCredentialSigningViewV1::from_credential(&fixture.credential),
        unsigned,
    );

    let mut first_root = fixture.root.clone();
    first_root.previous_root_digest = None;
    let first_root = sign_pop_commitment_root_ed25519_v1(first_root, &tests::signing_key(0x55))
        .expect("sign root without predecessor");
    for (case, root) in [
        ("previous-some", &fixture.root),
        ("previous-none", &first_root),
    ] {
        verify_pop_commitment_root_signature_v1(root).expect("signed root fixture");
        let mut unsigned = root.clone();
        unsigned.publisher_signature.signature.clear();
        shapes_decodable(
            rows,
            &format!("pop/commitment-root/{case}"),
            || PopCommitmentRootSigningViewV1::from_root(root),
            unsigned,
        );
    }

    let mut populated = fixture.revocations.clone();
    populated.entries = vec![
        PopRevocationEntryV1 {
            nonce: tests::nonce(0x10),
            revoked_at_epoch: 120,
            reason: PopRevocationReasonV1::Rotated,
        },
        PopRevocationEntryV1 {
            nonce: tests::nonce(0x20),
            revoked_at_epoch: 121,
            reason: PopRevocationReasonV1::HolderRequested,
        },
    ];
    populated.revocation_root =
        pop_revocation_root_v1(&populated.entries).expect("revocation root");
    let populated = sign_pop_revocation_list_ed25519_v1(populated, &tests::signing_key(0x55))
        .expect("sign populated revocation fixture");
    for (case, revocations) in [("empty", &fixture.revocations), ("populated", &populated)] {
        verify_pop_revocation_list_signature_v1(revocations).expect("signed revocation fixture");
        let mut unsigned = revocations.clone();
        unsigned.publisher_signature.signature.clear();
        shapes_decodable(
            rows,
            &format!("pop/revocations/{case}"),
            || PopRevocationListSigningViewV1::from_revocations(revocations),
            unsigned,
        );
    }
}

#[test]
fn nested_signature_view_preserves_payload_without_a_frame_owner() {
    let signed = &tests::fixture().credential.issuer_signature;
    let mut owned = signed.clone();
    owned.signature.clear();
    let borrowed = PopSignatureSigningViewV1::from_signature(signed);
    crate::canonical_test_support::assert_same_payload(&borrowed, &owned);
}
