#[test]
fn config_rejects_noncanonical_webauthn_rp_ids_and_origins() {
    let key = SigningKey::from_bytes(&[0x41; 32]);
    for rp_id in ["Review.example", "localhost", "127.0.0.1"] {
        let mut config = valid_config(key.verifying_key().to_bytes());
        config.webauthn_rp_id = rp_id.to_owned();
        assert_eq!(
            config.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig),
            "{rp_id:?} must fail closed"
        );
    }

    for origin in [
        "http://review.example",
        "https://operator:secret@review.example",
        "https://review.example/path",
        "https://review.example?challenge=1",
        "https://review.example#fragment",
        "https://review.example:443",
        "https://foreign.example",
    ] {
        let mut config = valid_config(key.verifying_key().to_bytes());
        config.webauthn_allowed_origins = vec![origin.to_owned()];
        assert_eq!(
            config.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig),
            "{origin:?} must fail closed"
        );
    }

    let mut canonical = valid_config(key.verifying_key().to_bytes());
    canonical.webauthn_allowed_origins = vec!["https://login.review.example:8443".to_owned()];
    canonical
        .validate()
        .expect("canonical non-default origin port");
}

#[test]
fn case_bound_webauthn_session_rotates_grants_reauthorizes_and_survives_restart() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    assert!(
        !format!("{service:?}").contains(MOCK_PROVIDER_SECRET),
        "runtime provider Debug output must remain outside service logs"
    );
    let challenge = fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0x01; 32],
        BASE_UNIX_MS,
    );
    assert_eq!(
        challenge.expires_at_unix_ms,
        BASE_UNIX_MS + fixture.config.challenge_ttl_ms
    );
    let challenge_token = challenge.challenge.expose().to_owned();
    let issued = fixture
        .create_session(
            &service,
            &challenge_token,
            b"valid-webauthn-assertion",
            [0x02; 32],
            BASE_UNIX_MS + 1,
        )
        .expect("create case-bound session");
    assert_eq!(issued.session.case_id, CASE_ID);
    assert_eq!(issued.session.round_id, ROUND_ID);
    assert_eq!(issued.session.role, EvidenceViewerRoleV1::Juror);
    assert_eq!(
        issued.session.local_session.expires_at_unix_ms,
        BASE_UNIX_MS + 1 + EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1
    );
    assert_eq!(
        issued.receipt.body.kind,
        EvidenceViewerReceiptKindV1::SessionIssued
    );
    let session_id = issued.session.local_session.session_id;
    let initial_grant = issued.grant.expose().to_owned();

    let first_manifest = service
        .manifest(
            session_id,
            JUROR_ACCOUNT,
            &opaque(&initial_grant),
            [0x03; 32],
            [0xA3; 32],
            BASE_UNIX_MS + 2,
        )
        .expect("read first manifest");
    assert_eq!(first_manifest.manifest.case_id, CASE_ID);
    assert_eq!(first_manifest.manifest.round_id, ROUND_ID);
    assert_eq!(first_manifest.manifest.quarantine_id, fixture.quarantine_id);
    assert_eq!(first_manifest.manifest.object_id, fixture.object.object_id);
    assert_eq!(
        first_manifest.manifest.evidence_digest,
        fixture.object.payload_digest
    );
    assert_eq!(
        first_manifest.manifest.payload_len,
        EVIDENCE_PAYLOAD.len() as u64
    );
    assert!(
        first_manifest
            .manifest
            .visible_watermark
            .starts_with("CONFIDENTIAL · juror · ")
    );
    assert_eq!(
        first_manifest.receipt.body.kind,
        EvidenceViewerReceiptKindV1::ManifestAccessed
    );
    let second_grant = first_manifest.rotated_grant.expose().to_owned();
    assert_ne!(initial_grant, second_grant);
    assert!(fixture.grants.was_revoked(&initial_grant));
    assert_eq!(
        service
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&initial_grant),
                [0x04; 32],
                [0xA4; 32],
                BASE_UNIX_MS + 3,
            )
            .expect_err("rotated grant must be single use"),
        EvidenceViewerErrorV1::AuthenticationRejected
    );

    drop(service);
    let restarted = fixture.open();
    let second_manifest = restarted
        .manifest(
            session_id,
            JUROR_ACCOUNT,
            &opaque(&second_grant),
            [0x05; 32],
            [0xA5; 32],
            BASE_UNIX_MS + 4,
        )
        .expect("continue rotating grant after restart");
    let third_grant = second_manifest.rotated_grant.expose().to_owned();
    assert!(fixture.grants.was_revoked(&second_grant));
    let receipts = restarted.receipts(None, 16).expect("read receipt chain");
    assert_eq!(receipts.len(), 3);
    assert_receipt_chain(&receipts, &fixture.config);

    fixture
        .authorization
        .set_allowed(JUROR_ACCOUNT, EvidenceViewerRoleV1::Juror, false);
    assert_eq!(
        restarted
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&third_grant),
                [0x06; 32],
                [0xA6; 32],
                BASE_UNIX_MS + 5,
            )
            .expect_err("revoked finalized assignment must fail"),
        EvidenceViewerErrorV1::Forbidden
    );
    fixture
        .authorization
        .set_allowed(JUROR_ACCOUNT, EvidenceViewerRoleV1::Juror, true);
    fixture.authorization.set_policy_digest([0x93; 32]);
    assert_eq!(
        restarted
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&third_grant),
                [0x07; 32],
                [0xA7; 32],
                BASE_UNIX_MS + 6,
            )
            .expect_err("policy substitution must fail"),
        EvidenceViewerErrorV1::Forbidden
    );
    fixture.authorization.set_policy_digest([0x91; 32]);
    let third_manifest = restarted
        .manifest(
            session_id,
            JUROR_ACCOUNT,
            &opaque(&third_grant),
            [0x08; 32],
            [0xA8; 32],
            BASE_UNIX_MS + 7,
        )
        .expect("failed reauthorization must not consume the active grant");
    let fourth_grant = third_manifest.rotated_grant.expose().to_owned();
    assert_eq!(
        restarted
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&fourth_grant),
                [0x09; 32],
                [0xA9; 32],
                issued.session.local_session.expires_at_unix_ms,
            )
            .expect_err("exact expiry boundary must fail"),
        EvidenceViewerErrorV1::SessionInactive,
        "the exact fifteen-minute boundary must be expired"
    );
}

