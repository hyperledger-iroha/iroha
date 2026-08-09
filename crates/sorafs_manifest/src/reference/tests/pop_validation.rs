#[test]
fn validate_pop_payload_bytes_accepts_signed_publications() {
    let (credential, root, revocations) = signed_pop_material();

    let credential_bytes = to_bytes(&credential).expect("encode PoP credential");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::Credential,
        &credential_bytes,
        "pop-credential.to",
        31,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-OK-000");
    assert!(outcome.context.iter().any(|field| {
        field.key == "credential_id_hex" && field.value == hex::encode(credential.credential_id)
    }));

    let root_bytes = to_bytes(&root).expect("encode PoP commitment root");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::CommitmentRoot,
        &root_bytes,
        "pop-root.to",
        31,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-OK-000");

    let revocation_bytes = to_bytes(&revocations).expect("encode PoP revocations");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::RevocationList,
        &revocation_bytes,
        "pop-revocations.to",
        31,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-OK-000");

    let bundle = issued_pop_bundle();
    let bundle_bytes = to_bytes(&bundle).expect("encode PoP issued bundle");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::IssuedCredentialBundle,
        &bundle_bytes,
        "pop-issued-bundle.to",
        31,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-OK-000");
    assert!(outcome.context.iter().any(|field| {
        field.key == "revocation_list_version"
            && field.value == bundle.revocation_list.list_version.to_string()
    }));
}

#[test]
fn validate_pop_payload_bytes_accepts_request_and_proof_shapes() {
    let enrollment_bytes = to_bytes(&pop_enrollment()).expect("encode PoP enrollment");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::EnrollmentRequest,
        &enrollment_bytes,
        "pop-enrollment.to",
        32,
    );
    assert!(outcome.is_ok(), "{outcome:?}");

    let renewal_bytes = to_bytes(&pop_renewal()).expect("encode PoP renewal");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::RenewalRequest,
        &renewal_bytes,
        "pop-renewal.to",
        32,
    );
    assert!(outcome.is_ok(), "{outcome:?}");

    let proof = pop_membership_proof();
    let proof_bytes = to_bytes(&proof).expect("encode PoP proof");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::MembershipProof,
        &proof_bytes,
        "pop-proof.to",
        32,
    );
    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(
        outcome
            .telemetry_tags
            .contains(&"sorafs.reference.pop.membership_proof".to_owned())
    );
    for private_key in [
        "proof_id_hex",
        "credential_id_hex",
        "holder_commitment_hex",
        "revocation_nonce_hex",
        "proof_bytes",
    ] {
        assert!(
            outcome.context.iter().all(|field| field.key != private_key),
            "private proof context key leaked: {private_key}"
        );
    }
    assert!(outcome.context.iter().any(|field| {
        field.key == "proof_bytes_len" && field.value == proof.proof_bytes.len().to_string()
    }));
}

#[test]
fn validate_pop_payload_bytes_rejects_malformed_norito() {
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::Credential,
        b"not norito",
        "bad-pop-credential.to",
        33,
    );
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-NORITO-001");
    assert_eq!(outcome.category, CATEGORY_NORITO);
}

#[test]
fn validate_pop_payload_bytes_rejects_oversize_and_noncanonical_archives() {
    let oversized = vec![0u8; POP_REFERENCE_PAYLOAD_MAX_BYTES_V1 + 1];
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::EnrollmentRequest,
        &oversized,
        "oversized-pop-enrollment.to",
        33,
    );
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-NORITO-001");

    let compressed = norito::to_compressed_bytes(
        &pop_enrollment(),
        Some(norito::CompressionConfig::default()),
    )
    .expect("encode noncanonical compressed PoP enrollment");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::EnrollmentRequest,
        &compressed,
        "compressed-pop-enrollment.to",
        33,
    );
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-NORITO-001");
    assert!(outcome.message.contains("canonical Norito"), "{outcome:?}");
}

#[test]
fn validate_pop_payload_bytes_rejects_signature_tampering() {
    let (mut credential, _, _) = signed_pop_material();
    // Keep the tampered value structurally canonical so this negative
    // reaches signature verification instead of being rejected earlier as
    // a malformed Pasta scalar.
    credential.credential_id = pop_scalar(0x99);
    let bytes = to_bytes(&credential).expect("encode tampered credential");
    let outcome = validate_pop_payload_bytes(
        PopValidationPayloadKindV1::Credential,
        &bytes,
        "bad-pop-signature.to",
        34,
    );
    assert!(!outcome.is_ok());
    assert_eq!(outcome.code, "SFS-SIG-009", "{outcome:?}");
    assert_eq!(outcome.category, CATEGORY_SIGNATURE);
}
