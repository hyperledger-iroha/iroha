use super::*;
use crate::sorafs::token::signer_test_support::{
    NOW_MS, ObserverFault, PROVIDER, SignedFixture, TestSignerMode,
};

fn policy() -> RegisteredAccountReadV1 {
    RegisteredAccountReadV1 {
        https_host: "gateway.example".into(),
        https_port: 443,
        ttl_secs: 60,
        max_streams: 1,
        rate_limit_bytes: 1024,
        requests_per_minute: 1,
    }
}
fn request<'a>(operation: &'a str, policy: &'a RegisteredAccountReadV1) -> Request<'a> {
    Request {
        subject: StreamTokenQuotaSubject([8; 32]),
        operation,
        manifest_id: "aa",
        manifest_cid: &[0xaa],
        provider: PROVIDER,
        profile: "sorafs.sf1@1.0.0",
        overrides: TokenOverrides::default(),
        policy,
    }
}
fn status() -> StatusKey {
    StatusKey::new(
        StreamTokenQuotaSubject([8; 32]),
        b"signed paid request",
        &[0xaa],
        "sorafs.sf1@1.0.0",
    )
}
fn op(n: u64) -> String {
    format!("{n:064x}:0")
}

#[test]
fn paid_ack_requires_one_admitted_original_and_each_release_checks_custody() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("jobs");
    let fixture = SignedFixture::new(1, TestSignerMode::Sign);
    let issuer = fixture.issuer().unwrap();
    let policy = policy();
    let operation = op(7);
    let req = || request(&operation, &policy);
    // Stop the real signer observer immediately before its first current-custody response.
    let stalled = fixture.requests.lock().unwrap();
    let issue = std::thread::scope(|scope| {
        let (send, received) = oneshot::channel();
        let worker = scope.spawn(|| issuer.issue_paid(&path, req(), status(), send));
        received.blocking_recv().unwrap();
        assert!(issuer.paid_work_active(status(), NOW_MS / 1000));
        assert!(!issuer.paid_work_active(
            StatusKey::new(
                StreamTokenQuotaSubject([9; 32]),
                b"signed paid request",
                &[0xaa],
                "sorafs.sf1@1.0.0"
            ),
            NOW_MS / 1000
        ));
        assert!(!issuer.paid_work_active(
            StatusKey::new(
                StreamTokenQuotaSubject([8; 32]),
                b"another signed request",
                &[0xaa],
                "sorafs.sf1@1.0.0"
            ),
            NOW_MS / 1000
        ));

        let record = std::fs::read_dir(&path)
            .unwrap()
            .flatten()
            .find(|e| e.path().extension().is_some_and(|s| s == "nrt"))
            .unwrap();
        assert!(record.metadata().unwrap().len() < MAX_RECORD as u64);
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        let (send, mut received) = oneshot::channel();
        assert!(issuer.issue_paid(&path, req(), status(), send).is_err());
        received.try_recv().unwrap();
        let (send, mut received) = oneshot::channel();
        let changed = Request {
            manifest_id: "ab",
            ..req()
        };
        assert!(matches!(
            issuer.issue_paid(&path, changed, status(), send),
            Err(Error::SignerEvidenceInvalid)
        ));
        assert!(received.try_recv().is_err());
        drop(stalled);
        worker.join().unwrap().unwrap()
    });
    assert_eq!(issue.remaining_quota, 0);
    assert!(!issuer.paid_work_active(status(), NOW_MS / 1000));
    let (send, mut rx) = oneshot::channel();
    let before = fixture.observer_calls.load(Ordering::SeqCst);
    let warm = issuer.issue_paid(&path, req(), status(), send).unwrap();
    assert!(rx.try_recv().is_err());
    assert_eq!(warm.token, issue.token);
    assert_eq!(fixture.observer_calls.load(Ordering::SeqCst), before + 1);
    fixture
        .faults
        .lock()
        .unwrap()
        .insert(before + 2, ObserverFault::SignerRevoked);
    let (send, mut rx) = oneshot::channel();
    assert!(issuer.issue_paid(&path, req(), status(), send).is_err());
    assert!(rx.try_recv().is_err());
    let different = op(9);
    let (send, mut rx) = oneshot::channel();
    assert!(matches!(
        issuer.issue_paid(&path, request(&different, &policy), status(), send),
        Err(Error::IssuanceQuotaExceeded { .. })
    ));
    assert!(rx.try_recv().is_err());
    let (send, mut rx) = oneshot::channel();
    let changed_policy = RegisteredAccountReadV1 {
        https_port: 8443,
        ..policy.clone()
    };
    assert!(
        issuer
            .issue_paid(&path, request(&operation, &changed_policy), status(), send)
            .is_err()
    );
    assert!(rx.try_recv().is_err());
    drop(issuer);
    let cold = fixture.issuer().unwrap();
    let (send, mut rx) = oneshot::channel();
    let reopened = cold.issue_paid(&path, req(), status(), send).unwrap();
    rx.try_recv().unwrap();
    assert_eq!(issue.token, reopened.token);
    assert_eq!(reopened.remaining_quota, 0);
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.recover_calls.load(Ordering::SeqCst), 1);
    let record = std::fs::read_dir(&path)
        .unwrap()
        .flatten()
        .find(|e| e.path().extension().is_some_and(|s| s == "nrt"))
        .unwrap()
        .path();
    let bytes = std::fs::read(&record).unwrap();
    assert!(
        !bytes
            .windows(issue.token.signature.len())
            .any(|w| w == issue.token.signature)
    );
    // A canonical frame with a substituted quota cannot recover an original.
    let mut forged: Original = norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(512, MAX_RECORD, 4096, 128 * 1024, 16),
    )
    .unwrap();
    forged.remaining = 1;
    std::fs::write(&record, norito::encode_canonical(&forged).unwrap()).unwrap();
    drop(cold);
    let cold = fixture.issuer().unwrap();
    let (send, mut rx) = oneshot::channel();
    assert!(matches!(
        cold.issue_paid(&path, req(), status(), send),
        Err(Error::SignerEvidenceInvalid)
    ));
    assert!(rx.try_recv().is_err());
    std::fs::write(&record, bytes).unwrap();
    fixture.clock.now.store(NOW_MS + 60_000, Ordering::SeqCst);
    let (send, mut rx) = oneshot::channel();
    assert!(matches!(
        cold.issue_paid(&path, req(), status(), send),
        Err(Error::SignerStateChanged)
    ));
    assert!(rx.try_recv().is_err());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn paid_operation_and_workspace_admission_are_bounded() {
    assert!(valid_operation(&op(1)));
    for bad in [
        "".into(),
        format!("{}:0", "0".repeat(64)),
        format!("{}:0", "A".repeat(64)),
        format!("{}:01", "a".repeat(64)),
        format!("{}:+1", "a".repeat(64)),
        format!("{}:18446744073709551616", "a".repeat(64)),
    ] {
        assert!(!valid_operation(&bad));
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("jobs");
    let fixture = SignedFixture::new(1, TestSignerMode::Sign);
    let issuer = fixture.issuer().unwrap();
    let policy = policy();
    let operation = op(1);
    let req = || request(&operation, &policy);
    let slot = req().identity(&issuer.verifying_key_bytes()).unwrap().0[0] % SLOTS as u8;
    let collision = (2..10_000)
        .map(op)
        .find(|s| {
            request(s, &policy)
                .identity(&issuer.verifying_key_bytes())
                .unwrap()
                .0[0]
                % SLOTS as u8
                == slot
        })
        .unwrap();
    let (send, _) = oneshot::channel();
    issuer.issue_paid(&path, req(), status(), send).unwrap();
    let (send, mut rx) = oneshot::channel();
    assert!(matches!(
        issuer.issue_paid(&path, request(&collision, &policy), status(), send),
        Err(Error::IssuanceQuotaCapacityExceeded { .. })
    ));
    assert!(rx.try_recv().is_err());
    let issuer = fixture.issuer().unwrap();
    let dir = PrivateDirectory::open_or_create(&path).unwrap();
    let lease = dir.open_lock(format!("slot-{slot}.lock")).unwrap();
    lease.try_lock().unwrap();
    let (send, mut rx) = oneshot::channel();
    assert!(matches!(
        issuer.issue_paid(&path, req(), status(), send),
        Err(Error::IssuanceQuotaCapacityExceeded { .. })
    ));
    assert!(rx.try_recv().is_err());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
}
