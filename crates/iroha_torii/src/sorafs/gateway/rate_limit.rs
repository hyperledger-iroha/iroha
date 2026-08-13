//! Lightweight rolling-window rate limiter for the SoraFS gateway.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use blake3::Hasher;
use dashmap::DashMap;
use parking_lot::Mutex;
use thiserror::Error;
const MAX_CLIENT_BUCKETS: usize = 4_096;
/// Fingerprint derived from client connection metadata (e.g., IP address).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClientFingerprint([u8; 32]);
impl ClientFingerprint {
    /// Construct a fingerprint directly from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Derive a fingerprint from an arbitrary identifier (remote IP, TLS session ID, etc.).
    #[must_use]
    pub fn from_identifier(identifier: &str) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(identifier.as_bytes());
        let mut out = [0u8; 32];
        out.copy_from_slice(hasher.finalize().as_bytes());
        Self(out)
    }
    /// Returns the canonical bytes representing the fingerprint.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
/// Configuration for the gateway rate limiter.
#[derive(Clone, Copy, Debug)]
pub struct GatewayRateLimitConfig {
    /// Maximum requests permitted within the rolling window. `None` disables limiting.
    pub max_requests: Option<u32>,
    /// Length of the rolling window used for accounting.
    pub window: Duration,
    /// Duration for which a client is temporarily banned after exceeding the limit.
    pub ban_duration: Option<Duration>,
}
impl GatewayRateLimitConfig {
    /// Returns a configuration with rate limits disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            max_requests: None,
            window: Duration::from_secs(1),
            ban_duration: None,
        }
    }
}
impl Default for GatewayRateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: Some(120),
            window: Duration::from_mins(1),
            ban_duration: Some(Duration::from_secs(30)),
        }
    }
}
/// Error returned when a client exceeds the configured limits.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitError {
    /// The client has exhausted the allowance for the current window.
    #[error("rate limited; retry after {retry_after:?}")]
    Limited {
        /// Suggested retry-after period.
        retry_after: Duration,
    },
    /// The client is temporarily banned after repeated violations.
    #[error("temporarily banned; retry after {retry_after:?}")]
    Banned {
        /// Optional retry-after period when a ban is active.
        retry_after: Option<Duration>,
    },
}
#[derive(Debug, Default)]
struct ClientWindow {
    events: VecDeque<Instant>,
    ban_until: Option<Instant>,
    last_seen: Option<Instant>,
}
/// Rolling-window rate limiter keyed by [`ClientFingerprint`].
#[derive(Debug)]
pub struct GatewayRateLimiter {
    config: GatewayRateLimitConfig,
    buckets: DashMap<ClientFingerprint, ClientWindow>,
    bucket_admission: Mutex<()>,
}
impl GatewayRateLimiter {
    /// Construct a rate limiter using the provided configuration.
    #[must_use]
    pub fn new(config: GatewayRateLimitConfig) -> Self {
        Self {
            config,
            buckets: DashMap::new(),
            bucket_admission: Mutex::new(()),
        }
    }
    /// Construct a rate limiter with the default configuration.
    #[must_use]
    pub fn new_default() -> Self {
        Self::new(GatewayRateLimitConfig::default())
    }
    /// Validates whether the client is permitted to perform another request.
    ///
    /// # Errors
    ///
    /// Returns [`RateLimitError`] if the client exceeds the configured allowance.
    pub fn check(&self, client: &ClientFingerprint, now: Instant) -> Result<(), RateLimitError> {
        let Some(max_requests) = self.config.max_requests else {
            return Ok(());
        };
        if let Some(mut window) = self.buckets.get_mut(client) {
            return self.check_window(&mut window, now, max_requests);
        }
        // Only first-seen fingerprints enter this critical section. It makes the hard bucket
        // bound race-free without serializing normal requests from known clients.
        let _admission = self.bucket_admission.lock();
        if let Some(mut window) = self.buckets.get_mut(client) {
            return self.check_window(&mut window, now, max_requests);
        }
        self.prune_inactive(now);
        if self.buckets.len() >= MAX_CLIENT_BUCKETS {
            // Preserve availability under high-cardinality ingress: after inactive state has
            // been reclaimed, replace the least-recently-seen subject rather than turning the
            // memory ceiling into a gateway-wide lockout. The fingerprint is transport-derived,
            // so a caller cannot rotate an opaque header to force this path.
            let oldest = self
                .buckets
                .iter()
                .min_by_key(|entry| entry.value().last_seen)
                .map(|entry| *entry.key());
            if let Some(oldest) = oldest {
                self.buckets.remove(&oldest);
            }
        }
        let mut window = ClientWindow::default();
        window.events.push_back(now);
        window.last_seen = Some(now);
        self.buckets.insert(*client, window);
        Ok(())
    }
    fn check_window(
        &self,
        window: &mut ClientWindow,
        now: Instant,
        max_requests: u32,
    ) -> Result<(), RateLimitError> {
        window.last_seen = Some(now);
        if let Some(ban_until) = window.ban_until {
            if ban_until > now {
                let retry_after = ban_until.saturating_duration_since(now);
                return Err(RateLimitError::Banned {
                    retry_after: Some(retry_after),
                });
            }
            window.ban_until = None;
        }
        while let Some(&front) = window.events.front() {
            if now.saturating_duration_since(front) > self.config.window {
                window.events.pop_front();
            } else {
                break;
            }
        }
        if window.events.len() as u32 >= max_requests {
            let retry_after = window
                .events
                .front()
                .map(|oldest| {
                    self.config
                        .window
                        .saturating_sub(now.saturating_duration_since(*oldest))
                })
                .unwrap_or_else(|| self.config.window);
            if let Some(ban_duration) = self.config.ban_duration {
                window.ban_until = Some(now + ban_duration);
                return Err(RateLimitError::Banned {
                    retry_after: Some(ban_duration),
                });
            }
            return Err(RateLimitError::Limited { retry_after });
        }
        window.events.push_back(now);
        Ok(())
    }
    fn prune_inactive(&self, now: Instant) {
        let inactive = self
            .buckets
            .iter()
            .filter_map(|entry| {
                let ban_active = entry.ban_until.is_some_and(|until| until > now);
                let event_active = entry.events.back().is_some_and(|event| {
                    now.saturating_duration_since(*event) <= self.config.window
                });
                (!ban_active && !event_active).then_some(*entry.key())
            })
            .collect::<Vec<_>>();
        for fingerprint in inactive {
            self.buckets.remove(&fingerprint);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_limiter_permits_within_budget() {
        let limiter = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(3),
            window: Duration::from_secs(5),
            ban_duration: None,
        });
        let client = ClientFingerprint::from_identifier("client-1");
        let start = Instant::now();
        assert!(limiter.check(&client, start).is_ok());
        assert!(
            limiter
                .check(&client, start + Duration::from_secs(1))
                .is_ok()
        );
        assert!(
            limiter
                .check(&client, start + Duration::from_secs(2))
                .is_ok()
        );
        // Fourth request exceeds limit.
        let err = limiter
            .check(&client, start + Duration::from_secs(3))
            .expect_err("expected limit to trigger");
        assert!(matches!(err, RateLimitError::Limited { .. }));
    }
    #[test]
    fn rate_limiter_bans_when_configured() {
        let limiter = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(1),
            window: Duration::from_secs(10),
            ban_duration: Some(Duration::from_secs(30)),
        });
        let client = ClientFingerprint::from_identifier("client-2");
        let now = Instant::now();
        assert!(limiter.check(&client, now).is_ok());
        let err = limiter
            .check(&client, now + Duration::from_millis(100))
            .expect_err("expected ban");
        assert!(matches!(err, RateLimitError::Banned { .. }));
    }
    #[test]
    fn rate_limiter_bounds_first_seen_client_state() {
        let limiter = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(1),
            window: Duration::from_secs(60),
            ban_duration: None,
        });
        let now = Instant::now();
        for index in 0..MAX_CLIENT_BUCKETS + 128 {
            let client = ClientFingerprint::from_identifier(&format!("client-{index}"));
            assert!(limiter.check(&client, now).is_ok());
        }
        assert_eq!(limiter.buckets.len(), MAX_CLIENT_BUCKETS);
    }
}