#[test]
fn signed_transparency_projection_is_authoritative_payload_free_and_restart_stable() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let challenge = fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0x81; 32],
        BASE_UNIX_MS,
    );
    let challenge_token = challenge.challenge.expose().to_owned();
    let issued = fixture
        .create_session(
            &service,
            &challenge_token,
            b"valid-webauthn-assertion-projection",
            [0x82; 32],
            BASE_UNIX_MS + 1,
        )
        .expect("create projected session");
    let initial_grant = issued.grant.expose().to_owned();
    let (rotated_grant, _) = service
        .record_interaction(
            issued.session.local_session.session_id,
            JUROR_ACCOUNT,
            &opaque(&initial_grant),
            ModerationEvidenceViewerAccessKind::Viewed,
            Some([0x83; 32]),
            [0x84; 32],
            [0x85; 32],
            BASE_UNIX_MS + 2,
        )
        .expect("record signed interaction");
    let rotated_grant = rotated_grant.expose().to_owned();

    let legacy = fixture
        .node
        .export_moderation_evidence_viewer_snapshot()
        .expect("export legacy evidence viewer snapshot");
    assert!(
        legacy.sessions.is_empty() && legacy.access_events.is_empty(),
        "the production viewer must not populate the competing local registry"
    );

    let checkpoint_digest = current_checkpoint_digest(&service);
    let signer_calls_before_reads = fixture.signer.sign_call_count();
    let first_page = service
        .transparency_projection(checkpoint_digest, None, 1)
        .expect("read first signed projection page");
    assert_eq!(first_page.receipts.len(), 1);
    assert!(first_page.has_more);
    assert_eq!(
        first_page.receipts[0].body.kind,
        EvidenceViewerReceiptKindV1::SessionIssued
    );
    first_page
        .verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        )
        .expect("verify first transparency page");
    let cursor = first_page.next_cursor.expect("first exact cursor");
    let second_page = service
        .transparency_projection(checkpoint_digest, Some(cursor), 16)
        .expect("read second signed projection page");
    assert_eq!(second_page.receipts.len(), 1);
    assert!(!second_page.has_more);
    assert_eq!(
        second_page.receipts[0].body.kind,
        EvidenceViewerReceiptKindV1::InteractionRecorded
    );
    second_page
        .verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        )
        .expect("verify second transparency page");
    let full_projection = service
        .transparency_projection(checkpoint_digest, None, 16)
        .expect("read complete signed projection");
    assert_eq!(full_projection.receipts.len(), 2);
    assert_receipt_chain(&full_projection.receipts, &fixture.config);
    full_projection
        .verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        )
        .expect("verify complete transparency page");

    let mut substituted_cursor = cursor;
    substituted_cursor.receipt_digest[0] ^= 1;
    assert_eq!(
        service
            .transparency_projection(checkpoint_digest, Some(substituted_cursor), 16)
            .expect_err("same-sequence digest substitution must fail"),
        EvidenceViewerErrorV1::InvalidRequest
    );
    assert_eq!(
        fixture.signer.sign_call_count(),
        signer_calls_before_reads,
        "audit reads must return the retained signed anchor without invoking the signer"
    );

    let encoded =
        norito::to_bytes(&full_projection).expect("encode payload-free transparency page");
    for secret in [
        std::str::from_utf8(EVIDENCE_PAYLOAD).expect("ASCII evidence fixture"),
        "valid-webauthn-assertion-projection",
        challenge_token.as_str(),
        initial_grant.as_str(),
        rotated_grant.as_str(),
        JUROR_ACCOUNT,
        MOCK_PROVIDER_SECRET,
    ] {
        assert!(
            !encoded
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "payload-free projection leaked forbidden material"
        );
    }

    drop(service);
    let restarted = fixture.open();
    assert_eq!(
        restarted
            .transparency_projection(checkpoint_digest, None, 16)
            .expect("rebuild signed projection after restart"),
        full_projection
    );
    let legacy = fixture
        .node
        .export_moderation_evidence_viewer_snapshot()
        .expect("export legacy evidence viewer snapshot");
    assert!(legacy.sessions.is_empty() && legacy.access_events.is_empty());
}

