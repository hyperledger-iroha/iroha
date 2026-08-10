// Exact-network and bounded-wire tests for provider VRF submissions.

#[test]
fn provider_vrf_submission_requires_exact_non_inert_ed25519_material() {
    let submission = provider_vrf_submission_fixture();
    assert!(submission.validate().is_ok());
    let exact = submission
        .encoded_len_exact()
        .expect("VRF submission exposes exact canonical length");
    assert_eq!(
        preflight_provider_vrf_submission_len(&submission, exact),
        Ok(exact)
    );
    assert_eq!(
        preflight_provider_vrf_submission_len(&submission, exact - 1),
        Err(ProviderVrfSubmissionValidationError::PayloadTooLarge {
            found: exact,
            maximum: exact - 1,
        })
    );
    let encoded = norito::to_bytes(&submission).expect("encode VRF submission");
    assert_eq!(
        decode_provider_vrf_submission_v1(&encoded).expect("bounded VRF decoder"),
        submission
    );
    assert!(
        decode_provider_vrf_submission_v1(&vec![
            0;
            PROVIDER_VRF_SUBMISSION_MAX_CANONICAL_BYTES_V1 + 1
        ])
        .is_err()
    );

    let mut inert_network = submission.clone();
    inert_network.network_id.fill(0);
    assert_eq!(
        inert_network.validate(),
        Err(ProviderVrfSubmissionValidationError::InvalidNetworkId)
    );

    let mut short_key = submission.clone();
    short_key.signature.public_key.pop();
    assert_eq!(
        short_key.validate(),
        Err(
            ProviderVrfSubmissionValidationError::InvalidSignaturePublicKeyLength {
                found: PUBLIC_KEY_LENGTH - 1,
                expected: PUBLIC_KEY_LENGTH,
            }
        )
    );

    let mut overlong_signature = submission.clone();
    overlong_signature.signature.signature.push(10);
    assert_eq!(
        overlong_signature.validate(),
        Err(
            ProviderVrfSubmissionValidationError::InvalidSignatureLength {
                found: SIGNATURE_LENGTH + 1,
                expected: SIGNATURE_LENGTH,
            }
        )
    );

    let mut inert = submission.clone();
    inert.signature.public_key.fill(0);
    assert_eq!(
        inert.validate(),
        Err(ProviderVrfSubmissionValidationError::InvalidSignature)
    );

    let mut reserved = submission;
    reserved.signature.algorithm = SignatureAlgorithm::MultiSig;
    assert_eq!(
        reserved.validate(),
        Err(ProviderVrfSubmissionValidationError::UnsupportedSignatureAlgorithm)
    );
}

#[test]
fn provider_vrf_submission_signature_binds_exact_network_id() {
    let signing_key = SigningKey::from_bytes(&[0x47; 32]);
    let mut submission = provider_vrf_submission_fixture();
    submission.signature.public_key = signing_key.verifying_key().to_bytes().to_vec();
    let payload = submission
        .signature_payload_bytes()
        .expect("encode provider VRF submission payload");
    submission.signature.signature = signing_key.sign(&payload).to_bytes().to_vec();

    submission
        .verify_signature_for_provider(&signing_key.verifying_key().to_bytes())
        .expect("exact-network provider signature");
    submission.network_id[0] ^= 1;
    assert!(matches!(
        submission.verify_signature_for_provider(&signing_key.verifying_key().to_bytes()),
        Err(PorSignatureVerificationError::Verification { .. })
    ));
}
