//! Lease ownership through physical storage work and application body consumption.

use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{Body, Bytes},
    response::Response,
};
use futures::Stream;
use tokio::time::Sleep;

use super::{
    SharedAppState, StatusCode, json_error, node_storage_error_response, record_storage_metrics,
    sorafs_heavy_blocking_task,
    stream_token_enforcement::{RangeFetchConcurrencyGuard, RangeFetchLeaseWindow},
};
use crate::ShutdownSignal;

const RESPONSE_CHUNK_BYTES: usize = 64 * 1024;

fn lease_window_is_active(window: Option<RangeFetchLeaseWindow>) -> bool {
    let Some(window) = window else {
        // Only the existing test-local admission owner omits an external lease.
        return cfg!(test);
    };
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    let Ok(now_ms) = u64::try_from(now.as_millis()) else {
        return false;
    };
    now_ms >= window.validated_at_unix_ms
        && now_ms < window.expires_at_unix_ms
        && Instant::now() < window.monotonic_deadline
}

/// Refuse response production once the externally authenticated window closes.
pub(super) fn ensure_stream_token_lease_active(
    guard: &RangeFetchConcurrencyGuard,
) -> Result<(), Response> {
    if lease_window_is_active(guard.lease_window()) {
        Ok(())
    } else {
        let mut response = json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "stream token lease expired before response production",
        );
        response.headers_mut().insert(
            http::header::RETRY_AFTER,
            http::HeaderValue::from_static("1"),
        );
        Err(response)
    }
}

/// Keep the lease with the actual storage worker even if its HTTP waiter leaves.
pub(super) async fn read_chunk_with_stream_token_lease(
    state: &SharedAppState,
    manifest_id: String,
    digest: [u8; 32],
    guard: RangeFetchConcurrencyGuard,
) -> Result<(RangeFetchConcurrencyGuard, Vec<u8>), Response> {
    let worker_state = state.clone();
    sorafs_heavy_blocking_task(state, "SoraFS chunk read", move || {
        ensure_stream_token_lease_active(&guard)?;
        let (_, bytes) = worker_state
            .sorafs_node
            .read_chunk_by_digest(&manifest_id, &digest)
            .map_err(node_storage_error_response)?;
        record_storage_metrics(&worker_state);
        ensure_stream_token_lease_active(&guard)?;
        Ok((guard, bytes))
    })
    .await
}

/// Retain the accepted lease until application-body EOF, error or cancellation.
///
/// Checks stop subsequent body production; bytes already given to Hyper or the
/// socket cannot be revoked. An unpolled body conservatively retains its ticket.
pub(super) fn stream_token_response_body(
    guard: RangeFetchConcurrencyGuard,
    bytes: Bytes,
    shutdown: ShutdownSignal,
) -> Body {
    let window = guard.lease_window();
    Body::from_stream(LeaseBoundBytes::new(guard, bytes, window, shutdown))
}

struct LeaseBoundBytes<G> {
    guard: Option<G>,
    bytes: Bytes,
    offset: usize,
    window: Option<RangeFetchLeaseWindow>,
    expiry: Option<Pin<Box<Sleep>>>,
    shutdown: Pin<Box<dyn Future<Output = ()> + Send>>,
    finished: bool,
}

impl<G> LeaseBoundBytes<G> {
    fn new(
        guard: G,
        bytes: Bytes,
        window: Option<RangeFetchLeaseWindow>,
        shutdown: ShutdownSignal,
    ) -> Self {
        let expiry = window.map(|window| {
            Box::pin(tokio::time::sleep_until(tokio::time::Instant::from_std(
                window.monotonic_deadline,
            )))
        });
        Self {
            guard: Some(guard),
            bytes,
            offset: 0,
            window,
            expiry,
            shutdown: Box::pin(async move { shutdown.receive().await }),
            finished: false,
        }
    }

    fn finish(&mut self) {
        self.finished = true;
        self.bytes = Bytes::new();
        self.expiry = None;
        drop(self.guard.take());
    }

    fn fail(
        &mut self,
        kind: io::ErrorKind,
        message: &'static str,
    ) -> Poll<Option<io::Result<Bytes>>> {
        self.finish();
        Poll::Ready(Some(Err(io::Error::new(kind, message))))
    }
}

impl<G: Unpin> Stream for LeaseBoundBytes<G> {
    type Item = io::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(None);
        }
        if this.shutdown.as_mut().poll(cx).is_ready() {
            return this.fail(
                io::ErrorKind::Interrupted,
                "stream token response stopped by shutdown",
            );
        }
        // Poll the timer even while bytes are ready so backpressure cannot erase
        // the registered expiry wake-up. The validation-time deadline is never reset.
        if this
            .expiry
            .as_mut()
            .is_some_and(|timer| timer.as_mut().poll(cx).is_ready())
            || !lease_window_is_active(this.window)
        {
            return this.fail(
                io::ErrorKind::TimedOut,
                "stream token response lease expired",
            );
        }
        if this.offset == this.bytes.len() {
            this.finish();
            return Poll::Ready(None);
        }
        let end = this
            .offset
            .saturating_add(RESPONSE_CHUNK_BYTES)
            .min(this.bytes.len());
        let bytes = this.bytes.slice(this.offset..end);
        this.offset = end;
        Poll::Ready(Some(Ok(bytes)))
    }
}

#[cfg(test)]
#[path = "stream_token_body_tests.rs"]
mod tests;
