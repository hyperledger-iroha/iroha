// Fixed V1 signing preimages and opaque cursor encodings under foreign codec contexts.

#[test]
fn por_signatures_are_canonical_across_caller_layouts() {
    let key = SigningKey::from_bytes(&[0x57; 32]);
    let public_key = key.verifying_key().to_bytes().to_vec();
    let canonical_flags = norito::core::default_encode_flags();
    let submission_payload = encode_frame_with_flags(
        &ProviderVrfSubmissionSigningPayloadV1::from(&provider_vrf_submission_fixture()),
        canonical_flags,
    );
    let proof_payload = encode_frame_with_flags(
        &PorProofSigningPayloadV1::from(&proof_fixture()),
        canonical_flags,
    );
    let verdict_payload = encode_frame_with_flags(
        &AuditVerdictSigningPayloadV1::from(&verdict_fixture()),
        canonical_flags,
    );
    for signing_flags in supported_layouts() {
        let _signing_context = norito::core::DecodeFlagsGuard::enter(signing_flags);
        let mut submission = provider_vrf_submission_fixture();
        submission.signature.public_key = public_key.clone();
        let payload = submission.signature_payload_bytes().unwrap();
        assert_eq!(payload, submission_payload);
        submission.signature.signature = key.sign(&payload).to_bytes().to_vec();
        assert_eq!(
            submission.signature.signature,
            key.sign(&submission_payload).to_bytes()
        );
        let mut proof = proof_fixture();
        assert_eq!(proof.signature_payload_bytes().unwrap(), proof_payload);
        sign_proof(&mut proof, &key);
        assert_eq!(
            proof.signature.signature,
            key.sign(&proof_payload).to_bytes()
        );
        let mut verdict = verdict_fixture();
        assert_eq!(verdict.signature_payload_bytes().unwrap(), verdict_payload);
        add_verdict_signature(&mut verdict, &key);
        assert_eq!(
            verdict.auditor_signatures[0].signature,
            key.sign(&verdict_payload).to_bytes()
        );
        for verifying_flags in supported_layouts() {
            let _verification_context = norito::core::DecodeFlagsGuard::enter(verifying_flags);
            submission
                .validate()
                .expect("valid signed submission shape");
            proof.validate().expect("valid signed proof shape");
            verdict.validate().expect("valid signed verdict shape");
            submission
                .verify_signature_for_provider(&public_key)
                .expect("canonical submission signature survives caller layout");
            proof
                .verify_signature_for_provider(&public_key)
                .expect("canonical proof signature survives caller layout");
            verdict
                .verify_signatures_with_policy(std::slice::from_ref(&public_key), 1)
                .expect("canonical governed auditor signature survives caller layout");
        }
    }
}