#[test]
fn transparency_projection_binds_signed_checkpoint_limit_and_freshness() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let checkpoint_digest = current_checkpoint_digest(&service);
    let signer_calls_before_reads = fixture.signer.sign_call_count();
    assert_eq!(
        service
            .transparency_projection([0; 32], None, 16)
            .expect_err("zero checkpoint expectation must fail"),
        EvidenceViewerErrorV1::InvalidRequest
    );
    for invalid_limit in [0, 1_025] {
        assert_eq!(
            service
                .transparency_projection(checkpoint_digest, None, invalid_limit)
                .expect_err("out-of-bounds page limit must fail"),
            EvidenceViewerErrorV1::InvalidRequest
        );
    }
    assert_eq!(
        service
            .transparency_projection(
                checkpoint_digest,
                Some(EvidenceViewerReceiptCursorV1 {
                    sequence: 0,
                    receipt_digest: [0xF2; 32],
                }),
                16,
            )
            .expect_err("zero-sequence predecessor must fail"),
        EvidenceViewerErrorV1::InvalidRequest
    );

    let empty_page = service
        .transparency_projection(checkpoint_digest, None, 16)
        .expect("read signed empty checkpoint");
    assert_eq!(empty_page.checkpoint_anchor.receipt_count, 0);
    assert_eq!(empty_page.checkpoint_anchor.chain_head, None);
    assert_eq!(empty_page.next_cursor, None);
    assert!(!empty_page.has_more);
    empty_page
        .verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        )
        .expect("verify signed empty checkpoint");

    let differently_bounded = service
        .transparency_projection(checkpoint_digest, None, 17)
        .expect("read same checkpoint with a different bound");
    assert_ne!(
        differently_bounded.projection_digest, empty_page.projection_digest,
        "the exact requested page bound must be digest-bound"
    );
    assert_eq!(
        fixture.signer.sign_call_count(),
        signer_calls_before_reads,
        "status and projection reads must not invoke the signer"
    );

    let mut tampered_limit = empty_page.clone();
    tampered_limit.page_limit = 15;
    assert_eq!(
        tampered_limit.verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        ),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    let mut tampered_anchor = empty_page;
    tampered_anchor.checkpoint_anchor.receipt_count = 1;
    assert_eq!(
        tampered_anchor.verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        ),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );

    fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0xF1; 32],
        BASE_UNIX_MS,
    );
    assert_ne!(current_checkpoint_digest(&service), checkpoint_digest);
    assert_eq!(
        service
            .transparency_projection(checkpoint_digest, None, 16)
            .expect_err("a stale checkpoint expectation must not silently repaginate"),
        EvidenceViewerErrorV1::CheckpointChanged
    );
}

