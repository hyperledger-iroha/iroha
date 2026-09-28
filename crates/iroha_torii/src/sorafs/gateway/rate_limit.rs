//! Constant-memory token budgets for the SoraFS gateway.
use blake3::Hasher;
use dashmap::DashMap;
use parking_lot::Mutex;
use std::time::{Duration, Instant};
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
    /// Maximum burst and tokens replenished per window. `None` disables limiting.
    pub max_requests: Option<u32>,
    /// Time required to replenish the complete token budget.
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
            max_requests:
                iroha_config::parameters::defaults::sorafs::gateway::rate_limit::MAX_REQUESTS,
            window: iroha_config::parameters::defaults::sorafs::gateway::rate_limit::WINDOW,
            ban_duration: iroha_config::parameters::defaults::sorafs::gateway::rate_limit::BAN,
        }
    }
}
/// Error returned when a client exceeds the configured limits.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitError {
    /// The client has exhausted its token budget.
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
#[derive(Debug)]
struct ClientBucket {
    // One request costs window.as_nanos() credits; each elapsed nanosecond adds
    // max_requests credits. This retains fractional tokens without floating point.
    credits: u128,
    last_refill: Instant,
    ban_started: Option<Instant>,
    last_seen: Instant,
}
/// Constant-memory token bucket keyed by [`ClientFingerprint`].
#[derive(Debug)]
pub struct GatewayRateLimiter {
    config: GatewayRateLimitConfig,
    buckets: DashMap<ClientFingerprint, ClientBucket>,
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
        if let Some(mut bucket) = self.buckets.get_mut(client) {
            return self.check_bucket(&mut bucket, now, max_requests);
        }
        // Only first-seen fingerprints enter this critical section. It makes the hard bucket
        // bound race-free without serializing normal requests from known clients.
        let _admission = self.bucket_admission.lock();
        if let Some(mut bucket) = self.buckets.get_mut(client) {
            return self.check_bucket(&mut bucket, now, max_requests);
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
        let mut bucket = ClientBucket {
            credits: self.capacity(max_requests),
            last_refill: now,
            ban_started: None,
            last_seen: now,
        };
        let result = self.check_bucket(&mut bucket, now, max_requests);
        self.buckets.insert(*client, bucket);
        result
    }
    fn request_cost(&self) -> u128 {
        // Normalize a zero interval to a nanosecond, keeping the credit cost
        // positive without division by zero.
        self.config.window.as_nanos().max(1)
    }
    fn capacity(&self, max_requests: u32) -> u128 {
        self.request_cost().saturating_mul(u128::from(max_requests))
    }
    fn active_ban(&self, bucket: &ClientBucket, now: Instant) -> Option<Duration> {
        let duration = self.config.ban_duration?;
        let elapsed = now.saturating_duration_since(bucket.ban_started?);
        (elapsed < duration).then(|| duration.saturating_sub(elapsed))
    }
    fn check_bucket(
        &self,
        bucket: &mut ClientBucket,
        now: Instant,
        max_requests: u32,
    ) -> Result<(), RateLimitError> {
        // Concurrent callers can acquire the bucket out of timestamp order. Never
        // refill twice over the same interval or shorten a ban for those callers.
        let now = now.max(bucket.last_seen);
        bucket.last_seen = now;
        if let Some(retry_after) = self.active_ban(bucket, now) {
            return Err(RateLimitError::Banned {
                retry_after: Some(retry_after),
            });
        }
        bucket.ban_started = None;
        let elapsed = now.saturating_duration_since(bucket.last_refill);
        let replenished = elapsed.as_nanos().saturating_mul(u128::from(max_requests));
        bucket.credits = bucket
            .credits
            .saturating_add(replenished)
            .min(self.capacity(max_requests));
        bucket.last_refill = now;
        let cost = self.request_cost();
        if bucket.credits >= cost {
            bucket.credits -= cost;
            return Ok(());
        }
        if let Some(ban_duration) = self.config.ban_duration {
            // Store the starting instant, avoiding Instant + Duration overflow for
            // large configured bans while retaining exact expiry semantics.
            bucket.ban_started = Some(now);
            return Err(RateLimitError::Banned {
                retry_after: Some(ban_duration),
            });
        }
        let retry_after = if max_requests == 0 {
            self.config.window.max(Duration::from_nanos(1))
        } else {
            let nanos = (cost - bucket.credits).div_ceil(u128::from(max_requests));
            // The wait is at most one configured window, so it fits Duration.
            Duration::new(
                u64::try_from(nanos / 1_000_000_000).unwrap_or(u64::MAX),
                (nanos % 1_000_000_000) as u32,
            )
        };
        Err(RateLimitError::Limited { retry_after })
    }
    fn prune_inactive(&self, now: Instant) {
        self.buckets.retain(|_, bucket| {
            self.active_ban(bucket, now).is_some()
                || now.saturating_duration_since(bucket.last_seen) <= self.config.window
        });
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
        for _ in 0..3 {
            assert!(limiter.check(&client, start).is_ok());
        }
        assert_eq!(
            limiter.check(&client, start),
            Err(RateLimitError::Limited {
                retry_after: Duration::from_nanos(1_666_666_667),
            })
        );
        assert!(
            limiter
                .check(&client, start + Duration::from_secs(1))
                .is_err()
        );
        assert!(
            limiter
                .check(&client, start + Duration::from_nanos(1_666_666_666))
                .is_err()
        );
        assert!(
            limiter
                .check(&client, start + Duration::from_nanos(1_666_666_667))
                .is_ok()
        );
        assert!(
            limiter
                .check(&client, start + Duration::from_nanos(3_333_333_333))
                .is_err()
        );
        assert!(
            limiter
                .check(&client, start + Duration::from_nanos(3_333_333_334))
                .is_ok()
        );
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
        assert_eq!(
            err,
            RateLimitError::Banned {
                retry_after: Some(Duration::from_secs(30))
            }
        );
        assert_eq!(
            limiter.check(&client, now + Duration::from_millis(30_099)),
            Err(RateLimitError::Banned {
                retry_after: Some(Duration::from_millis(1))
            })
        );
        assert!(
            limiter
                .check(&client, now + Duration::from_millis(30_100))
                .is_ok()
        );
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
    #[test]
    fn rate_limiter_caps_idle_credit_and_keeps_clients_independent() {
        let limiter = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(2),
            window: Duration::from_secs(1),
            ban_duration: None,
        });
        let first = ClientFingerprint::from_identifier("first");
        let second = ClientFingerprint::from_identifier("second");
        let start = Instant::now();
        assert!(limiter.check(&first, start).is_ok());
        let later = start + Duration::from_secs(1_000);
        assert!(limiter.check(&first, later).is_ok());
        assert!(limiter.check(&first, later).is_ok());
        assert!(limiter.check(&first, later).is_err());
        assert!(limiter.check(&second, later).is_ok());
        assert!(limiter.check(&second, later).is_ok());
        assert!(limiter.check(&second, later).is_err());
        // A stale timestamp must not enable a second refill of the same interval.
        assert!(limiter.check(&first, start).is_err());
        assert!(limiter.check(&first, later).is_err());
        assert!(
            limiter
                .check(&first, later + Duration::from_millis(500))
                .is_ok()
        );
    }
    #[test]
    fn rate_limiter_handles_zero_and_extreme_budgets_without_overflow() {
        let client = ClientFingerprint::from_identifier("extreme");
        let now = Instant::now();
        let zero = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(0),
            window: Duration::ZERO,
            ban_duration: None,
        });
        assert_eq!(
            zero.check(&client, now),
            Err(RateLimitError::Limited {
                retry_after: Duration::from_nanos(1),
            })
        );
        let limiter = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(u32::MAX),
            window: Duration::MAX,
            ban_duration: None,
        });
        assert!(limiter.check(&client, now).is_ok());
        limiter.buckets.get_mut(&client).unwrap().credits = 0;
        let wait_nanos = Duration::MAX.as_nanos().div_ceil(u128::from(u32::MAX));
        assert_eq!(
            limiter.check(&client, now),
            Err(RateLimitError::Limited {
                retry_after: Duration::new(
                    (wait_nanos / 1_000_000_000) as u64,
                    (wait_nanos % 1_000_000_000) as u32
                ),
            })
        );
        let ban = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(0),
            window: Duration::MAX,
            ban_duration: Some(Duration::MAX),
        });
        assert_eq!(
            ban.check(&client, now),
            Err(RateLimitError::Banned {
                retry_after: Some(Duration::MAX)
            })
        );
        assert_eq!(
            ban.check(&client, now),
            Err(RateLimitError::Banned {
                retry_after: Some(Duration::MAX)
            })
        );
        let fast = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(u32::MAX),
            window: Duration::ZERO,
            ban_duration: None,
        });
        assert!(fast.check(&client, now).is_ok());
        fast.buckets.get_mut(&client).unwrap().credits = 0;
        assert!(fast.check(&client, now + Duration::from_nanos(1)).is_ok());
        assert_eq!(
            fast.buckets.get(&client).unwrap().credits,
            u128::from(u32::MAX) - 1
        );
    }
    #[test]
    fn rate_limiter_default_admits_large_solo_burst_with_one_fixed_bucket() {
        let limiter = GatewayRateLimiter::new_default();
        assert_eq!(limiter.config.max_requests, Some(600_000));
        assert_eq!(limiter.config.window, Duration::from_secs(60));
        assert_eq!(limiter.config.ban_duration, Some(Duration::from_secs(30)));
        let client = ClientFingerprint::from_identifier("solo-public-client");
        let now = Instant::now();
        for request in 0..600_000 {
            assert!(limiter.check(&client, now).is_ok(), "request {request}");
        }
        assert_eq!(limiter.buckets.len(), 1);
        assert_eq!(limiter.buckets.get(&client).unwrap().credits, 0);
        assert!(matches!(
            limiter.check(&client, now),
            Err(RateLimitError::Banned { .. })
        ));
    }
    #[test]
    fn rate_limiter_reclaims_idle_buckets_but_retains_active_bans() {
        let limiter = GatewayRateLimiter::new(GatewayRateLimitConfig {
            max_requests: Some(1),
            window: Duration::from_secs(1),
            ban_duration: Some(Duration::from_secs(30)),
        });
        let idle = ClientFingerprint::from_identifier("idle");
        let banned = ClientFingerprint::from_identifier("banned");
        let now = Instant::now();
        assert!(limiter.check(&idle, now).is_ok());
        assert!(limiter.check(&banned, now).is_ok());
        assert!(limiter.check(&banned, now).is_err());
        limiter.prune_inactive(now + Duration::from_secs(2));
        assert!(!limiter.buckets.contains_key(&idle));
        assert!(limiter.buckets.contains_key(&banned));
        limiter.prune_inactive(now + Duration::from_secs(30));
        assert!(limiter.buckets.is_empty());
        let disabled = GatewayRateLimiter::new(GatewayRateLimitConfig::disabled());
        assert!(disabled.check(&idle, now).is_ok());
        assert!(disabled.buckets.is_empty());
    }
}