#[test]
fn por_noncanonical_signing_preimages_have_no_fallback() {
    let key = SigningKey::from_bytes(&[0x58; 32]);
    let public_key = key.verifying_key().to_bytes().to_vec();
    let mut submission = provider_vrf_submission_fixture();
    let mut proof = proof_fixture();
    let mut verdict = verdict_fixture();
    let submission_payload =
        encode_frame_with_flags(&ProviderVrfSubmissionSigningPayloadV1::from(&submission), 0);
    let proof_payload = encode_frame_with_flags(&PorProofSigningPayloadV1::from(&proof), 0);
    let verdict_payload = encode_frame_with_flags(&AuditVerdictSigningPayloadV1::from(&verdict), 0);
    assert_ne!(
        submission_payload,
        submission.signature_payload_bytes().unwrap()
    );
    assert_ne!(proof_payload, proof.signature_payload_bytes().unwrap());
    assert_ne!(verdict_payload, verdict.signature_payload_bytes().unwrap());
    submission.signature.public_key = public_key.clone();
    submission.signature.signature = key.sign(&submission_payload).to_bytes().to_vec();
    proof.signature.public_key = public_key.clone();
    proof.signature.signature = key.sign(&proof_payload).to_bytes().to_vec();
    verdict.auditor_signatures.push(AdvertSignature {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: public_key.clone(),
        signature: key.sign(&verdict_payload).to_bytes().to_vec(),
    });
    // These signatures are genuine and well formed, but authenticate the wrong wire preimage.
    verify_ed25519_signature(&submission.signature, &submission_payload).unwrap();
    verify_ed25519_signature(&proof.signature, &proof_payload).unwrap();
    verify_ed25519_signature(&verdict.auditor_signatures[0], &verdict_payload).unwrap();
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        submission.validate().unwrap();
        proof.validate().unwrap();
        verdict.validate().unwrap();
        assert!(matches!(
            submission.verify_signature_for_provider(&public_key),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
        assert!(matches!(
            proof.verify_signature_for_provider(&public_key),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
        assert!(matches!(
            verdict.verify_signatures_with_policy(std::slice::from_ref(&public_key), 1),
            Err(PorSignatureVerificationError::Verification { .. })
        ));
    }
}

#[test]
fn por_signing_preflight_bounds_are_canonical_across_caller_layouts() {
    let submission = provider_vrf_submission_fixture();
    let proof = proof_fixture();
    let verdict = verdict_fixture();
    let (submission_len, proof_len, verdict_len) = {
        let _canonical_context =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        (
            submission.encoded_len_exact().unwrap(),
            proof.encoded_len_exact().unwrap(),
            verdict.encoded_len_exact().unwrap(),
        )
    };
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            preflight_provider_vrf_submission_len(&submission, submission_len),
            Ok(submission_len)
        );
        assert_eq!(
            preflight_provider_vrf_submission_len(&submission, submission_len - 1),
            Err(ProviderVrfSubmissionValidationError::PayloadTooLarge {
                found: submission_len,
                maximum: submission_len - 1,
            })
        );
        assert_eq!(preflight_por_proof_len(&proof, proof_len), Ok(proof_len));
        assert_eq!(
            preflight_por_proof_len(&proof, proof_len - 1),
            Err(PorProofValidationError::PayloadTooLarge {
                found: proof_len,
                maximum: proof_len - 1,
            })
        );
        assert_eq!(
            preflight_audit_verdict_len(&verdict, verdict_len),
            Ok(verdict_len)
        );
        assert_eq!(
            preflight_audit_verdict_len(&verdict, verdict_len - 1),
            Err(AuditVerdictValidationError::PayloadTooLarge {
                found: verdict_len,
                maximum: verdict_len - 1,
            })
        );
    }
}

#[test]
fn por_status_cursor_is_unique_across_caller_layouts() {
    let cursor = PorStatusCursorV1 {
        version: POR_STATUS_CURSOR_VERSION_V1,
        snapshot_generation: 17,
        selection_digest: [1; 32],
        last_epoch_id: 42,
        last_issued_at: 1_700_000_000,
        last_challenge_id: [2; 32],
    };
    let canonical = encode_frame_with_flags(&cursor, norito::core::default_encode_flags());
    let opaque = URL_SAFE_NO_PAD.encode(&canonical);
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(cursor.encode_opaque().unwrap(), opaque);
        assert_eq!(PorStatusCursorV1::decode_opaque(&opaque).unwrap(), cursor);
    }
    let alternate = encode_frame_with_flags(&cursor, 0);
    assert_ne!(alternate, canonical);
    assert_eq!(
        norito::decode_from_bytes::<PorStatusCursorV1>(&alternate).unwrap(),
        cursor,
        "the alternate frame is well formed and carries the same boundary"
    );
    let alternate_opaque = URL_SAFE_NO_PAD.encode(&alternate);
    assert!(alternate.len() <= POR_STATUS_CURSOR_MAX_CANONICAL_BYTES_V1);
    assert!(alternate_opaque.len() <= POR_STATUS_CURSOR_MAX_ENCODED_BYTES_V1);
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            PorStatusCursorV1::decode_opaque(&alternate_opaque),
            Err(PorStatusCursorCodecError::Canonical(
                norito::core::Error::NonCanonicalEncoding.to_string()
            ))
        );
    }
}
