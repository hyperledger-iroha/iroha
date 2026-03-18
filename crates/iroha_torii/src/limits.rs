//! Rate limiting and API token utilities for Torii.
//!
//! Implements a simple token-bucket rate limiter keyed by a caller identity
//! (API token or authority id). This protects the node from abuse without
//! introducing gas/fees on read endpoints.

#![allow(clippy::redundant_pub_crate)]

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::http::HeaderMap;
use dashmap::{DashMap, mapref::entry::Entry};
use tokio::sync::Mutex;

/// Shared, cheap-to-clone limiter.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<InnerLimiter>>,
}

struct InnerLimiter {
    rate_per_sec: Option<f64>,
    burst: f64,
    buckets: HashMap<String, TokenBucket>,
    order: VecDeque<String>,
    max_buckets: usize,
}

#[derive(Clone, Copy)]
struct TokenBucket {
    tokens: f64,
    last: Instant,
}

const DEFAULT_MAX_BUCKETS: usize = 4_096;
const PREAUTH_NOFILE_RESERVE: u64 = 128;

impl RateLimiter {
    /// Create a new limiter. If `rate_per_sec` is None or 0, the limiter allows all.
    pub fn new(rate_per_sec: Option<u32>, burst: Option<u32>) -> Self {
        Self::new_with_capacity(rate_per_sec, burst, DEFAULT_MAX_BUCKETS)
    }

    /// Create a new limiter configured with `u64`-sized token buckets.
    pub fn new_u64(rate_per_sec: Option<u64>, burst: Option<u64>) -> Self {
        let rate = rate_per_sec.and_then(|v| if v == 0 { None } else { Some(v as f64) });
        let burst = burst.unwrap_or_else(|| rate_per_sec.unwrap_or(0)).max(1) as f64;
        Self {
            inner: Arc::new(Mutex::new(InnerLimiter {
                rate_per_sec: rate,
                burst,
                buckets: HashMap::new(),
                order: VecDeque::new(),
                max_buckets: DEFAULT_MAX_BUCKETS,
            })),
        }
    }

    pub(crate) fn new_with_capacity(
        rate_per_sec: Option<u32>,
        burst: Option<u32>,
        max_buckets: usize,
    ) -> Self {
        let rate = rate_per_sec.and_then(|v| if v == 0 { None } else { Some(v as f64) });
        let burst = burst.unwrap_or_else(|| rate_per_sec.unwrap_or(0)).max(1) as f64;
        Self {
            inner: Arc::new(Mutex::new(InnerLimiter {
                rate_per_sec: rate,
                burst,
                buckets: HashMap::new(),
                order: VecDeque::new(),
                max_buckets: max_buckets.max(1),
            })),
        }
    }

    /// Returns true if allowed (consumed 1 token), false if limited.
    pub async fn allow(&self, key: &str) -> bool {
        self.allow_cost(key, 1).await
    }

