//! Actual response-stream ownership, expiry and wake-up regressions.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Wake, Waker},
    time::Duration,
};

use futures::StreamExt as _;

use super::*;

struct CountedLease(Arc<AtomicUsize>);

impl Drop for CountedLease {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn window() -> RangeFetchLeaseWindow {
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    RangeFetchLeaseWindow {
        validated_at_unix_ms: now.saturating_sub(1_000),
        expires_at_unix_ms: now + 60_000,
        monotonic_deadline: Instant::now() + Duration::from_secs(60),
    }
}

fn response_stream(
    bytes: Bytes,
    window: RangeFetchLeaseWindow,
    shutdown: ShutdownSignal,
) -> (LeaseBoundBytes<CountedLease>, Arc<AtomicUsize>) {
    let releases = Arc::new(AtomicUsize::new(0));
    (
        LeaseBoundBytes::new(
            CountedLease(releases.clone()),
            bytes,
            Some(window),
            shutdown,
        ),
        releases,
    )
}

#[tokio::test]
async fn lease_body_preserves_every_byte_and_owns_lease_until_eof() {
    let original = Bytes::from(
        (0_u8..=255)
            .cycle()
            .take(RESPONSE_CHUNK_BYTES * 2 + 17)
            .collect::<Vec<_>>(),
    );
    let (mut body, releases) = response_stream(original.clone(), window(), ShutdownSignal::new());
    let mut actual = Vec::new();
    for expected_size in [RESPONSE_CHUNK_BYTES, RESPONSE_CHUNK_BYTES, 17] {
        let frame = body.next().await.unwrap().unwrap();
        assert_eq!(frame.len(), expected_size);
        actual.extend_from_slice(&frame);
        assert_eq!(
            releases.load(Ordering::SeqCst),
            0,
            "even the final emitted frame still owns the lease"
        );
    }
    assert_eq!(actual.as_slice(), original.as_ref());
    assert!(body.next().await.is_none());
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    assert!(body.next().await.is_none());
    drop(body);
    assert_eq!(releases.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn lease_body_drop_releases_once_before_poll_or_after_partial_consumption() {
    for consume_frame in [false, true] {
        let (mut body, releases) = response_stream(
            Bytes::from(vec![0xA3; RESPONSE_CHUNK_BYTES + 1]),
            window(),
            ShutdownSignal::new(),
        );
        assert_eq!(releases.load(Ordering::SeqCst), 0);
        if consume_frame {
            assert_eq!(
                body.next().await.unwrap().unwrap().len(),
                RESPONSE_CHUNK_BYTES
            );
        }
        assert_eq!(releases.load(Ordering::SeqCst), 0);
        drop(body);
        assert_eq!(releases.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn lease_body_empty_payload_releases_only_when_consumed() {
    let (mut body, releases) = response_stream(Bytes::new(), window(), ShutdownSignal::new());
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    assert!(body.next().await.is_none());
    assert_eq!(releases.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn lease_body_rejects_clock_rollback_and_each_expiry_clock_independently() {
    for fault in 0..3 {
        let mut deadline = window();
        match fault {
            0 => deadline.validated_at_unix_ms = deadline.expires_at_unix_ms - 1,
            1 => deadline.expires_at_unix_ms = deadline.validated_at_unix_ms,
            _ => deadline.monotonic_deadline = Instant::now(),
        }
        assert!(!lease_window_is_active(Some(deadline)), "fault {fault}");
        let (mut body, releases) = response_stream(
            Bytes::from_static(b"must not be emitted"),
            deadline,
            ShutdownSignal::new(),
        );
        assert_eq!(
            body.next().await.unwrap().unwrap_err().kind(),
            io::ErrorKind::TimedOut,
            "fault {fault}"
        );
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        assert!(body.next().await.is_none());
        drop(body);
        assert_eq!(releases.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn lease_body_shutdown_after_a_frame_stops_further_production() {
    let shutdown = ShutdownSignal::new();
    let (mut body, releases) = response_stream(
        Bytes::from(vec![0xCA; RESPONSE_CHUNK_BYTES + 1]),
        window(),
        shutdown.clone(),
    );
    assert!(body.next().await.unwrap().is_ok());
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    shutdown.send();
    assert_eq!(
        body.next().await.unwrap().unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    assert!(body.next().await.is_none());
}

#[tokio::test]
async fn lease_body_preexisting_shutdown_emits_no_payload() {
    let shutdown = ShutdownSignal::new();
    shutdown.send();
    let (mut body, releases) = response_stream(Bytes::from_static(b"withheld"), window(), shutdown);
    assert_eq!(
        body.next().await.unwrap().unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    assert!(body.next().await.is_none());
}

struct ExpiryWake {
    wakes: AtomicUsize,
    changed: tokio::sync::Notify,
}

impl Wake for ExpiryWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        self.changed.notify_one();
    }
}

#[tokio::test]
async fn lease_body_expiry_wakes_backpressured_consumer_without_resetting_deadline() {
    let mut deadline = window();
    deadline.monotonic_deadline = Instant::now() + Duration::from_millis(500);
    let (mut body, releases) = response_stream(
        Bytes::from(vec![0x5A; RESPONSE_CHUNK_BYTES + 1]),
        deadline,
        ShutdownSignal::new(),
    );
    let wake = Arc::new(ExpiryWake {
        wakes: AtomicUsize::new(0),
        changed: tokio::sync::Notify::new(),
    });
    let waker = Waker::from(wake.clone());
    let mut cx = Context::from_waker(&waker);
    assert!(matches!(
        Pin::new(&mut body).poll_next(&mut cx),
        Poll::Ready(Some(Ok(_)))
    ));
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let changed = wake.changed.notified();
            if wake.wakes.load(Ordering::SeqCst) != 0
                && Instant::now() >= deadline.monotonic_deadline
            {
                break;
            }
            changed.await;
        }
    })
    .await
    .expect("the original expiry timer must wake the body consumer");
    assert!(Instant::now() >= deadline.monotonic_deadline);
    assert_eq!(
        releases.load(Ordering::SeqCst),
        0,
        "a wake cannot consume or retract bytes for the HTTP owner"
    );
    assert_eq!(
        body.next().await.unwrap().unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    assert!(body.next().await.is_none());
}
