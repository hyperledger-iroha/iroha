//! Post-restart catalog checks reuse genuine original signatures and perform only one status GET.
use super::*;

#[test]
fn observation_reopens_exact_promoted_original_without_mutating_any_retained_bytes() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-observe");
    let provider = provider(&prepared);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    let path = operation(&publisher);
    let mut script = complete_script();
    script.push(Step::Status(Observation::Promoted));
    let mut http = RuntimeHttp::start(&prepared, provider, &path, script);
    let expected = publisher
        .advance(&mut TestLive::default(), deadline())
        .unwrap();
    let original = bytes(&path, "original.nrt");
    let ack = bytes(&path, "acknowledgement.nrt");
    let before = http.requests.lock().unwrap().len();
    drop(publisher);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    publisher
        .observe_promoted(&mut TestLive::default(), expected, deadline())
        .unwrap();
    assert_eq!(bytes(&path, "original.nrt"), original);
    assert_eq!(bytes(&path, "acknowledgement.nrt"), ack);
    http.finish();
    let requests = http.requests.lock().unwrap();
    assert_eq!(requests.len(), before + 1);
    assert_eq!(requests.last().unwrap().method, "GET");
}

#[test]
fn observation_requires_exact_original_ack_and_live_guard_before_http() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-observe-refusal");
    let provider = provider(&prepared);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    let path = operation(&publisher);
    let mut http = RuntimeHttp::start(&prepared, provider, &path, complete_script());
    let expected = publisher
        .advance(&mut TestLive::default(), deadline())
        .unwrap();
    let before = http.requests.lock().unwrap().len();
    let mut earlier_issue = expected;
    earlier_issue.generated_at_unix = earlier_issue.generated_at_unix.saturating_sub(1);
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), earlier_issue, deadline())
            .is_err()
    );
    let mut foreign = expected;
    foreign.digest[0] ^= 1;
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), foreign, deadline())
            .is_err()
    );
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), expected, Instant::now())
            .is_err()
    );
    assert!(
        publisher
            .observe_promoted(
                &mut TestLive {
                    checks: 0,
                    fail_at: Some(1)
                },
                expected,
                deadline()
            )
            .is_err()
    );
    let child = PrivateDirectory::open_exact(path.join("catalogs").join(catalog_name(1))).unwrap();
    child
        .write_atomic("acknowledgement.nrt", b"changed ACK", PublishMode::Replace)
        .unwrap();
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), expected, deadline())
            .is_err()
    );
    assert_eq!(http.requests.lock().unwrap().len(), before);
    http.finish();
}

#[test]
fn observation_rejects_live_guard_loss_or_original_change_during_status_without_post() {
    let _resources = crate::managed::native_test_guard();
    for (label, step, fail_at) in [
        ("guard", Step::Status(Observation::Promoted), Some(3)),
        (
            "deleted",
            Step::ChangeAfterStatus(Observation::Promoted, RetainedChange::DeleteOriginal),
            None,
        ),
        (
            "replaced",
            Step::ChangeAfterStatus(Observation::Promoted, RetainedChange::ReplaceOriginal),
            None,
        ),
        (
            "ack",
            Step::ChangeAfterStatus(Observation::Promoted, RetainedChange::CorruptAck),
            None,
        ),
        ("foreign", Step::Status(Observation::ForeignHead), None),
        ("stale", Step::Status(Observation::Stale), None),
        (
            "pending",
            Step::Status(Observation::Candidate { ack: true }),
            None,
        ),
    ] {
        let (_temporary, prepared) = fixture(&format!("compliance-observe-{label}"));
        let provider = provider(&prepared);
        let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
        let path = operation(&publisher);
        let mut script = complete_script();
        script.push(step);
        let mut http = RuntimeHttp::start(&prepared, provider, &path, script);
        let expected = publisher
            .advance(&mut TestLive::default(), deadline())
            .unwrap();
        let before = http.requests.lock().unwrap().len();
        assert!(
            publisher
                .observe_promoted(&mut TestLive { checks: 0, fail_at }, expected, deadline())
                .is_err(),
            "{label}"
        );
        http.finish();
        let requests = http.requests.lock().unwrap();
        assert_eq!(requests.len(), before + 1, "{label}");
        assert_eq!(requests.last().unwrap().method, "GET");
    }
}

#[test]
fn absent_catalog_observation_does_not_create_catalog_custody() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("compliance-observe-absent");
    let provider = provider(&prepared);
    let publisher = ManagedGatewayCompliance::open(&prepared, provider).unwrap();
    let path = operation(&publisher);
    let mut http = RuntimeHttp::start(&prepared, provider, &path, vec![]);
    let expectation = PromotedGeneratedCatalog {
        digest: [1; 32],
        sequence: 1,
        generated_at_unix: now_ms().unwrap() / 1_000,
        valid_until_unix: now_ms().unwrap() / 1_000 + 60,
    };
    assert!(
        publisher
            .observe_promoted(&mut TestLive::default(), expectation, deadline())
            .is_err()
    );
    assert!(!path.join("catalogs").exists());
    assert!(http.requests.lock().unwrap().is_empty());
    http.finish();
}