    /// Returns true if allowed after consuming `cost` tokens, false if limited.
    pub async fn allow_cost(&self, key: &str, cost: u64) -> bool {
        let mut inner = self.inner.lock().await;
        // No limiting configured
        let Some(rate) = inner.rate_per_sec else {
            return true;
        };
        let now = Instant::now();
        let burst = inner.burst;
        let key_owned = key.to_string();
        if !inner.buckets.contains_key(&key_owned) {
            if inner.buckets.len() >= inner.max_buckets {
                if let Some(oldest) = inner.order.pop_front() {
                    inner.buckets.remove(&oldest);
                }
            }
            inner.order.push_back(key_owned.clone());
            inner.buckets.insert(
                key_owned.clone(),
                TokenBucket {
                    tokens: burst,
                    last: now,
                },
            );
        }
        let Some(bucket) = inner.buckets.get_mut(&key_owned) else {
            return true;
        };
        // Refill
        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f64();
        if elapsed > 0.0 {
            bucket.tokens = (bucket.tokens + elapsed * rate).min(burst);
            bucket.last = now;
        }
        let required = (cost.max(1) as f64).min(f64::MAX);
        if required > burst {
            return false;
        }
        if bucket.tokens >= required {
            bucket.tokens -= required;
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(crate) async fn bucket_count(&self) -> usize {
        self.inner.lock().await.buckets.len()
    }
}

/// Internal header recording the remote IP the connection was accepted from.
pub const REMOTE_ADDR_HEADER: &str = "x-iroha-remote-addr";

/// Derive a rate-limit key from headers and optional hint:
/// - Prefer `X-API-Token` if present and token usage is enabled
/// - Else the provided remote IP address (from listener metadata)
/// - Else the trusted header injected by middleware (`x-iroha-remote-addr`)
/// - Else provided hint
/// - Else "anon"
pub fn key_from_headers(
    headers: &HeaderMap,
    remote: Option<IpAddr>,
    hint: Option<&str>,
    use_api_token: bool,
) -> String {
    if use_api_token {
        if let Some(v) = headers.get("x-api-token").and_then(|v| v.to_str().ok()) {
            return v.to_string();
        }
    }
    if let Some(ip) = remote.or_else(|| {
        headers
            .get(REMOTE_ADDR_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
    }) {
        return ip.to_string();
    }
    if let Some(h) = hint {
        return h.to_string();
    }
    "anon".to_string()
}

/// Awaitable helper: returns true when request should pass (either not enforced
/// or limiter allows), false when it should be rate-limited.
pub async fn allow_conditionally(limiter: &RateLimiter, key: &str, enforce: bool) -> bool {
    if !enforce {
        true
    } else {
        limiter.allow(key).await
    }
}

/// Awaitable helper for costed operations: returns true when request should pass (either not
/// enforced or limiter allows), false when it should be rate-limited.
pub async fn allow_cost_conditionally(
    limiter: &RateLimiter,
    key: &str,
    cost: u64,
    enforce: bool,
) -> bool {
    if !enforce {
        true
    } else {
        limiter.allow_cost(key, cost).await
    }
}

#[allow(dead_code)]
fn _assert_allow_conditionally_future_send() {
    fn assert_send_future<F: std::future::Future + Send>(future: F) {
        drop(future);
    }
    let limiter = RateLimiter::new(Some(1), Some(1));
    assert_send_future(allow_conditionally(&limiter, "key", true));
}

// ---------------- CIDR allowlist helpers ----------------

#[derive(Clone, Debug)]
pub struct IpNet {
    kind: IpKind,
}

#[derive(Clone, Debug)]
enum IpKind {
    V4 { net: u32, mask: u32 },
    V6 { net: [u8; 16], bits: u8 },
}

pub fn parse_cidr(s: &str) -> Option<IpNet> {
    if let Some((ip, bits_str)) = s.split_once('/') {
        let bits: u8 = bits_str.parse().ok()?;
        if let Ok(v4) = ip.parse::<Ipv4Addr>() {
            if bits > 32 {
                return None;
            }
            let n = u32::from(v4);
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            return Some(IpNet {
                kind: IpKind::V4 {
                    net: n & mask,
                    mask,
                },
            });
        }
        if let Ok(v6) = ip.parse::<Ipv6Addr>() {
            if bits > 128 {
                return None;
            }
            let mut net = [0u8; 16];
            net.copy_from_slice(&v6.octets());
            let full_bytes = (bits / 8) as usize;
            let rem_bits = bits % 8;
            if full_bytes < 16 {
                if rem_bits == 0 {
                    for b in net.iter_mut().skip(full_bytes) {
                        *b = 0;
                    }
                } else {
                    for b in net.iter_mut().skip(full_bytes + 1) {
                        *b = 0;
                    }
                    let mask = 0xFFu8 << (8 - rem_bits);
                    net[full_bytes] &= mask;
                }
            }
            return Some(IpNet {
                kind: IpKind::V6 { net, bits },
            });
        }
    }
    None
}

pub fn parse_cidrs(list: &[String]) -> Vec<IpNet> {
    list.iter().filter_map(|s| parse_cidr(s)).collect()
}

pub fn cidr_contains(nets: &[IpNet], ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let x = u32::from(v4);
            nets.iter().any(|n| match &n.kind {
                IpKind::V4 { net, mask } => (x & mask) == *net,
                _ => false,
            })
        }
        IpAddr::V6(v6) => {
            let x = v6.octets();
            nets.iter().any(|n| match &n.kind {
                IpKind::V6 { net, bits } => {
                    let full = (*bits / 8) as usize;
                    let rem = *bits % 8;
                    (full == 0 || x[..full] == net[..full])
                        && (rem == 0 || {
                            let mask = 0xFFu8 << (8 - rem);
                            (x[full] & mask) == (net[full] & mask)
                        })
                }
                _ => false,
            })
        }
    }
}

/// Returns true if the request should bypass rate limits due to CIDR allowlist.
/// Prefers the explicitly supplied remote IP and falls back to the trusted
/// header injected by middleware.
pub fn is_allowed_by_cidr(headers: &HeaderMap, remote: Option<IpAddr>, allow: &[IpNet]) -> bool {
    let candidate_ip = remote.or_else(|| {
        headers
            .get(REMOTE_ADDR_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
    });
    candidate_ip.map_or(false, |ip| cidr_contains(allow, ip))
}

/// Configuration for the pre-authentication connection gate.
#[derive(Debug, Clone)]
pub struct PreAuthConfig {
    pub max_total: Option<usize>,
    pub max_per_ip: Option<usize>,
    pub rate_per_ip: Option<u32>,
    pub burst_per_ip: Option<u32>,
    pub ban_duration: Option<Duration>,
    pub allow_nets: Vec<IpNet>,
    pub scheme_limits: Vec<SchemeLimit>,
}

#[cfg(unix)]
#[allow(unsafe_code)]
pub(crate) fn nofile_soft_limit() -> Option<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: libc::getrlimit expects a valid, mutable rlimit pointer.
    let result = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) };
    if result != 0 {
        return None;
    }
    let soft = limit.rlim_cur;
    if soft == libc::RLIM_INFINITY || soft == 0 {
        return None;
    }
    Some(soft)
}

