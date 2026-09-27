//! Time-based token budgets and typed transport failure classification for retrieval.
use super::{AttemptFailure, FetchProvider};
use std::{error::Error, time::Duration};
use tokio::time::Instant;

pub(super) async fn sleep_until(deadline: Instant) {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::time::sleep_until(deadline).await;
    } else {
        futures_timer::Delay::new(deadline.saturating_duration_since(Instant::now())).await;
    }
}

#[derive(Clone)]
pub(super) struct ProviderRateWindow {
    byte_limit: Option<u64>,
    request_limit: Option<u32>,
    byte_start: Instant,
    request_start: Instant,
    bytes: u64,
    requests: u32,
    cooldown: Option<Instant>,
}
impl ProviderRateWindow {
    pub(super) fn new(provider: &FetchProvider) -> Self {
        let now = Instant::now();
        Self {
            byte_limit: provider
                .metadata()
                .and_then(|metadata| {
                    metadata
                        .stream_budget
                        .as_ref()
                        .map(|budget| budget.max_bytes_per_sec)
                })
                .filter(|limit| *limit != 0),
            request_limit: provider
                .metadata()
                .and_then(|metadata| metadata.requests_per_minute),
            byte_start: now,
            request_start: now,
            bytes: 0,
            requests: 0,
            cooldown: None,
        }
    }
    pub(super) fn ready_at(&self, bytes: u64, now: Instant) -> Instant {
        let mut ready = self.cooldown.unwrap_or(now).max(now);
        if let Some(limit) = self.byte_limit
            && now < self.byte_start + Duration::from_secs(1)
            && self.bytes.saturating_add(bytes) > limit
        {
            ready = ready.max(self.byte_start + Duration::from_secs(1));
        }
        if let Some(limit) = self.request_limit
            && now < self.request_start + Duration::from_secs(60)
            && self.requests >= limit
        {
            ready = ready.max(self.request_start + Duration::from_secs(60));
        }
        ready
    }
    pub(super) fn reserve(&mut self, bytes: u64, now: Instant) {
        if now >= self.byte_start + Duration::from_secs(1) {
            self.byte_start = now;
            self.bytes = 0;
        }
        if now >= self.request_start + Duration::from_secs(60) {
            self.request_start = now;
            self.requests = 0;
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.requests = self.requests.saturating_add(1);
    }
    pub(super) fn throttle(&mut self, delay: Duration) {
        // A malicious retry delay cannot overflow an Instant; session admission has its own
        // absolute deadline and never sleeps beyond it.
        self.cooldown = Instant::now()
            .checked_add(delay.clamp(Duration::from_millis(1), Duration::from_secs(86_400)));
    }
}

pub(super) fn classify_provider_error(
    error: &(dyn Error + 'static),
) -> (AttemptFailure, Option<Duration>) {
    #[cfg(feature = "manifest")]
    {
        let mut current = Some(error);
        // Preserve typed material through honest wrappers, while bounding malformed source chains.
        for _ in 0..16 {
            let Some(cause) = current else { break };
            if let Some(gateway) = cause.downcast_ref::<crate::gateway::GatewayFetchError>() {
                let retry = match gateway {
                    crate::gateway::GatewayFetchError::RateLimited { retry_after, .. } => {
                        Some(*retry_after)
                    }
                    _ => None,
                };
                return (AttemptFailure::from(gateway), retry);
            }
            current = cause.source();
        }
    }
    (
        AttemptFailure::Provider {
            message: error.to_string(),
            policy_block: None,
        },
        None,
    )
}
