//! Resolver event-stream framing, wakeup, lag, and subscription lifetime tests.

use std::{
    io,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

use axum::{body::to_bytes, response::IntoResponse};
use futures::{Stream, StreamExt};
use tokio::sync::broadcast;

use super::*;

#[derive(Default)]
struct WakeCounter(AtomicUsize);

impl Wake for WakeCounter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

fn resolver_added(resolver_id: &str) -> ResolverEvent {
    ResolverEvent::ResolverAdded {
        resolver_id: resolver_id.to_owned(),
    }
}

#[test]
fn sse_events_wakes_pending_receiver_and_preserves_payload_bytes() {
    let (sender, receiver) = broadcast::channel(2);
    let mut body = Sse::new(sse_events(receiver))
        .into_response()
        .into_body()
        .into_data_stream();
    let wakes = Arc::new(WakeCounter::default());
    let waker = Waker::from(wakes.clone());
    let mut context = Context::from_waker(&waker);
    assert!(Pin::new(&mut body).poll_next(&mut context).is_pending());

    sender.send(resolver_added("東京\"\n")).expect("send event");
    assert!(wakes.0.load(Ordering::Relaxed) > 0);
    let Poll::Ready(Some(Ok(bytes))) = Pin::new(&mut body).poll_next(&mut context) else {
        panic!("the awakened stream must deliver its event");
    };
    assert_eq!(
        bytes.as_ref(),
        "data: {\"event\":\"ResolverAdded\",\"value\":{\"resolver_id\":\"東京\\\"\\n\"}}\n\n"
            .as_bytes()
    );

    assert!(Pin::new(&mut body).poll_next(&mut context).is_pending());
    let previous_wakes = wakes.0.load(Ordering::Relaxed);
    drop(sender);
    assert!(wakes.0.load(Ordering::Relaxed) > previous_wakes);
    assert!(matches!(
        Pin::new(&mut body).poll_next(&mut context),
        Poll::Ready(None)
    ));
}

#[tokio::test]
async fn sse_events_drains_buffered_events_in_order_before_closing() {
    for ids in [vec![], vec!["first", "second"]] {
        let (sender, receiver) = broadcast::channel(2);
        for id in &ids {
            sender.send(resolver_added(id)).expect("send event");
        }
        drop(sender);
        let body = Sse::new(sse_events(receiver)).into_response().into_body();
        let bytes = to_bytes(body, 1024).await.expect("read event body");
        let expected = if ids.is_empty() {
            ""
        } else {
            concat!(
                "data: {\"event\":\"ResolverAdded\",\"value\":{\"resolver_id\":\"first\"}}\n\n",
                "data: {\"event\":\"ResolverAdded\",\"value\":{\"resolver_id\":\"second\"}}\n\n"
            )
        };
        assert_eq!(bytes.as_ref(), expected.as_bytes());
    }
}

#[derive(Clone, Default)]
struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturedLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("capture log").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn sse_events_logs_lag_and_recovers_the_oldest_retained_event() {
    let log = CapturedLog::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let (sender, receiver) = broadcast::channel(2);
    for id in [
        "lost-first",
        "lost-second",
        "retained-first",
        "retained-second",
    ] {
        sender.send(resolver_added(id)).expect("send event");
    }
    drop(sender);
    let body = Sse::new(sse_events(receiver)).into_response().into_body();
    let bytes = to_bytes(body, 1024).await.expect("read retained events");
    assert_eq!(
        bytes.as_ref(),
        concat!(
            "data: {\"event\":\"ResolverAdded\",\"value\":{\"resolver_id\":\"retained-first\"}}\n\n",
            "data: {\"event\":\"ResolverAdded\",\"value\":{\"resolver_id\":\"retained-second\"}}\n\n"
        )
        .as_bytes()
    );
    let log = String::from_utf8(log.0.lock().expect("read log").clone()).expect("UTF-8 log");
    assert_eq!(
        log.matches("SSE consumer lagged; dropping 2 events")
            .count(),
        1
    );
    assert!(log.contains("lagged=2"), "{log}");
}

#[test]
fn sse_events_drop_releases_unpolled_and_pending_subscriptions() {
    for poll in [false, true] {
        let (sender, receiver) = broadcast::channel(2);
        let mut stream = Box::pin(sse_events(receiver));
        assert_eq!(sender.receiver_count(), 1);
        if poll {
            let mut context = Context::from_waker(Waker::noop());
            assert!(stream.as_mut().poll_next(&mut context).is_pending());
        }
        drop(stream);
        assert_eq!(sender.receiver_count(), 0);
        assert!(sender.send(resolver_added("unobserved")).is_err());
    }
}

#[test]
fn sse_events_remains_closed_when_polled_again() {
    let (sender, receiver) = broadcast::channel(2);
    let mut stream = std::pin::pin!(sse_events(receiver));
    let mut context = Context::from_waker(Waker::noop());
    drop(sender);
    for _ in 0..3 {
        assert!(matches!(
            stream.as_mut().poll_next(&mut context),
            Poll::Ready(None)
        ));
    }
}

#[tokio::test]
async fn sse_handler_wires_events_and_closure_with_response_headers() {
    let emitter = EventEmitter::new("resolver.test", None).expect("event emitter");
    let response = sse_handler(AxumState(emitter.clone()))
        .await
        .into_response();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
    let mut diff = crate::state::ResolverDiff::default();
    diff.added.push("resolver.test".into());
    emitter.emit_resolver_diff(&diff);
    drop(emitter);
    let mut body = response.into_body().into_data_stream();
    let bytes = timeout(Duration::from_secs(1), body.next())
        .await
        .expect("event arrives before keepalive")
        .expect("event frame")
        .expect("event bytes");
    assert_eq!(
        bytes.as_ref(),
        b"data: {\"event\":\"ResolverAdded\",\"value\":{\"resolver_id\":\"resolver.test\"}}\n\n"
    );
    assert!(
        timeout(Duration::from_secs(1), body.next())
            .await
            .expect("closed emitter ends stream")
            .is_none()
    );
}
