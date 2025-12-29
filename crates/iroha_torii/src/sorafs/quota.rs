//! Per-token request quota tracking for SoraFS stream tokens.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::{DashMap, mapref::entry::Entry};

const QUOTA_WINDOW: Duration = Duration::from_mins(1);

/// Tracks the number of requests served under each stream token.
#[derive(Clone, Default)]
pub struct StreamTokenQuotaTracker {
    inner: Arc<StreamTokenQuotaInner>,
}

impl StreamTokenQuotaTracker {
    /// Attempt to record a request for `token_id` within the configured quota window.
    ///
    /// When `requests_per_minute` is `0`, the token is treated as unlimited and no state is
    /// retained. Otherwise this method enforces a 60-second rolling window and returns
    /// [`StreamTokenQuotaExceeded`] when the new request would exceed the quota.
    ///
    /// # Errors
    ///
    /// Returns [`StreamTokenQuotaExceeded`] when the quota for `token_id` has already been met.
    pub fn try_acquire(
        &self,
        token_id: &str,
        requests_per_minute: u32,
    ) -> Result<(), StreamTokenQuotaExceeded> {
        if requests_per_minute == 0 {
            self.inner.remove(token_id);
            return Ok(());
        }
        self.inner
            .try_acquire(token_id, requests_per_minute, Instant::now())
    }

    #[cfg(test)]
    pub(crate) fn try_acquire_at(
        &self,
        token_id: &str,
        requests_per_minute: u32,
        now: Instant,
    ) -> Result<(), StreamTokenQuotaExceeded> {
        if requests_per_minute == 0 {
            self.inner.remove(token_id);
            return Ok(());
        }
        self.inner.try_acquire(token_id, requests_per_minute, now)
    }
}

#[derive(Default)]
struct StreamTokenQuotaInner {
    windows: DashMap<String, TokenQuotaWindow>,
}

impl StreamTokenQuotaInner {
    fn try_acquire(
        &self,
        token_id: &str,
        requests_per_minute: u32,
        now: Instant,
    ) -> Result<(), StreamTokenQuotaExceeded> {
        match self.windows.entry(token_id.to_owned()) {
            Entry::Occupied(mut entry) => entry.get_mut().consume(requests_per_minute, now),
            Entry::Vacant(entry) => {
                entry.insert(TokenQuotaWindow::new(requests_per_minute, now));
                Ok(())
            }
        }
    }

    fn remove(&self, token_id: &str) {
        self.windows.remove(token_id);
    }
}

#[derive(Debug, Clone)]
struct TokenQuotaWindow {
    started_at: Instant,
    limit: u32,
    used: u32,
}

impl TokenQuotaWindow {
    fn new(limit: u32, now: Instant) -> Self {
        Self {
            started_at: now,
            limit,
            used: 1,
        }
    }

    fn reset(&mut self, limit: u32, now: Instant) {
        self.started_at = now;
        self.limit = limit;
        self.used = 0;
    }

    fn consume(&mut self, limit: u32, now: Instant) -> Result<(), StreamTokenQuotaExceeded> {
        let elapsed = now.saturating_duration_since(self.started_at);
        if elapsed >= QUOTA_WINDOW || self.limit != limit {
            self.reset(limit, now);
        }

        if self.used >= limit {
            let wait = QUOTA_WINDOW
                .saturating_sub(elapsed.min(QUOTA_WINDOW))
                .as_secs()
                .saturating_add(1);
            let retry_after_secs = wait.clamp(1, u64::from(u32::MAX)) as u32;
            return Err(StreamTokenQuotaExceeded { retry_after_secs });
        }

        self.used = self.used.saturating_add(1);
        Ok(())
    }
}

/// Error returned when the stream-token request quota would be exceeded.
#[derive(Debug, Clone, Copy)]
pub struct StreamTokenQuotaExceeded {
    /// Seconds until the quota window resets.
    pub retry_after_secs: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_allows_within_limit() {
        let tracker = StreamTokenQuotaTracker::default();
        let start = Instant::now();
        tracker
            .try_acquire_at("token", 3, start)
            .expect("first request allowed");
        tracker
            .try_acquire_at("token", 3, start + Duration::from_secs(1))
            .expect("second request allowed");
        tracker
            .try_acquire_at("token", 3, start + Duration::from_secs(2))
            .expect("third request allowed");
        let err = tracker
            .try_acquire_at("token", 3, start + Duration::from_secs(3))
            .expect_err("fourth request should exceed quota");
        assert!(err.retry_after_secs <= 60);
    }

    #[test]
    fn quota_resets_after_window() {
        let tracker = StreamTokenQuotaTracker::default();
        let start = Instant::now();
        tracker
            .try_acquire_at("token", 1, start)
            .expect("first request allowed");
        let err = tracker
            .try_acquire_at("token", 1, start + Duration::from_secs(10))
            .expect_err("second request within window denied");
        assert!(err.retry_after_secs >= 1);
        tracker
            .try_acquire_at("token", 1, start + Duration::from_secs(65))
            .expect("quota resets after window elapses");
    }

    #[test]
    fn unlimited_quota_clears_state() {
        let tracker = StreamTokenQuotaTracker::default();
        let start = Instant::now();
        tracker
            .try_acquire_at("token", 2, start)
            .expect("limited request allowed");
        tracker
            .try_acquire_at("token", 0, start + Duration::from_secs(10))
            .expect("unlimited quota clears state");
        tracker
            .try_acquire_at("token", 2, start + Duration::from_secs(11))
            .expect("state re-initialised after unlimited quota");
    }
}