#[cfg(not(unix))]
pub(crate) fn nofile_soft_limit() -> Option<u64> {
    None
}

fn preauth_budget_from_nofile(nofile_soft: u64) -> u64 {
    let reserve = PREAUTH_NOFILE_RESERVE.min(nofile_soft.saturating_sub(1));
    let budget = nofile_soft.saturating_sub(reserve);
    (budget / 2).max(1)
}

pub(crate) fn clamp_preauth_max_total(
    configured: Option<usize>,
    nofile_soft: Option<u64>,
) -> Option<usize> {
    let configured = configured?;
    let Some(nofile_soft) = nofile_soft else {
        return Some(configured);
    };
    let cap = preauth_budget_from_nofile(nofile_soft) as usize;
    Some(configured.min(cap))
}

pub(crate) fn clamp_preauth_max_per_ip(
    configured: Option<usize>,
    max_total: Option<usize>,
) -> Option<usize> {
    let configured = configured?;
    Some(max_total.map_or(configured, |max_total| configured.min(max_total)))
}

/// Per-scheme concurrency limit description.
#[derive(Debug, Clone)]
pub struct SchemeLimit {
    /// Scheme label (matches `ConnScheme::label()`).
    pub name: String,
    /// Maximum concurrent connections allowed for the scheme.
    pub max_connections: usize,
}

#[derive(Clone)]
pub struct PreAuthGate {
    inner: Arc<PreAuthGateInner>,
}

struct PreAuthGateInner {
    disabled: bool,
    max_total: Option<usize>,
    max_per_ip: Option<usize>,
    rate_limiter: Option<RateLimiter>,
    ban_duration: Option<Duration>,
    allow_nets: Vec<IpNet>,
    active_total: AtomicUsize,
    active_per_ip: DashMap<IpAddr, usize>,
    scheme_limits: HashMap<String, usize>,
    active_per_scheme: DashMap<String, usize>,
    bans: DashMap<IpAddr, Instant>,
}

/// Guard tracking held slots within the pre-auth gate.
pub struct PreAuthPermit {
    gate: Arc<PreAuthGateInner>,
    ip: Option<IpAddr>,
    counted_global: bool,
    counted_ip: bool,
    scheme: Option<String>,
    counted_scheme: bool,
}

impl fmt::Debug for PreAuthPermit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreAuthPermit")
            .field("ip", &self.ip)
            .field("counted_global", &self.counted_global)
            .field("counted_ip", &self.counted_ip)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    GlobalCap,
    IpCap,
    RateLimited,
    Banned,
    SchemeCap,
}

impl RejectReason {
    pub fn metric_label(self) -> &'static str {
        match self {
            Self::GlobalCap => "global_cap",
            Self::IpCap => "ip_cap",
            Self::RateLimited => "rate",
            Self::Banned => "ban",
            Self::SchemeCap => "scheme_cap",
        }
    }
}