#[test]
fn finalized_reauthorization_rejects_same_height_forks_and_persists_monotonic_head() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let challenge = fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0x86; 32],
        BASE_UNIX_MS,
    );
    let challenge_token = challenge.challenge.expose().to_owned();

    fixture
        .authorization
        .set_finalized_anchor(77, [0x93; 32], BASE_UNIX_MS - 1_000);
    assert_eq!(
        fixture
            .create_session(
                &service,
                &challenge_token,
                b"valid-webauthn-assertion-fork",
                [0x87; 32],
                BASE_UNIX_MS + 1,
            )
            .expect_err("same-height challenge fork must fail before WebAuthn consumption"),
        EvidenceViewerErrorV1::Forbidden
    );
    fixture
        .authorization
        .set_finalized_anchor(77, [0x92; 32], BASE_UNIX_MS - 1_000);
    let issued = fixture
        .create_session(
            &service,
            &challenge_token,
            b"valid-webauthn-assertion-fork",
            [0x87; 32],
            BASE_UNIX_MS + 2,
        )
        .expect("exact challenge anchor remains usable");
    let session_id = issued.session.local_session.session_id;
    let first_grant = issued.grant.expose().to_owned();

    fixture
        .authorization
        .set_finalized_anchor(78, [0x94; 32], BASE_UNIX_MS - 500);
    let advanced = service
        .manifest(
            session_id,
            JUROR_ACCOUNT,
            &opaque(&first_grant),
            [0x88; 32],
            [0x89; 32],
            BASE_UNIX_MS + 3,
        )
        .expect("strictly newer finalized anchor extends the session");
    assert_eq!(advanced.manifest.finalized_height, 78);
    assert_eq!(advanced.manifest.finalized_block_hash, [0x94; 32]);
    let second_grant = advanced.rotated_grant.expose().to_owned();

    fixture
        .authorization
        .set_finalized_anchor(78, [0x95; 32], BASE_UNIX_MS - 500);
    assert_eq!(
        service
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&second_grant),
                [0x8A; 32],
                [0x8B; 32],
                BASE_UNIX_MS + 4,
            )
            .expect_err("same-height session fork must fail"),
        EvidenceViewerErrorV1::Forbidden
    );
    fixture
        .authorization
        .set_finalized_anchor(77, [0x92; 32], BASE_UNIX_MS - 1_000);
    assert_eq!(
        service
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&second_grant),
                [0x8C; 32],
                [0x8D; 32],
                BASE_UNIX_MS + 5,
            )
            .expect_err("persisted finalized head must reject rollback"),
        EvidenceViewerErrorV1::Forbidden
    );

    drop(service);
    let restarted = fixture.open();
    fixture
        .authorization
        .set_finalized_anchor(79, [0x96; 32], BASE_UNIX_MS);
    let after_restart = restarted
        .manifest(
            session_id,
            JUROR_ACCOUNT,
            &opaque(&second_grant),
            [0x8E; 32],
            [0x8F; 32],
            BASE_UNIX_MS + 6,
        )
        .expect("new finalized head extends the persisted cursor after restart");
    assert_eq!(after_restart.manifest.finalized_height, 79);
    assert_eq!(after_restart.manifest.finalized_block_hash, [0x96; 32]);
}
