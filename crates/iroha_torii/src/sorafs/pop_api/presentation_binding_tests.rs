//! Mandatory presentation-recipient binding in both public membership request schemas.

use super::*;

#[derive(NoritoSerialize)]
struct MissingPresentationMembershipRequest {
    credential_commitment_hex: String,
    challenge_digest_hex: String,
    verifier_context: String,
}

#[derive(NoritoSerialize)]
struct MissingPresentationVerifyRequest {
    canonical_proof_base64url: String,
    challenge_digest_hex: String,
    verifier_context: String,
}

#[test]
fn membership_request_requires_presentation_binding_in_json_and_norito() {
    let unbound_json = format!(
        r#"{{"credential_commitment_hex":"{}","challenge_digest_hex":"{}","verifier_context":"jury"}}"#,
        "11".repeat(32),
        "22".repeat(32),
    );
    assert!(norito::json::from_json::<PopMembershipRequestV1>(&unbound_json).is_err());
    let unbound = MissingPresentationMembershipRequest {
        credential_commitment_hex: "11".repeat(32),
        challenge_digest_hex: "22".repeat(32),
        verifier_context: "jury".to_owned(),
    };
    // Advertise the actual request schema so its missing field, rather than
    // the deliberately incomplete fixture type's schema hash, causes rejection.
    let unbound_wire = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (payload, flags) = norito::codec::encode_with_header_flags(&unbound);
        norito::core::frame_bare_with_header_flags::<PopMembershipRequestV1>(&payload, flags)
            .expect("frame incomplete payload with actual membership request schema")
    };
    let error = norito::decode_from_bytes::<PopMembershipRequestV1>(&unbound_wire)
        .expect_err("current request schema must reject omitted presentation binding");
    assert!(!matches!(error, norito::Error::SchemaMismatch));
    assert!(norito::decode_canonical::<PopMembershipRequestV1>(&unbound_wire).is_err());

    let request = PopMembershipRequestV1 {
        credential_commitment_hex: "11".repeat(32),
        challenge_digest_hex: "22".repeat(32),
        verifier_context: "jury".to_owned(),
        presentation_binding_digest_hex: "33".repeat(32),
    };
    let json = norito::json::to_json(&request).expect("encode bound request JSON");
    let decoded = norito::json::from_json::<PopMembershipRequestV1>(&json)
        .expect("decode bound request JSON");
    assert_eq!(decoded.presentation_binding_digest_hex, "33".repeat(32));
    let wire = crate::frame_test_support::assert_current_frame(
        &request,
        "iroha_torii::sorafs::pop_api::PopMembershipRequestV1",
    );
    let decoded = norito::decode_from_bytes::<PopMembershipRequestV1>(&wire)
        .expect("decode bound request Norito");
    assert_eq!(decoded.presentation_binding_digest_hex, "33".repeat(32));
}

#[test]
fn verify_request_requires_presentation_binding_in_json_and_norito() {
    let unbound_json = format!(
        r#"{{"canonical_proof_base64url":"cHJvb2Y","challenge_digest_hex":"{}","verifier_context":"jury"}}"#,
        "22".repeat(32),
    );
    assert!(norito::json::from_json::<PopVerifyMembershipRequestV1>(&unbound_json).is_err());
    let unbound = MissingPresentationVerifyRequest {
        canonical_proof_base64url: "cHJvb2Y".to_owned(),
        challenge_digest_hex: "22".repeat(32),
        verifier_context: "jury".to_owned(),
    };
    let unbound_wire = {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (payload, flags) = norito::codec::encode_with_header_flags(&unbound);
        norito::core::frame_bare_with_header_flags::<PopVerifyMembershipRequestV1>(&payload, flags)
            .expect("frame incomplete payload with actual verification request schema")
    };
    let error = norito::decode_from_bytes::<PopVerifyMembershipRequestV1>(&unbound_wire)
        .expect_err("current request schema must reject omitted presentation binding");
    assert!(!matches!(error, norito::Error::SchemaMismatch));
    assert!(norito::decode_canonical::<PopVerifyMembershipRequestV1>(&unbound_wire).is_err());

    let request = PopVerifyMembershipRequestV1 {
        canonical_proof_base64url: "cHJvb2Y".to_owned(),
        challenge_digest_hex: "22".repeat(32),
        verifier_context: "jury".to_owned(),
        presentation_binding_digest_hex: "33".repeat(32),
    };
    let json = norito::json::to_json(&request).expect("encode bound request JSON");
    let decoded = norito::json::from_json::<PopVerifyMembershipRequestV1>(&json)
        .expect("decode bound request JSON");
    assert_eq!(decoded.presentation_binding_digest_hex, "33".repeat(32));
    let wire = crate::frame_test_support::assert_current_frame(
        &request,
        "iroha_torii::sorafs::pop_api::PopVerifyMembershipRequestV1",
    );
    let decoded = norito::decode_from_bytes::<PopVerifyMembershipRequestV1>(&wire)
        .expect("decode bound request Norito");
    assert_eq!(decoded.presentation_binding_digest_hex, "33".repeat(32));
}

#[test]
fn presentation_binding_rejects_zero_and_noncanonical_digests() {
    let field = "presentation_binding_digest_hex";
    assert_eq!(decode_hex_32(&"ab".repeat(32), field), Ok([0xab; 32]));
    for malformed in [
        String::new(),
        "00".repeat(32),
        "AB".repeat(32),
        "ab".repeat(31),
        "ab".repeat(33),
        format!(" {}", "ab".repeat(32)),
    ] {
        assert_eq!(
            decode_hex_32(&malformed, field),
            Err(PopCredentialServiceError::InvalidInput { field })
        );
    }
}