impl PreAuthGate {
    pub fn new(cfg: PreAuthConfig) -> Self {
        let PreAuthConfig {
            max_total,
            max_per_ip,
            rate_per_ip,
            burst_per_ip,
            ban_duration,
            allow_nets,
            scheme_limits,
        } = cfg;
        let scheme_limits_map: HashMap<String, usize> = scheme_limits
            .into_iter()
            .filter(|limit| limit.max_connections > 0)
            .map(|limit| (limit.name.to_ascii_lowercase(), limit.max_connections))
            .collect();
        let disabled = max_total.is_none()
            && max_per_ip.is_none()
            && rate_per_ip.is_none()
            && ban_duration.is_none()
            && scheme_limits_map.is_empty();
        let rate_limiter = rate_per_ip.map(|rate| RateLimiter::new(Some(rate), burst_per_ip));
        let inner = PreAuthGateInner {
            disabled,
            max_total,
            max_per_ip,
            rate_limiter,
            ban_duration,
            allow_nets,
            active_total: AtomicUsize::new(0),
            active_per_ip: DashMap::new(),
            scheme_limits: scheme_limits_map,
            active_per_scheme: DashMap::new(),
            bans: DashMap::new(),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn disabled() -> Self {
        Self::new(PreAuthConfig {
            max_total: None,
            max_per_ip: None,
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            allow_nets: Vec::new(),
            scheme_limits: Vec::new(),
        })
    }

    pub async fn acquire(
        &self,
        ip: Option<IpAddr>,
        scheme: Option<&str>,
    ) -> Result<PreAuthPermit, RejectReason> {
        let inner = &self.inner;
        if inner.disabled {
            return Ok(PreAuthPermit::bypass(inner.clone(), ip));
        }

        if let Some(addr) = ip {
            if inner.is_allowlisted(addr) {
                return Ok(PreAuthPermit::bypass(inner.clone(), Some(addr)));
            }

            if inner.is_banned(addr) {
                return Err(RejectReason::Banned);
            }

            if let Some(rate) = inner.rate_limiter.as_ref() {
                if !rate.allow(&addr.to_string()).await {
                    inner.note_ban(addr);
                    return Err(RejectReason::RateLimited);
                }
            }
        }

        let counted_ip_addr = if let Some(addr) = ip {
            match inner.active_per_ip.entry(addr) {
                Entry::Occupied(mut occ) => {
                    if let Some(limit) = inner.max_per_ip {
                        if *occ.get() >= limit {
                            inner.note_ban(addr);
                            return Err(RejectReason::IpCap);
                        }
                    }
                    *occ.get_mut() += 1;
                }
                Entry::Vacant(vac) => {
                    vac.insert(1);
                }
            }
            Some(addr)
        } else {
            None
        };
        let counted_ip = counted_ip_addr.is_some();

        let scheme_key = if let Some(label) = scheme {
            if let Some(limit) = inner.scheme_limits.get(label) {
                let key = label.to_string();
                match inner.active_per_scheme.entry(key.clone()) {
                    Entry::Occupied(mut occ) => {
                        if *occ.get() >= *limit {
                            if let Some(addr) = counted_ip_addr {
                                inner.release_ip(addr);
                            }
                            return Err(RejectReason::SchemeCap);
                        }
                        *occ.get_mut() += 1;
                    }
                    Entry::Vacant(vac) => {
                        vac.insert(1);
                    }
                }
                Some(key)
            } else {
                None
            }
        } else {
            None
        };
        let counted_scheme = scheme_key.is_some();

        let counted_global = if let Some(limit) = inner.max_total {
            let prev = inner.active_total.fetch_add(1, Ordering::AcqRel);
            if prev >= limit {
                inner.active_total.fetch_sub(1, Ordering::Release);
                if let Some(addr) = counted_ip_addr {
                    inner.release_ip(addr);
                }
                if let Some(label) = scheme_key.as_deref() {
                    inner.release_scheme(label);
                }
                if let Some(addr) = ip {
                    inner.note_ban(addr);
                }
                return Err(RejectReason::GlobalCap);
            }
            true
        } else {
            false
        };

        Ok(PreAuthPermit {
            gate: Arc::clone(&self.inner),
            ip,
            counted_global,
            counted_ip,
            scheme: scheme_key,
            counted_scheme,
        })
    }
}

impl PreAuthGateInner {
    fn is_allowlisted(&self, ip: IpAddr) -> bool {
        cidr_contains(&self.allow_nets, ip)
    }

    fn is_banned(&self, ip: IpAddr) -> bool {
        if let Some(expiry) = self.bans.get(&ip) {
            if expiry.checked_duration_since(Instant::now()).is_some() {
                return true;
            }
            self.bans.remove(&ip);
        }
        false
    }

    fn note_ban(&self, ip: IpAddr) {
        if let Some(duration) = self.ban_duration {
            self.bans.insert(ip, Instant::now() + duration);
        }
    }

    fn release_ip(&self, ip: IpAddr) {
        let should_remove = self.active_per_ip.get_mut(&ip).map_or(false, |mut entry| {
            if *entry > 1 {
                *entry -= 1;
                false
            } else {
                true
            }
        });
        if should_remove {
            self.active_per_ip.remove(&ip);
        }
    }

    fn release_scheme(&self, scheme: &str) {
        let should_remove = self
            .active_per_scheme
            .get_mut(scheme)
            .map_or(false, |mut entry| {
                if *entry > 1 {
                    *entry -= 1;
                    false
                } else {
                    true
                }
            });
        if should_remove {
            self.active_per_scheme.remove(scheme);
        }
    }
}

impl PreAuthPermit {
    fn bypass(gate: Arc<PreAuthGateInner>, ip: Option<IpAddr>) -> Self {
        Self {
            gate,
            ip,
            counted_global: false,
            counted_ip: false,
            scheme: None,
            counted_scheme: false,
        }
    }
}

impl Drop for PreAuthPermit {
    fn drop(&mut self) {
        if self.counted_global {
            self.gate.active_total.fetch_sub(1, Ordering::Release);
        }
        if self.counted_ip {
            if let Some(ip) = self.ip {
                self.gate.release_ip(ip);
            }
        }
        if self.counted_scheme {
            if let Some(label) = self.scheme.as_deref() {
                self.gate.release_scheme(label);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn limiter_allows_then_limits() {
        let limiter = RateLimiter::new(Some(2), Some(2));
        // First two immediate requests allowed
        assert!(limiter.allow("a").await);
        assert!(limiter.allow("a").await);
        // Third should be limited
        assert!(!limiter.allow("a").await);
    }

    #[tokio::test]
    async fn limiter_respects_costs() {
        let limiter = RateLimiter::new(Some(10), Some(10));
        assert!(limiter.allow_cost("cost", 5).await);
        assert!(limiter.allow_cost("cost", 4).await);
        // Bucket should be drained beyond burst
        assert!(!limiter.allow_cost("cost", 3).await);
    }

    #[test]
    fn key_from_headers_prefers_token_then_remote_then_hint() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            key_from_headers(
                &headers,
                Some("203.0.113.99".parse().unwrap()),
                Some("hint"),
                true
            ),
            "203.0.113.99"
        );

        headers.insert("x-api-token", "secret".parse().unwrap());
        assert_eq!(
            key_from_headers(&headers, Some("203.0.113.99".parse().unwrap()), None, true),
            "secret"
        );

        let headers2 = HeaderMap::new();
        assert_eq!(
            key_from_headers(&headers2, None, Some("hint"), true),
            "hint"
        );
        assert_eq!(key_from_headers(&headers2, None, None, true), "anon");
    }

    #[test]
    fn key_from_headers_ignores_token_when_disabled() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-token", "secret".parse().unwrap());
        assert_eq!(
            key_from_headers(
                &headers,
                Some("203.0.113.77".parse().unwrap()),
                Some("hint"),
                false
            ),
            "203.0.113.77"
        );
        let headers2 = HeaderMap::new();
        assert_eq!(
            key_from_headers(&headers2, None, Some("hint"), false),
            "hint"
        );
    }

    #[tokio::test]
    async fn limiter_caps_bucket_growth() {
        let limiter = RateLimiter::new_with_capacity(Some(1), Some(1), 2);
        assert!(limiter.allow("a").await);
        assert!(limiter.allow("b").await);
        assert_eq!(limiter.bucket_count().await, 2);

        assert!(limiter.allow("c").await);
        // Capacity is 2, so one bucket must have been evicted.
        assert!(limiter.bucket_count().await <= 2);

        // Previously inserted keys should still be serviced without panicking.
        assert!(limiter.allow("a").await);
        assert!(limiter.bucket_count().await <= 2);
    }

    #[tokio::test]
    async fn preauth_gate_limits_global_and_per_ip() {
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: Some(1),
            max_per_ip: Some(1),
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            allow_nets: Vec::new(),
            scheme_limits: Vec::new(),
        });
        let ip: IpAddr = "192.0.2.10".parse().unwrap();
        let permit = gate
            .acquire(Some(ip), Some("http"))
            .await
            .expect("first allowed");
        let err = gate
            .acquire(Some(ip), Some("http"))
            .await
            .expect_err("second should fail");
        assert_eq!(err, RejectReason::IpCap);
        drop(permit);
        gate.acquire(Some(ip), Some("http"))
            .await
            .expect("permit released allows again");
    }

