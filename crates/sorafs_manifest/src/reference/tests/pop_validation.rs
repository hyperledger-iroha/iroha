#[test]
fn validate_pop_payload_bytes_accepts_signed_publications() {
    let (credential, root, revocations) = signed_pop_material();
    let outcome = pop_outcome(PopKind::Credential, &credential, "pop-credential.to", 31);
    assert_success(&outcome);
    assert_context(
        &outcome,
        "credential_id_hex",
        hex::encode(credential.credential_id),
    );

    let outcome = pop_outcome(PopKind::CommitmentRoot, &root, "pop-root.to", 31);
    assert_success(&outcome);

    let outcome = pop_outcome(
        PopKind::RevocationList,
        &revocations,
        "pop-revocations.to",
        31,
    );
    assert_success(&outcome);

    let bundle = issued_pop_bundle();
    let outcome = pop_outcome(
        PopKind::IssuedCredentialBundle,
        &bundle,
        "pop-issued-bundle.to",
        31,
    );
    assert_success(&outcome);
    assert_context(
        &outcome,
        "revocation_list_version",
        bundle.revocation_list.list_version.to_string(),
    );
}

#[test]
fn validate_pop_payload_bytes_accepts_request_and_proof_shapes() {
    let outcome = pop_outcome(
        PopKind::EnrollmentRequest,
        &pop_enrollment(),
        "pop-enrollment.to",
        32,
    );
    assert_success(&outcome);

    let outcome = pop_outcome(
        PopKind::RenewalRequest,
        &pop_renewal(),
        "pop-renewal.to",
        32,
    );
    assert_success(&outcome);

    let proof = pop_membership_proof();
    let outcome = pop_outcome(PopKind::MembershipProof, &proof, "pop-proof.to", 32);
    assert_success(&outcome);
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
    assert_context(
        &outcome,
        "proof_bytes_len",
        proof.proof_bytes.len().to_string(),
    );
}

#[test]
fn validate_pop_payload_bytes_rejects_malformed_norito() {
    let outcome = validate_pop_payload_bytes(
        PopKind::Credential,
        b"not norito",
        "bad-pop-credential.to",
        33,
    );
    assert_failure(&outcome, "SFS-NORITO-001", CATEGORY_NORITO);
}

#[test]
fn validate_pop_payload_bytes_rejects_oversize_and_noncanonical_archives() {
    let oversized = vec![0u8; POP_REFERENCE_PAYLOAD_MAX_BYTES_V1 + 1];
    let outcome = validate_pop_payload_bytes(
        PopKind::EnrollmentRequest,
        &oversized,
        "oversized-pop-enrollment.to",
        33,
    );
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-NORITO-001", "{outcome:?}");

    let compressed = norito::to_compressed_bytes(
        &pop_enrollment(),
        Some(norito::CompressionConfig::default()),
    )
    .expect("encode noncanonical compressed PoP enrollment");
    let outcome = validate_pop_payload_bytes(
        PopKind::EnrollmentRequest,
        &compressed,
        "compressed-pop-enrollment.to",
        33,
    );
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code, "SFS-NORITO-001", "{outcome:?}");
    assert!(outcome.message.contains("canonical Norito"), "{outcome:?}");
}

#[test]
fn validate_pop_payload_bytes_rejects_signature_tampering() {
    let (mut credential, _, _) = signed_pop_material();
    // Keep the tampered value structurally canonical so this negative
    // reaches signature verification instead of being rejected earlier as
    // a malformed Pasta scalar.
    credential.credential_id = pop_scalar(0x99);
    let outcome = pop_outcome(PopKind::Credential, &credential, "bad-pop-signature.to", 34);
    assert_failure(&outcome, "SFS-SIG-009", CATEGORY_SIGNATURE);
}
