//! Exact overlap, deadline and physical-runtime admission controls.
use super::*;

fn mismatch(expected: u64, actual: u64) -> MergeLedgerCommitError {
    iroha_core::kura::Error::QueuePlanAdmissionDurableHeightMismatch {
        expected_durable_height: expected,
        actual_durable_height: actual,
    }
    .into()
}

#[test]
fn only_an_exact_one_block_lead_is_retryable() {
    assert_eq!(publication_overlap_height(&mismatch(5, 6)), Some(6));
    for error in [
        mismatch(5, 5),
        mismatch(5, 4),
        mismatch(5, 7),
        mismatch(u64::MAX, 0),
        MergeLedgerCommitError::ExecutionBatchInvalid("invalid certificate".to_owned()),
    ] {
        assert_eq!(publication_overlap_height(&error), None);
    }
}

#[test]
fn finalization_uses_the_original_monotonic_budget() {
    let started = Instant::now() - super::super::TORII_PROXY_EXECUTION_BUDGET;
    let deadline =
        PersistenceDeadline::new(started, super::super::torii_proxy_test_deadline_unix_ms());
    assert!(
        deadline.remaining().is_err(),
        "a fresh wall-clock horizon cannot renew elapsed ingress time"
    );
    let started = Instant::now() - Duration::from_secs(63);
    let deadline =
        PersistenceDeadline::new(started, super::super::torii_proxy_test_deadline_unix_ms());
    assert!(deadline.remaining().unwrap() <= Duration::from_secs(1));
}

#[test]
fn finalization_also_obeys_the_original_wire_deadline() {
    let now = super::super::torii_proxy_now_unix_ms().unwrap();
    let expired = PersistenceDeadline::new(Instant::now(), now.saturating_sub(1));
    assert!(expired.remaining().is_err());
    let short = PersistenceDeadline::new(Instant::now(), now + 1_000);
    assert!(short.remaining().unwrap() <= Duration::from_secs(1));
}

#[tokio::test]
async fn unsupported_runtime_fails_before_any_certificate_work() {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        iroha_core::state::World::default(),
        kura.clone(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    let deadline = PersistenceDeadline::new(
        Instant::now(),
        super::super::torii_proxy_test_deadline_unix_ms(),
    );
    let error = deadline
        .persist(&state, b"invalid certificate")
        .await
        .unwrap_err();
    assert!(error.contains("multi-thread Tokio runtime"));
    assert!(
        !kura
            .store_root()
            .join("pending_queue_plan_admissions")
            .try_exists()
            .unwrap(),
        "rejected input must not even create the durable admission namespace"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn elapsed_deadline_and_invalid_certificates_fail_before_publication_wait() {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        iroha_core::state::World::default(),
        kura.clone(),
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    let wire = super::super::torii_proxy_test_deadline_unix_ms();
    let expired = PersistenceDeadline::new(
        Instant::now() - super::super::TORII_PROXY_EXECUTION_BUDGET,
        wire,
    );
    assert!(
        expired
            .persist(&state, b"invalid certificate")
            .await
            .unwrap_err()
            .contains("budget expired")
    );
    let live = PersistenceDeadline::new(Instant::now(), wire);
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        live.persist(&state, b"invalid certificate"),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(!error.contains("deadline"));
    assert!(
        !kura
            .store_root()
            .join("pending_queue_plan_admissions")
            .try_exists()
            .unwrap(),
        "rejected input must not even create the durable admission namespace"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_admission_wait_requires_exact_input_and_the_original_deadline() {
    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        iroha_core::state::World::default(),
        kura,
        iroha_core::query::store::LiveQueryStore::start_test(),
    );
    let wire = super::super::torii_proxy_test_deadline_unix_ms();
    let expired = PersistenceDeadline::new(
        Instant::now() - super::super::TORII_PROXY_EXECUTION_BUDGET,
        wire,
    );
    assert!(
        expired
            .wait_for_canonical_admission(&state, b"invalid complete input")
            .await
            .unwrap_err()
            .contains("budget expired")
    );
    let live = PersistenceDeadline::new(Instant::now(), wire);
    let error = live
        .wait_for_canonical_admission(&state, b"invalid complete input")
        .await
        .unwrap_err();
    assert!(error.contains("complete") || error.contains("admission"));
    assert!(!error.contains("deadline"));
}