    #[tokio::test]
    async fn preauth_gate_respects_allowlist() {
        let nets = parse_cidrs(&["127.0.0.0/8".to_string()]);
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: Some(1),
            max_per_ip: Some(1),
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            allow_nets: nets,
            scheme_limits: Vec::new(),
        });
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        gate.acquire(Some(ip), Some("http"))
            .await
            .expect("allowlisted bypass");
        gate.acquire(Some(ip), Some("http"))
            .await
            .expect("allowlisted bypass repeated");
    }

    #[tokio::test]
    async fn preauth_gate_rate_limits_and_bans() {
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: None,
            max_per_ip: None,
            rate_per_ip: Some(1),
            burst_per_ip: Some(1),
            ban_duration: Some(Duration::from_millis(50)),
            allow_nets: Vec::new(),
            scheme_limits: Vec::new(),
        });
        let ip: IpAddr = "198.51.100.1".parse().unwrap();
        gate.acquire(Some(ip), Some("http"))
            .await
            .expect("first allowed");
        let err = gate
            .acquire(Some(ip), Some("http"))
            .await
            .expect_err("rate limit triggers");
        assert_eq!(err, RejectReason::RateLimited);
        let banned = gate
            .acquire(Some(ip), Some("http"))
            .await
            .expect_err("ban active");
        assert_eq!(banned, RejectReason::Banned);
    }

    #[tokio::test]
    async fn preauth_gate_limits_per_scheme() {
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: None,
            max_per_ip: None,
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            allow_nets: Vec::new(),
            scheme_limits: vec![SchemeLimit {
                name: "norito_rpc".to_string(),
                max_connections: 1,
            }],
        });
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        let permit = gate
            .acquire(Some(ip), Some("norito_rpc"))
            .await
            .expect("first scheme connection allowed");
        let err = gate
            .acquire(Some(ip), Some("norito_rpc"))
            .await
            .expect_err("second scheme connection rejected");
        assert_eq!(err, RejectReason::SchemeCap);
        drop(permit);
        gate.acquire(Some(ip), Some("norito_rpc"))
            .await
            .expect("scheme permit released");
        // HTTP (no scheme limit) should be unaffected.
        gate.acquire(Some(ip), Some("http"))
            .await
            .expect("http scheme uses global pool");
    }

    fn parse_cidrs_skips_invalid_entries() {
        let nets = parse_cidrs(&[
            "203.0.113.0/24".into(),
            "bad-entry".into(),
            "2001:db8::/129".into(),
        ]);
        assert_eq!(nets.len(), 1);
        assert!(matches!(nets[0].kind, IpKind::V4 { .. }));
    }

    #[test]
    fn parse_cidr_ipv6_zero_prefix_zeroes_octets() {
        let parsed = parse_cidr("::/0").expect("valid zero prefix");
        match parsed.kind {
            IpKind::V6 { net, bits } => {
                assert_eq!(bits, 0);
                assert!(net.iter().all(|b| *b == 0));
            }
            _ => panic!("expected IPv6 network"),
        }
    }

    #[test]
    fn parse_cidr_ipv6_full_prefix_retains_address() {
        let parsed = parse_cidr("2001:db8::dead:beef/128").expect("valid /128");
        match parsed.kind {
            IpKind::V6 { net, bits } => {
                assert_eq!(bits, 128);
                assert_eq!(
                    net,
                    "2001:db8::dead:beef".parse::<Ipv6Addr>().unwrap().octets()
                );
            }
            _ => panic!("expected IPv6 network"),
        }
    }

    #[test]
    fn cidr_contains_supports_ipv6_partial_prefix() {
        let net = parse_cidr("2001:db8::/65").expect("valid IPv6 CIDR");
        let nets = [net];
        assert!(cidr_contains(
            &nets,
            "2001:db8::1".parse().expect("valid IPv6 address")
        ));
        assert!(!cidr_contains(
            &nets,
            "2001:db8:0:0:8000::1"
                .parse()
                .expect("valid IPv6 address outside net")
        ));
    }

    #[test]
    fn key_from_headers_uses_trusted_header_when_remote_missing() {
        let mut headers = HeaderMap::new();
        headers.insert(
            REMOTE_ADDR_HEADER,
            "2001:db8::42".parse().expect("valid header value"),
        );
        assert_eq!(key_from_headers(&headers, None, None, true), "2001:db8::42");
    }

    #[test]
    fn key_from_headers_remote_overrides_injected_header() {
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_ADDR_HEADER, "203.0.113.55".parse().unwrap());
        assert_eq!(
            key_from_headers(
                &headers,
                Some("198.51.100.1".parse().unwrap()),
                Some("hint"),
                true
            ),
            "198.51.100.1"
        );
    }

    #[tokio::test]
    async fn allow_conditionally_bypasses_when_disabled() {
        let limiter = RateLimiter::new(Some(1), Some(1));
        // Saturate the limiter so subsequent checks would fail when enforced.
        assert!(limiter.allow("key").await);
        assert!(!limiter.allow("key").await);

        assert!(allow_conditionally(&limiter, "key", false).await);
        assert!(!allow_conditionally(&limiter, "key", true).await);
    }

    #[tokio::test]
    async fn limiter_with_disabled_configuration_short_circuits() {
        let limiter = RateLimiter::new(None, None);
        for _ in 0..100 {
            assert!(limiter.allow("any").await);
        }
    }

    #[test]
    fn is_allowed_by_cidr_prefers_remote_ip() {
        let allow = vec![parse_cidr("203.0.113.0/24").unwrap()];
        let headers = HeaderMap::new();
        assert!(is_allowed_by_cidr(
            &headers,
            Some("203.0.113.42".parse().unwrap()),
            &allow
        ));
        assert!(!is_allowed_by_cidr(
            &headers,
            Some("198.51.100.1".parse().unwrap()),
            &allow
        ));

        let mut headers_with_injected = HeaderMap::new();
        headers_with_injected.insert(REMOTE_ADDR_HEADER, "203.0.113.55".parse().unwrap());
        assert!(is_allowed_by_cidr(&headers_with_injected, None, &allow));
    }

    #[test]
    fn is_allowed_by_cidr_remote_override_rejects_spoofed_header() {
        let allow = vec![parse_cidr("203.0.113.0/24").unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_ADDR_HEADER, "203.0.113.55".parse().unwrap());
        assert!(!is_allowed_by_cidr(
            &headers,
            Some("198.51.100.1".parse().unwrap()),
            &allow
        ));
    }

    #[test]
    fn is_allowed_by_cidr_respects_dual_stack_injected_header() {
        let allow = vec![
            parse_cidr("203.0.113.0/24").unwrap(),
            parse_cidr("2001:db8::/64").unwrap(),
        ];
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_ADDR_HEADER, "203.0.113.77".parse().unwrap());
        assert!(is_allowed_by_cidr(&headers, None, &allow));

        headers.insert(REMOTE_ADDR_HEADER, "2001:db8::99".parse().unwrap());
        assert!(is_allowed_by_cidr(&headers, None, &allow));
    }

    #[test]
    fn clamp_preauth_max_total_respects_nofile_budget() {
        let nofile_soft = Some(256);
        assert_eq!(clamp_preauth_max_total(Some(200), nofile_soft), Some(64));
        assert_eq!(clamp_preauth_max_total(Some(32), nofile_soft), Some(32));
    }

    #[test]
    fn clamp_preauth_max_per_ip_caps_to_total() {
        assert_eq!(clamp_preauth_max_per_ip(Some(100), Some(64)), Some(64));
        assert_eq!(clamp_preauth_max_per_ip(Some(50), Some(64)), Some(50));
        assert_eq!(clamp_preauth_max_per_ip(Some(50), None), Some(50));
        assert_eq!(clamp_preauth_max_per_ip(None, Some(64)), None);
    }
}
