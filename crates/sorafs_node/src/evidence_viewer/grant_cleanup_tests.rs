// Real session/Node/persistence regressions for explicit grant-publication settlement.
struct GrantCleanupProbe {
    inner: Arc<MockGrantBoundary>,
    service: Mutex<Option<std::sync::Weak<EvidenceViewerServiceV1>>>,
    revoke_calls: Mutex<Vec<[u8; 32]>>,
    fail_revoke: AtomicBool,
    on_issue: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl fmt::Debug for GrantCleanupProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GrantCleanupProbe(<redacted>)")
    }
}
impl EvidenceViewerRuntimeProviderV1 for GrantCleanupProbe {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        EvidenceViewerRuntimeProviderQualificationV1,
        EvidenceViewerRuntimeProviderReadinessErrorV1,
    > {
        self.inner.qualification()
    }
}
impl EvidenceViewerGrantBoundaryV1 for GrantCleanupProbe {
    fn issue(
        &self,
        claims: &EvidenceViewerGrantClaimsV1,
    ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1> {
        // A deterministic exact-claims issuer deliberately has no sequence counter. Independent
        // revocation depends on the production nonce, not on a uniqueness property of this mock.
        let token = {
            let _operation = self.inner.qualification.operation_guard();
            let bytes = norito::encode_canonical(claims)
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
            let token = format!("grant-{}", blake3::hash(&bytes).to_hex());
            self.inner
                .issued
                .lock()
                .expect("probe grants")
                .insert(token.clone(), claims.clone());
            OpaqueEvidenceViewerSecretV1::new(token)?
        };
        let hook = self.on_issue.lock().expect("probe issue hook").take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(token)
    }
    fn verify(
        &self,
        token: &str,
        claims: &EvidenceViewerGrantClaimsV1,
        now: u64,
    ) -> Result<(), EvidenceViewerExternalErrorV1> {
        self.inner.verify(token, claims, now)
    }
    fn revoke(&self, digest: [u8; 32]) -> Result<(), EvidenceViewerExternalErrorV1> {
        let service = self
            .service
            .lock()
            .expect("probe service")
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        if let Some(service) = service {
            assert!(
                service.state.try_lock().is_ok(),
                "grant cleanup must not hold the service mutex"
            );
        }
        self.revoke_calls
            .lock()
            .expect("probe revocations")
            .push(digest);
        if self.fail_revoke.load(Ordering::SeqCst) {
            return Err(EvidenceViewerExternalErrorV1::Unavailable);
        }
        self.inner.revoke(digest)
    }
}
fn grant_probe(fixture: &mut EvidenceViewerFixture) -> Arc<GrantCleanupProbe> {
    let probe = Arc::new(GrantCleanupProbe {
        inner: fixture.grants.clone(),
        service: Mutex::new(None),
        revoke_calls: Mutex::new(Vec::new()),
        fail_revoke: AtomicBool::new(false),
        on_issue: Mutex::new(None),
    });
    fixture.deps.grants = probe.clone();
    probe
}
fn probe_service(
    fixture: &EvidenceViewerFixture,
    probe: &GrantCleanupProbe,
) -> Arc<EvidenceViewerServiceV1> {
    let service = Arc::new(fixture.open());
    *probe.service.lock().expect("probe service") = Some(Arc::downgrade(&service));
    service
}
fn probe_session(
    fixture: &EvidenceViewerFixture,
    service: &EvidenceViewerServiceV1,
) -> EvidenceViewerSessionIssuedV1 {
    let challenge = fixture.issue_challenge(
        service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0x61; 32],
        BASE_UNIX_MS,
    );
    fixture
        .create_session(
            service,
            challenge.challenge.expose(),
            b"valid-webauthn-assertion-grant-cleanup",
            [0x62; 32],
            BASE_UNIX_MS + 1,
        )
        .expect("actual attested session")
}
fn probe_range(
    service: &EvidenceViewerServiceV1,
    session: &EvidenceViewerSessionIssuedV1,
    start: u64,
    end: u64,
    key: u8,
) -> Result<EvidenceViewerRangeOutcomeV1, EvidenceViewerErrorV1> {
    service.read_range(
        session.session.local_session.session_id,
        JUROR_ACCOUNT,
        &session.grant,
        start,
        end,
        [key; 32],
        [key.wrapping_add(1); 32],
        BASE_UNIX_MS + 2,
    )
}
fn only_new_grant(fixture: &EvidenceViewerFixture, before: &[String]) -> String {
    let added = fixture
        .grants
        .issued_tokens()
        .into_iter()
        .filter(|token| !before.contains(token))
        .collect::<Vec<_>>();
    assert_eq!(added.len(), 1, "exactly one actual issue");
    added[0].clone()
}
fn assert_revoke_once(probe: &GrantCleanupProbe, token: &str) {
    let digest = *blake3::hash(token.as_bytes()).as_bytes();
    assert_eq!(
        probe
            .revoke_calls
            .lock()
            .expect("probe revocations")
            .iter()
            .filter(|item| **item == digest)
            .count(),
        1
    );
}
#[test]
fn range_node_failures_revoke_one_replacement_and_preserve_original_grant() {
    for corrupt in [false, true] {
        let mut fixture = EvidenceViewerFixture::new();
        let probe = grant_probe(&mut fixture);
        let service = probe_service(&fixture, &probe);
        let issued = probe_session(&fixture, &service);
        let path = fixture
            .node
            .moderation_quarantine_object_root
            .as_ref()
            .unwrap()
            .join(&fixture.object.envelope_path);
        let envelope = fs::read(&path).expect("actual sealed object");
        let before = service.state.lock().expect("state").clone();
        let cache = fs::read(&fixture.config.checkpoint_path).expect("cache");
        let tokens = fixture.grants.issued_tokens();
        if corrupt {
            let mut damaged = envelope.clone();
            *damaged.last_mut().expect("frame") ^= 1;
            fs::write(&path, damaged).expect("corrupt actual envelope");
        }
        let end = EVIDENCE_PAYLOAD.len() as u64;
        let result = if corrupt {
            probe_range(&service, &issued, 0, end, 0x63)
        } else {
            probe_range(&service, &issued, end, end + 1, 0x63)
        };
        assert_eq!(
            result.expect_err("actual Node read must fail"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        let replacement = only_new_grant(&fixture, &tokens);
        assert_revoke_once(&probe, &replacement);
        assert!(fixture.grants.was_revoked(&replacement));
        assert!(!fixture.grants.was_revoked(issued.grant.expose()));
        assert_eq!(*service.state.lock().expect("state"), before);
        assert_eq!(fs::read(&fixture.config.checkpoint_path).unwrap(), cache);
        fs::write(&path, envelope).expect("restore exact ciphertext");
        drop(service);
        let restarted = probe_service(&fixture, &probe);
        let outcome = probe_range(&restarted, &issued, 0, end, 0x65)
            .expect("original grant remains usable after restart");
        assert_eq!(outcome.range.payload, EVIDENCE_PAYLOAD);
        assert_revoke_once(&probe, issued.grant.expose());
        assert!(!fixture.grants.was_revoked(outcome.rotated_grant.expose()));
    }
}
#[test]
fn replacement_object_binding_failure_reclaims_grant_before_output() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let original = fixture.object.clone();
    let input = ModerationQuarantineObjectInput {
        quarantine_id: fixture.quarantine_id,
        payload: EVIDENCE_PAYLOAD.to_vec(),
        captured_at_unix: BASE_UNIX_MS / 1000 - 4,
        content_type: Some("application/octet-stream".to_owned()),
        notes: None,
    };
    let binding = fixture
        .node
        .moderation_quarantine_key_provider_binding
        .as_ref()
        .expect("actual key binding");
    let wrapper = fixture
        .node
        .moderation_quarantine_key_wrapper
        .as_ref()
        .expect("actual wrapper");
    let (replacement, bytes) =
        crate::moderation::seal_moderation_quarantine_object(input, binding, &**wrapper)
            .expect("genuine same-payload independent object");
    assert_ne!(replacement.object_id, original.object_id);
    assert_eq!(replacement.payload_digest, original.payload_digest);
    let path = fixture
        .node
        .moderation_quarantine_object_root
        .as_ref()
        .unwrap()
        .join(&replacement.envelope_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_local_checkpoint_atomic_bounded(
        &path,
        &bytes,
        fixture
            .node
            .config()
            .runtime_retention()
            .checkpoint_max_bytes(),
    )
    .expect("private canonical object writer");
    fixture
        .node
        .restore_moderation_quarantine_object_snapshot(crate::ModerationQuarantineObjectSnapshot {
            objects: vec![replacement.clone()],
        })
        .expect("restore actual authenticated replacement index");
    fixture
        .node
        .read_moderation_quarantine_object_range(
            fixture.quarantine_id,
            0,
            EVIDENCE_PAYLOAD.len() as u64,
        )
        .expect("Node independently accepts actual replacement");
    let before = service.state.lock().unwrap().clone();
    let tokens = fixture.grants.issued_tokens();
    assert_eq!(
        probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x67)
            .expect_err("session binds original object"),
        EvidenceViewerErrorV1::AuthenticationRejected
    );
    let grant = only_new_grant(&fixture, &tokens);
    assert_revoke_once(&probe, &grant);
    assert_eq!(*service.state.lock().unwrap(), before);
    assert!(!fixture.grants.was_revoked(issued.grant.expose()));
    fixture
        .node
        .restore_moderation_quarantine_object_snapshot(crate::ModerationQuarantineObjectSnapshot {
            objects: vec![original],
        })
        .expect("restore original index");
    assert_eq!(
        probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x69)
            .unwrap()
            .range
            .payload,
        EVIDENCE_PAYLOAD
    );
}
#[test]
fn rejected_cas_reclaims_replacement_after_unlock_and_success_keeps_new_grant() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let before = service.state.lock().unwrap().clone();
    let tokens = fixture.grants.issued_tokens();
    fixture
        .checkpoint_store
        .set_next_cas_mode(MockCheckpointCasMode::RejectedNoCommit);
    assert_eq!(
        probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x6b)
            .expect_err("definite rejected CAS"),
        EvidenceViewerErrorV1::CheckpointChanged
    );
    let grant = only_new_grant(&fixture, &tokens);
    assert_revoke_once(&probe, &grant);
    assert_eq!(*service.state.lock().unwrap(), before);
    assert!(!fixture.grants.was_revoked(issued.grant.expose()));
    let result = probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x6d)
        .expect("successful rotation");
    assert_revoke_once(&probe, issued.grant.expose());
    assert!(!fixture.grants.was_revoked(result.rotated_grant.expose()));
    let stored =
        service.state.lock().unwrap().sessions[&issued.session.local_session.session_id].clone();
    assert_ne!(
        stored.active_grant_issuance_nonce,
        issued.session.active_grant_issuance_nonce
    );
    assert_eq!(stored.active_grant_digest, result.rotated_grant.digest());
}
#[test]
fn failed_revoke_is_one_attempt_and_never_claimed_as_reclaimed() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    probe.fail_revoke.store(true, Ordering::SeqCst);
    let tokens = fixture.grants.issued_tokens();
    let end = EVIDENCE_PAYLOAD.len() as u64;
    assert_eq!(
        probe_range(&service, &issued, end, end + 1, 0x6f).expect_err("range rejection retained"),
        EvidenceViewerErrorV1::RuntimeUnavailable
    );
    let grant = only_new_grant(&fixture, &tokens);
    assert_revoke_once(&probe, &grant);
    assert!(!fixture.grants.was_revoked(&grant));
    assert!(!fixture.grants.was_revoked(issued.grant.expose()));
    probe.fail_revoke.store(false, Ordering::SeqCst);
    probe_range(&service, &issued, 0, end, 0x71)
        .expect("original grant usable after cleanup failure");
}
#[test]
fn compacted_challenge_after_issue_reclaims_unpublished_session_grant() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let challenge = fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0x73; 32],
        BASE_UNIX_MS,
    );
    let weak = Arc::downgrade(&service);
    let cutoff = challenge.expires_at_unix_ms;
    *probe.on_issue.lock().unwrap() = Some(Box::new(move || {
        weak.upgrade()
            .unwrap()
            .compact_expired_tick(cutoff)
            .expect("real archive compaction")
            .expect("expired challenge removed");
    }));
    let tokens = fixture.grants.issued_tokens();
    assert_eq!(
        fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"valid-webauthn-assertion-late-challenge-cleanup",
                [0x74; 32],
                BASE_UNIX_MS + 1
            )
            .expect_err("challenge removed while external issue was in flight"),
        EvidenceViewerErrorV1::AuthenticationRejected
    );
    let grant = only_new_grant(&fixture, &tokens);
    assert_revoke_once(&probe, &grant);
    assert!(fixture.grants.was_revoked(&grant));
    assert!(service.state.lock().unwrap().sessions.is_empty());
    service
        .audit_status()
        .expect("unpublished failure does not poison valid compacted authority");
}
#[test]
fn post_issue_qualification_failure_retires_only_the_owned_returned_credential() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let tokens = fixture.grants.issued_tokens();

    // Drift after actual issue, not after the preceding current-grant verification.
    *probe.on_issue.lock().unwrap() = Some(Box::new({
        let grants = fixture.grants.clone();
        move || grants.qualification.set_policy_digest([0xe1; 32])
    }));
    assert_eq!(
        probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x75)
            .expect_err("post-issue qualification denied"),
        EvidenceViewerErrorV1::RuntimeUnavailable
    );
    let grant = only_new_grant(&fixture, &tokens);
    assert!(fixture.grants.was_revoked(&grant));
    assert_revoke_once(&probe, &grant);
    assert!(!fixture.grants.was_revoked(issued.grant.expose()));
    fixture.grants.qualification.set_policy_digest([0xA2; 32]);
    probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x77)
        .expect("original grant remains usable after restored qualification");
}
#[test]
fn independently_issued_nonce_prevents_loser_revoking_same_claims_winner() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let winner = Arc::new(Mutex::new(None));
    let captured = winner.clone();
    let weak = Arc::downgrade(&service);
    let original = issued.grant.expose().to_owned();
    let id = issued.session.local_session.session_id;
    // Deterministic interleaving: the outer request has issued its replacement but has not
    // committed; the nested real request verifies the same original grant at the same time.
    *probe.on_issue.lock().unwrap() = Some(Box::new(move || {
        let outcome = weak
            .upgrade()
            .unwrap()
            .manifest(
                id,
                JUROR_ACCOUNT,
                &opaque(&original),
                [0x79; 32],
                [0x7a; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("overlapping request wins");
        *captured.lock().unwrap() = Some(outcome);
    }));
    let tokens = fixture.grants.issued_tokens();
    assert_eq!(
        service
            .manifest(
                id,
                JUROR_ACCOUNT,
                &issued.grant,
                [0x7b; 32],
                [0x7c; 32],
                BASE_UNIX_MS + 2
            )
            .expect_err("stale generation loses"),
        EvidenceViewerErrorV1::AuthenticationRejected
    );
    let winner = winner.lock().unwrap().take().unwrap();
    let added = fixture
        .grants
        .issued_tokens()
        .into_iter()
        .filter(|token| !tokens.contains(token))
        .collect::<Vec<_>>();
    assert_eq!(added.len(), 2);
    let loser = added
        .iter()
        .find(|token| token.as_str() != winner.rotated_grant.expose())
        .unwrap();
    assert_revoke_once(&probe, loser);
    assert!(fixture.grants.was_revoked(loser));
    assert!(!fixture.grants.was_revoked(winner.rotated_grant.expose()));
    let claims = fixture.grants.issued.lock().unwrap();
    let mut left = claims[loser].clone();
    let right = claims[winner.rotated_grant.expose()].clone();
    assert_ne!(left.issuance_nonce, right.issuance_nonce);
    left.issuance_nonce = right.issuance_nonce;
    assert_eq!(left, right, "all other claims are identical");
    drop(claims);
    drop(service);
    let restarted = probe_service(&fixture, &probe);
    assert_eq!(
        restarted.state.lock().unwrap().sessions[&id].active_grant_issuance_nonce,
        right.issuance_nonce
    );
    restarted
        .manifest(
            id,
            JUROR_ACCOUNT,
            &winner.rotated_grant,
            [0x7d; 32],
            [0x7e; 32],
            BASE_UNIX_MS + 3,
        )
        .expect("winner grant remains usable after restart");
}
#[test]
fn grant_nonce_is_canonical_nonzero_and_verified_from_installed_state() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let claims = grant_claims(&issued.session);
    assert_ne!(claims.issuance_nonce, [0; 32]);
    let expected = norito::encode_canonical(&claims).unwrap();
    for flags in viewer_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&claims).unwrap(), expected);
        assert_eq!(
            norito::decode_canonical::<EvidenceViewerGrantClaimsV1>(&expected).unwrap(),
            claims
        );
    }
    let mut wrong = claims.clone();
    wrong.issuance_nonce[0] ^= 1;
    assert_eq!(
        probe.verify(issued.grant.expose(), &wrong, BASE_UNIX_MS + 2),
        Err(EvidenceViewerExternalErrorV1::Rejected)
    );
    probe
        .verify(issued.grant.expose(), &claims, BASE_UNIX_MS + 2)
        .expect("exact installed claims");
    let record = fixture.checkpoint_store.current().unwrap();
    let mut envelope: EvidenceViewerCheckpointEnvelopeV1 =
        norito::decode_canonical(&record.checkpoint_bytes).unwrap();
    envelope.checkpoint.sessions[0].active_grant_issuance_nonce[0] ^= 1;
    assert_eq!(
        verify_checkpoint_envelope(&fixture.config, envelope),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint),
        "signed checkpoint binds the issuance nonce"
    );
    let mut checkpoint = checkpoint_from_state(&service.state.lock().unwrap());
    checkpoint.sessions[0].active_grant_issuance_nonce = [0; 32];
    assert_eq!(
        validate_checkpoint(&fixture.config, &checkpoint),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
}
#[test]
fn committed_grant_survives_local_cache_write_failure_and_restart() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let tokens = fixture.grants.issued_tokens();
    let old = fixture.checkpoint_store.current().unwrap();
    fs::remove_file(&fixture.config.checkpoint_path).unwrap();
    fs::create_dir(&fixture.config.checkpoint_path).unwrap();
    assert_eq!(
        probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x81)
            .expect_err("cache write fails after real authoritative CAS"),
        EvidenceViewerErrorV1::CheckpointUnavailable
    );
    let grant = only_new_grant(&fixture, &tokens);
    let next = fixture.checkpoint_store.current().unwrap();
    assert_eq!(next.generation, old.generation + 1);
    assert!(service.state.lock().unwrap().durability_uncertain);
    assert_eq!(
        service.state.lock().unwrap().sessions[&issued.session.local_session.session_id]
            .active_grant_digest,
        *blake3::hash(grant.as_bytes()).as_bytes()
    );
    assert!(!fixture.grants.was_revoked(&grant));
    assert!(probe.revoke_calls.lock().unwrap().is_empty());
    fs::remove_dir(&fixture.config.checkpoint_path).unwrap();
    drop(service);
    let restarted = probe_service(&fixture, &probe);
    restarted
        .manifest(
            issued.session.local_session.session_id,
            JUROR_ACCOUNT,
            &opaque(&grant),
            [0x83; 32],
            [0x84; 32],
            BASE_UNIX_MS + 3,
        )
        .expect("actual installed grant remains valid after authoritative restart");
}
struct DeferredGrantCheckpointStore {
    inner: Arc<MockCheckpointStore>,
    defer: AtomicBool,
    unavailable_readback: AtomicBool,
    pending: Mutex<Option<EvidenceViewerCheckpointStoreRecordV1>>,
}
impl fmt::Debug for DeferredGrantCheckpointStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeferredGrantCheckpointStore")
    }
}
impl EvidenceViewerRuntimeProviderV1 for DeferredGrantCheckpointStore {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        EvidenceViewerRuntimeProviderQualificationV1,
        EvidenceViewerRuntimeProviderReadinessErrorV1,
    > {
        self.inner.qualification()
    }
}
impl EvidenceViewerCheckpointStoreV1 for DeferredGrantCheckpointStore {
    fn load_latest(
        &self,
    ) -> Result<
        Option<EvidenceViewerCheckpointStoreRecordV1>,
        EvidenceViewerCheckpointStoreExternalErrorV1,
    > {
        if self.pending.lock().unwrap().is_some()
            && self.unavailable_readback.swap(false, Ordering::SeqCst)
        {
            return Err(EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable);
        }
        self.inner.load_latest()
    }
    fn compare_and_swap_latest(
        &self,
        expected: Option<[u8; 32]>,
        next: &EvidenceViewerCheckpointStoreRecordV1,
    ) -> Result<(), EvidenceViewerCheckpointStoreExternalErrorV1> {
        if self.defer.swap(false, Ordering::SeqCst) {
            assert_eq!(self.inner.current().as_ref().map(|r| r.revision), expected);
            *self.pending.lock().unwrap() = Some(next.clone());
            return Err(EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous);
        }
        self.inner.compare_and_swap_latest(expected, next)
    }
}
#[test]
fn predecessor_readback_after_ambiguous_cas_does_not_revoke_a_later_commit() {
    for unavailable_readback in [false, true] {
        let mut fixture = EvidenceViewerFixture::new();
        let probe = grant_probe(&mut fixture);
        let checkpoint = Arc::new(DeferredGrantCheckpointStore {
            inner: fixture.checkpoint_store.clone(),
            defer: AtomicBool::new(false),
            unavailable_readback: AtomicBool::new(unavailable_readback),
            pending: Mutex::new(None),
        });
        let service = Arc::new(
            EvidenceViewerServiceV1::open_with_checkpoint_store(
                fixture.config.clone(),
                fixture.deps.clone(),
                fixture.node.clone(),
                TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
                TEST_CHECKPOINT_STORE_QUALIFICATION,
                checkpoint.clone(),
            )
            .unwrap(),
        );
        *probe.service.lock().unwrap() = Some(Arc::downgrade(&service));
        let issued = probe_session(&fixture, &service);
        let before = fixture.checkpoint_store.current().unwrap();
        let tokens = fixture.grants.issued_tokens();
        checkpoint.defer.store(true, Ordering::SeqCst);
        assert_eq!(
            probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x85)
                .expect_err("predecessor readback cannot settle an in-flight CAS"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );
        let grant = only_new_grant(&fixture, &tokens);
        assert_eq!(fixture.checkpoint_store.current(), Some(before));
        assert!(service.state.lock().unwrap().durability_uncertain);
        assert!(!fixture.grants.was_revoked(&grant));
        assert!(probe.revoke_calls.lock().unwrap().is_empty());
        let delayed = checkpoint.pending.lock().unwrap().take().unwrap();
        fixture.checkpoint_store.replace_latest(Some(delayed));
        drop(service);
        let restarted = probe_service(&fixture, &probe);
        restarted
            .manifest(
                issued.session.local_session.session_id,
                JUROR_ACCOUNT,
                &opaque(&grant),
                [0x87; 32],
                [0x88; 32],
                BASE_UNIX_MS + 3,
            )
            .expect("delayed installed credential not revoked by predecessor readback");
    }
}
#[test]
fn exact_next_readback_preserves_ambiguous_commit_success() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    fixture
        .checkpoint_store
        .set_next_cas_mode(MockCheckpointCasMode::AmbiguousCommit);
    let outcome = probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x89)
        .expect("exact authenticated next record settles commit");
    assert_eq!(outcome.range.payload, EVIDENCE_PAYLOAD);
    assert!(!fixture.grants.was_revoked(outcome.rotated_grant.expose()));
    assert_revoke_once(&probe, issued.grant.expose());
}
#[test]
fn unsettled_no_commit_and_signing_failures_keep_distinct_grant_dispositions() {
    for ambiguous in [false, true] {
        let mut fixture = EvidenceViewerFixture::new();
        let probe = grant_probe(&mut fixture);
        let service = probe_service(&fixture, &probe);
        let issued = probe_session(&fixture, &service);
        let before = fixture.checkpoint_store.current().unwrap();
        let tokens = fixture.grants.issued_tokens();
        if ambiguous {
            fixture
                .checkpoint_store
                .set_next_cas_mode(MockCheckpointCasMode::AmbiguousNoCommit);
        } else {
            fixture.signer.set_corrupt_signatures(true);
        }
        assert_eq!(
            probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x8b)
                .expect_err("injected publication failure"),
            if ambiguous {
                EvidenceViewerErrorV1::CheckpointUnavailable
            } else {
                EvidenceViewerErrorV1::RuntimeUnavailable
            }
        );
        let replacement = only_new_grant(&fixture, &tokens);
        assert_eq!(fixture.checkpoint_store.current(), Some(before));
        assert!(!fixture.grants.was_revoked(issued.grant.expose()));
        if ambiguous {
            assert!(service.state.lock().unwrap().durability_uncertain);
            assert!(!fixture.grants.was_revoked(&replacement));
            assert!(probe.revoke_calls.lock().unwrap().is_empty());
        } else {
            assert_revoke_once(&probe, &replacement);
            assert!(fixture.grants.was_revoked(&replacement));
            fixture.signer.set_corrupt_signatures(false);
            probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x8d)
                .expect("original grant valid after definite signing failure");
        }
    }
}
#[test]
fn invalid_range_preflight_never_issues_or_revokes_a_grant() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let tokens = fixture.grants.issued_tokens();
    assert_eq!(
        probe_range(&service, &issued, 1, 1, 0x8f).expect_err("empty range"),
        EvidenceViewerErrorV1::InvalidRequest
    );
    assert_eq!(fixture.grants.issued_tokens(), tokens);
    assert!(probe.revoke_calls.lock().unwrap().is_empty());
}
#[test]
fn verified_other_winner_cache_failure_still_reclaims_the_unpublished_grant() {
    let mut fixture = EvidenceViewerFixture::new();
    let probe = grant_probe(&mut fixture);
    let service = probe_service(&fixture, &probe);
    let issued = probe_session(&fixture, &service);
    let before = fixture.checkpoint_store.current().unwrap();
    let competing = service
        .sign_checkpoint_store_record(
            before.checkpoint_digest,
            fixture.successor_checkpoint_bytes(&before),
            Some(&before),
        )
        .expect("independently verified competing successor");
    let tokens = fixture.grants.issued_tokens();
    fixture
        .checkpoint_store
        .set_next_cas_mode(MockCheckpointCasMode::RaceWith(Box::new(competing.clone())));
    fs::remove_file(&fixture.config.checkpoint_path).unwrap();
    fs::create_dir(&fixture.config.checkpoint_path).unwrap();
    assert_eq!(
        probe_range(&service, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x91)
            .expect_err("cache write after verified losing CAS"),
        EvidenceViewerErrorV1::CheckpointUnavailable
    );
    let grant = only_new_grant(&fixture, &tokens);
    assert_eq!(fixture.checkpoint_store.current(), Some(competing));
    assert!(service.state.lock().unwrap().durability_uncertain);
    assert_revoke_once(&probe, &grant);
    assert!(fixture.grants.was_revoked(&grant));
    assert!(!fixture.grants.was_revoked(issued.grant.expose()));
    fs::remove_dir(&fixture.config.checkpoint_path).unwrap();
    drop(service);
    let restarted = probe_service(&fixture, &probe);
    probe_range(&restarted, &issued, 0, EVIDENCE_PAYLOAD.len() as u64, 0x93)
        .expect("competing authority kept the original grant");
}
