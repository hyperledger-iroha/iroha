//! Fixed authenticated lease-time bounds; these checks do not authenticate a raw record.
use super::*;
use crate::sorafs::stream_token_cleanup::test_support::{fixture, request};

fn current_record() -> StreamTokenGatewayAdmissionRecordV1 {
    let (_, _, capture) = fixture();
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let mut request = request("window-valid-record");
    request.validated_at_unix_ms = now;
    let quota = request.quota.as_mut().unwrap();
    quota.observed_at_epoch = now / 1_000;
    quota.expires_at_epoch = now / 1_000 + 600;
    capture.admit(&request).unwrap()
}

#[test]
fn window_retains_record_times_and_original_monotonic_anchor() {
    let monotonic_anchor = std::time::Instant::now();
    let record = current_record();
    let window = RangeFetchLeaseWindow::from_record(&record, monotonic_anchor).unwrap();
    assert_eq!(
        window.validated_at_unix_ms,
        record.outcome.validated_at_unix_ms
    );
    assert_eq!(
        window.expires_at_unix_ms,
        record.lease_expires_at_unix_ms.unwrap()
    );
    assert_eq!(
        window.monotonic_deadline,
        monotonic_anchor
            .checked_add(std::time::Duration::from_millis(
                window.expires_at_unix_ms - window.validated_at_unix_ms
            ),)
            .unwrap()
    );
    let again = RangeFetchLeaseWindow::from_record(&record, monotonic_anchor).unwrap();
    assert_eq!(
        again.monotonic_deadline, window.monotonic_deadline,
        "reconstruction never renews the relative lifetime"
    );
}

#[test]
fn invalid_or_elapsed_record_windows_fail_closed_without_a_new_relative_lifetime() {
    let record = current_record();
    let anchor = std::time::Instant::now();
    assert!(RangeFetchLeaseWindow::from_record(&record, anchor).is_ok());
    let mut missing = record;
    missing.lease_expires_at_unix_ms = None;
    let mut equal = record;
    equal.lease_expires_at_unix_ms = Some(record.outcome.validated_at_unix_ms);
    let mut before = record;
    before.lease_expires_at_unix_ms = Some(record.outcome.validated_at_unix_ms - 1);
    let mut elapsed = record;
    elapsed.outcome.validated_at_unix_ms = 1;
    elapsed.lease_expires_at_unix_ms = Some(2);
    let mut future = record;
    future.outcome.validated_at_unix_ms += 60_000;
    future.lease_expires_at_unix_ms = Some(future.outcome.validated_at_unix_ms + 60_000);
    for invalid in [missing, equal, before, elapsed, future] {
        let error = RangeFetchLeaseWindow::from_record(&invalid, anchor).unwrap_err();
        assert_eq!(error.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.headers().get(RETRY_AFTER).unwrap(), "1");
    }
    let old_anchor = anchor
        .checked_sub(std::time::Duration::from_secs(600))
        .unwrap();
    assert!(
        RangeFetchLeaseWindow::from_record(&record, old_anchor).is_err(),
        "slow admission cannot start another lifetime at response creation"
    );
}
