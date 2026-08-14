// Provider-policy, secret-redaction, and signed-state tamper regression tests.
//
// This file is included directly in `evidence_viewer::tests` so test paths and
// access to private fixture helpers remain unchanged.
#[test]
fn provider_policy_drift_is_checked_before_and_after_external_operations() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let request = |idempotency_key| EvidenceViewerChallengeRequestV1 {
        case_id: CASE_ID.to_owned(),
        round_id: ROUND_ID.to_owned(),
        quarantine_id: fixture.quarantine_id,
        viewer_account: JUROR_ACCOUNT.to_owned(),
        role: EvidenceViewerRoleV1::Juror,
        purpose: REVIEW_PURPOSE.to_owned(),
        idempotency_key,
        now_unix_ms: BASE_UNIX_MS,
    };
    fixture.webauthn.qualification.set_policy_digest([0xB1; 32]);
    assert_eq!(
        service
            .issue_challenge(request([0x51; 32]))
            .expect_err("pre-operation policy drift must fail closed"),
        EvidenceViewerErrorV1::RuntimeUnavailable
    );
    assert_eq!(
        fixture.webauthn.issue_call_count(),
        0,
        "provider operation must not run after failed preflight"
    );
    fixture.webauthn.qualification.set_policy_digest([0xA1; 32]);
    fixture
        .webauthn
        .qualification
        .drift_policy_after_next_operation([0xB2; 32]);
    assert_eq!(
        service
            .issue_challenge(request([0x52; 32]))
            .expect_err("post-operation policy drift must discard the result"),
        EvidenceViewerErrorV1::RuntimeUnavailable
    );
    assert_eq!(fixture.webauthn.issue_call_count(), 1);
    assert_eq!(
        service
            .audit_status()
            .expect("failed external operation must not mutate checkpoint state")
            .challenge_count,
        0
    );
}
#[test]
fn evidence_bytes_assertions_tokens_and_provider_diagnostics_are_debug_redacted() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let challenge = fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0x41; 32],
        BASE_UNIX_MS,
    );
    let challenge_token = challenge.challenge.expose().to_owned();
    let debug_request = EvidenceViewerSessionRequestV1 {
        case_id: CASE_ID.to_owned(),
        round_id: ROUND_ID.to_owned(),
        quarantine_id: fixture.quarantine_id,
        viewer_account: JUROR_ACCOUNT.to_owned(),
        role: EvidenceViewerRoleV1::Juror,
        purpose: REVIEW_PURPOSE.to_owned(),
        challenge: opaque(&challenge_token),
        webauthn_assertion: b"WEBAUTHN-ASSERTION-SECRET-MUST-NOT-LEAK".to_vec(),
        idempotency_key: [0x42; 32],
        now_unix_ms: BASE_UNIX_MS + 1,
    };
    let request_debug = format!("{debug_request:?}");
    assert!(!request_debug.contains(&challenge_token));
    assert!(!request_debug.contains("WEBAUTHN-ASSERTION-SECRET-MUST-NOT-LEAK"));
    assert!(request_debug.contains("<redacted>"));
    drop(debug_request);
    let issued = fixture
        .create_session(
            &service,
            &challenge_token,
            b"valid-webauthn-assertion",
            [0x43; 32],
            BASE_UNIX_MS + 2,
        )
        .expect("create session for range redaction");
    let outcome = service
        .read_range(
            issued.session.local_session.session_id,
            JUROR_ACCOUNT,
            &opaque(issued.grant.expose()),
            0,
            EVIDENCE_PAYLOAD.len() as u64,
            [0x44; 32],
            [0xC4; 32],
            BASE_UNIX_MS + 3,
        )
        .expect("read authenticated evidence range");
    assert_eq!(outcome.range.payload.as_slice(), EVIDENCE_PAYLOAD);
    let outcome_debug = format!("{outcome:?}");
    assert!(!outcome_debug.contains(std::str::from_utf8(EVIDENCE_PAYLOAD).expect("ASCII")));
    assert!(!outcome_debug.contains(outcome.rotated_grant.expose()));
    assert!(outcome_debug.contains("payload: \"<redacted>\""));
    let service_debug = format!("{service:?}");
    assert!(!service_debug.contains(MOCK_PROVIDER_SECRET));
    assert!(!service_debug.contains(std::str::from_utf8(EVIDENCE_PAYLOAD).expect("ASCII")));
    for error in [
        EvidenceViewerErrorV1::Forbidden,
        EvidenceViewerErrorV1::AuthenticationRejected,
        EvidenceViewerErrorV1::RuntimeUnavailable,
        EvidenceViewerErrorV1::CheckpointUnavailable,
    ] {
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains(MOCK_PROVIDER_SECRET));
        assert!(!rendered.contains(std::str::from_utf8(EVIDENCE_PAYLOAD).expect("ASCII")));
    }
}
#[test]
fn signed_checkpoint_envelope_rejects_non_receipt_state_tampering() {
    let key = SigningKey::from_bytes(&[0x43; 32]);
    let config = valid_config(key.verifying_key().to_bytes());
    let checkpoint = EvidenceViewerCheckpointV1::default();
    let envelope = signed_checkpoint_envelope(&key, &config, checkpoint);
    verify_checkpoint_envelope(&config, envelope.clone()).expect("signed checkpoint");
    let mut tampered_checkpoint = envelope.clone();
    tampered_checkpoint.checkpoint.version =
        EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1.saturating_add(1);
    assert_eq!(
        verify_checkpoint_envelope(&config, tampered_checkpoint),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut tampered_count = envelope.clone();
    tampered_count.checkpoint_anchor.receipt_count = 1;
    assert_eq!(
        verify_checkpoint_envelope(&config, tampered_count),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut tampered_head = envelope.clone();
    tampered_head.checkpoint_anchor.chain_head = Some(EvidenceViewerReceiptCursorV1 {
        sequence: 1,
        receipt_digest: [0xA1; 32],
    });
    assert_eq!(
        verify_checkpoint_envelope(&config, tampered_head),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut tampered_digest = envelope.clone();
    tampered_digest.checkpoint_anchor.checkpoint_digest[0] ^= 1;
    assert_eq!(
        verify_checkpoint_envelope(&config, tampered_digest),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut tampered_signature = envelope;
    tampered_signature.checkpoint_anchor.signature[0] ^= 1;
    assert_eq!(
        verify_checkpoint_envelope(&config, tampered_signature),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut substituted_handle =
        signed_checkpoint_envelope(&key, &config, EvidenceViewerCheckpointV1::default())
            .checkpoint_anchor;
    substituted_handle.signer_handle = "pkcs11:rotated-evidence-receipts".to_owned();
    assert_eq!(
        substituted_handle.verify(
            "pkcs11:rotated-evidence-receipts",
            config.receipt_signer_public_key,
        ),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint),
        "the signature must bind the embedded signer handle, not only the expected identity"
    );
}
#[test]
fn checkpoint_rejects_substituted_erasure_intent_operation_binding() {
    let key = SigningKey::from_bytes(&[0x44; 32]);
    let config = valid_config(key.verifying_key().to_bytes());
    let idempotency_key = [0x45; 32];
    let request_digest = [0x46; 32];
    let quarantine_id = [0x47; 16];
    let object_id = [0x48; 16];
    let evidence_digest = [0x49; 32];
    let intent = EvidenceViewerErasureIntentV1 {
        operation_id: erasure_operation_id(
            idempotency_key,
            request_digest,
            quarantine_id,
            object_id,
            evidence_digest,
        ),
        quarantine_id,
        object_id,
        evidence_digest,
        case_id: CASE_ID.to_owned(),
        round_id: ROUND_ID.to_owned(),
        actor_account: LEGAL_ACCOUNT.to_owned(),
        idempotency_key,
        request_digest,
        requested_at_unix_ms: BASE_UNIX_MS,
    };
    let mut checkpoint = EvidenceViewerCheckpointV1 {
        erasure_intents: vec![intent],
        ..EvidenceViewerCheckpointV1::default()
    };
    validate_checkpoint(&config, &checkpoint).expect("exact erasure intent");
    checkpoint.erasure_intents[0].operation_id[0] ^= 1;
    assert_eq!(
        validate_checkpoint(&config, &checkpoint),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
}
#[test]
fn signed_receipt_rejects_body_signature_and_chain_tampering() {
    let key = SigningKey::from_bytes(&[0x42; 32]);
    let receipt = signed_receipt(&key);
    receipt
        .verify(
            "pkcs11:prod-evidence-receipts",
            key.verifying_key().to_bytes(),
        )
        .expect("valid signed receipt");
    let mut tampered_body = receipt.clone();
    tampered_body.body.range_end = Some(2048);
    assert_eq!(
        tampered_body.verify(
            "pkcs11:prod-evidence-receipts",
            key.verifying_key().to_bytes()
        ),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut tampered_signature = receipt.clone();
    tampered_signature.signature[0] ^= 1;
    assert_eq!(
        tampered_signature.verify(
            "pkcs11:prod-evidence-receipts",
            key.verifying_key().to_bytes()
        ),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let config = valid_config(key.verifying_key().to_bytes());
    let mut second = signed_receipt(&key);
    second.body.sequence = 2;
    second.body.previous_receipt_digest = [0x99; 32];
    second.receipt_digest = receipt_body_digest(&second.body).expect("second digest");
    second.signature = key
        .sign(&receipt_signature_message(second.receipt_digest))
        .to_bytes();
    let checkpoint = EvidenceViewerCheckpointV1 {
        receipts: vec![receipt, second],
        ..EvidenceViewerCheckpointV1::default()
    };
    assert_eq!(
        validate_checkpoint(&config, &checkpoint),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
}
