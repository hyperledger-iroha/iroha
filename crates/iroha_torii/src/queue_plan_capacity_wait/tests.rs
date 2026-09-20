//! Closed-owner, terminal-error, clock, and cancellation controls.
use std::cell::Cell;

use iroha_core::sumeragi::AdmissionCapacityUnavailableV1;

use super::*;

#[tokio::test(start_paused = true)]
async fn closed_owner_waits_and_rechecks_until_activation() {
    let started = tokio::time::Instant::now();
    let checks = Cell::new(0);
    let deadline = started + Duration::from_secs(1);
    let wire = crate::torii_proxy_test_deadline_unix_ms();
    wait(
        || {
            checks.set(checks.get() + 1);
            if tokio::time::Instant::now() < started + Duration::from_millis(100) {
                Err(QueuePlanInputCapacityErrorV1::Inactive)
            } else {
                Ok(())
            }
        },
        || remaining(deadline, wire),
    )
    .await
    .unwrap();
    assert_eq!(checks.get(), 5);
    assert_eq!(
        tokio::time::Instant::now() - started,
        Duration::from_millis(100)
    );
}

#[tokio::test(start_paused = true)]
async fn only_inactive_is_waited_and_terminal_change_is_immediate() {
    for error in [
        QueuePlanInputCapacityErrorV1::Unavailable(AdmissionCapacityUnavailableV1::Pending),
        QueuePlanInputCapacityErrorV1::Unavailable(AdmissionCapacityUnavailableV1::Disabled),
        QueuePlanInputCapacityErrorV1::Unavailable(AdmissionCapacityUnavailableV1::RestartRequired),
        QueuePlanInputCapacityErrorV1::Invalid("changed binding".to_owned()),
        QueuePlanInputCapacityErrorV1::Oversized {
            envelope: "native",
            required: 2,
            capacity: 1,
        },
        QueuePlanInputCapacityErrorV1::Availability("changed geometry".to_owned()),
    ] {
        let started = tokio::time::Instant::now();
        let mut error = Some(error);
        let result = wait(|| Err(error.take().unwrap()), || Ok(Duration::from_secs(1))).await;
        assert!(matches!(result, Err(WaitError::Capacity(_))));
        assert_eq!(tokio::time::Instant::now(), started);
    }
    let checks = Cell::new(0);
    let started = tokio::time::Instant::now();
    let result = wait(
        || {
            checks.set(checks.get() + 1);
            Err(if checks.get() == 1 {
                QueuePlanInputCapacityErrorV1::Inactive
            } else {
                QueuePlanInputCapacityErrorV1::Unavailable(
                    AdmissionCapacityUnavailableV1::RestartRequired,
                )
            })
        },
        || Ok(Duration::from_secs(1)),
    )
    .await;
    assert!(matches!(
        result,
        Err(WaitError::Capacity(
            QueuePlanInputCapacityErrorV1::Unavailable(
                AdmissionCapacityUnavailableV1::RestartRequired
            )
        ))
    ));
    assert_eq!(checks.get(), 2);
    assert_eq!(
        tokio::time::Instant::now() - started,
        Duration::from_millis(25)
    );
}

#[tokio::test(start_paused = true)]
async fn original_monotonic_and_wire_deadlines_are_not_renewed() {
    let wire = crate::torii_proxy_test_deadline_unix_ms();
    let started = tokio::time::Instant::now();
    let deadline = started + Duration::from_millis(60);
    let checks = Cell::new(0);
    let result = wait(
        || {
            checks.set(checks.get() + 1);
            Err(QueuePlanInputCapacityErrorV1::Inactive)
        },
        || remaining(deadline, wire),
    )
    .await;
    assert!(matches!(result, Err(WaitError::Deadline(_))));
    assert_eq!(tokio::time::Instant::now(), deadline);
    assert_eq!(checks.get(), 3);
    let expired_wire = crate::torii_proxy_now_unix_ms().unwrap().saturating_sub(1);
    let result = wait(
        || panic!("expired wire must fail before capacity work"),
        || remaining(deadline + Duration::from_secs(10), expired_wire),
    )
    .await;
    assert!(matches!(result, Err(WaitError::Deadline(_))));
}

#[tokio::test(start_paused = true)]
async fn cancellation_drops_wait_without_detached_checks() {
    let checks = Cell::new(0);
    let mut future = Box::pin(wait(
        || {
            checks.set(checks.get() + 1);
            Err(QueuePlanInputCapacityErrorV1::Inactive)
        },
        || Ok(Duration::from_secs(1)),
    ));
    assert!(futures_util::poll!(&mut future).is_pending());
    assert_eq!(checks.get(), 1);
    drop(future);
    tokio::time::advance(Duration::from_secs(2)).await;
    assert_eq!(checks.get(), 1);
}
