//! Rate limiting and API token utilities for Torii.
//!
//! Implements a sharded token-bucket rate limiter keyed by a caller identity
//! (API token or authority id). This protects the node from abuse without
//! introducing gas/fees on read endpoints.

#![allow(clippy::redundant_pub_crate)]

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, VecDeque, hash_map::DefaultHasher},
    fmt,
    hash::{Hash, Hasher},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::http::HeaderMap;
use dashmap::{DashMap, mapref::entry::Entry};
use parking_lot::Mutex;

/// Shared, cheap-to-clone limiter.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<ShardedLimiter>,
}

struct ShardedLimiter {
    disabled: bool,
    shards: Vec<Mutex<InnerLimiter>>,
}

struct InnerLimiter {
    rate_per_sec: f64,
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
const DEFAULT_RATE_LIMITER_SHARDS: usize = 64;
const MIN_BUCKETS_PER_SHARD: usize = 64;
const PREAUTH_NOFILE_RESERVE: u64 = 128;
/// Hard upper bound for simultaneously tracked pre-authentication bans.
///
/// This matches the rate limiter's default identity budget and prevents both
/// the live map and stale expiry records from growing without bound.
const DEFAULT_PREAUTH_BAN_CAPACITY: NonZeroUsize =
    NonZeroUsize::new(DEFAULT_MAX_BUCKETS).expect("default ban capacity is non-zero");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BanEntry {
    expires_at: Instant,
    generation: u64,
}

/// Heap key ordered by earliest expiry and then IP for deterministic eviction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BanExpiry {
    expires_at: Instant,
    ip: IpAddr,
    generation: u64,
}

#[derive(Default)]
struct ExpiringBanState {
    entries: HashMap<IpAddr, BanEntry>,
    expiries: BinaryHeap<Reverse<BanExpiry>>,
    next_generation: u64,
}

struct ExpiringBanStore {
    capacity: usize,
    state: Mutex<ExpiringBanState>,
}

impl ExpiringBanStore {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            state: Mutex::new(ExpiringBanState::default()),
        }
    }

    fn is_banned_at(&self, ip: IpAddr, now: Instant) -> bool {
        let mut state = self.state.lock();
        state.purge_expired(now);
        state.entries.contains_key(&ip)
    }

    fn ban_for_at(&self, ip: IpAddr, duration: Duration, now: Instant) {
        if self.capacity == 0 || duration.is_zero() {
            return;
        }
        let Some(expires_at) = now.checked_add(duration) else {
            return;
        };

        let mut state = self.state.lock();
        state.purge_expired(now);

        let generation = state.allocate_generation();
        if !state.entries.contains_key(&ip) && state.entries.len() >= self.capacity {
            state.evict_earliest();
        }
        state.entries.insert(
            ip,
            BanEntry {
                expires_at,
                generation,
            },
        );
        state.expiries.push(Reverse(BanExpiry {
            expires_at,
            ip,
            generation,
        }));
        state.compact_expiries_if_needed(self.capacity);
    }

    #[cfg(test)]
    fn entry_count(&self) -> usize {
        self.state.lock().entries.len()
    }

    #[cfg(test)]
    fn expiry_count(&self) -> usize {
        self.state.lock().expiries.len()
    }
}

impl ExpiringBanState {
    fn purge_expired(&mut self, now: Instant) {
        while let Some(Reverse(expiry)) = self.expiries.peek().copied() {
            if expiry.expires_at > now {
                break;
            }
            self.expiries.pop();
            if self.entry_matches(expiry) {
                self.entries.remove(&expiry.ip);
            }
        }
    }

    fn evict_earliest(&mut self) {
        while let Some(Reverse(expiry)) = self.expiries.pop() {
            if self.entry_matches(expiry) {
                self.entries.remove(&expiry.ip);
                return;
            }
        }

        // The heap and map are updated under one mutex, so this branch is only
        // a defensive repair for an inconsistent in-memory index. Preserve the
        // hard capacity invariant even then.
        if let Some(ip) = self
            .entries
            .iter()
            .min_by_key(|(ip, entry)| (entry.expires_at, **ip))
            .map(|(&ip, _)| ip)
        {
            self.entries.remove(&ip);
        }
    }

    fn entry_matches(&self, expiry: BanExpiry) -> bool {
        self.entries.get(&expiry.ip).is_some_and(|entry| {
            entry.generation == expiry.generation && entry.expires_at == expiry.expires_at
        })
    }

    fn allocate_generation(&mut self) -> u64 {
        if self.next_generation == u64::MAX {
            self.renumber_generations();
        }
        let generation = self.next_generation;
        self.next_generation += 1;
        generation
    }

    fn renumber_generations(&mut self) {
        let mut ordered = self
            .entries
            .iter()
            .map(|(&ip, entry)| (entry.expires_at, ip))
            .collect::<Vec<_>>();
        ordered.sort_unstable();
        self.expiries.clear();
        for (generation, (expires_at, ip)) in ordered.into_iter().enumerate() {
            let generation = u64::try_from(generation)
                .expect("bounded pre-authentication ban count fits in u64");
            let entry = self
                .entries
                .get_mut(&ip)
                .expect("ban generation rebuild uses existing IPs");
            entry.generation = generation;
            self.expiries.push(Reverse(BanExpiry {
                expires_at,
                ip,
                generation,
            }));
        }
        self.next_generation = u64::try_from(self.entries.len())
            .expect("bounded pre-authentication ban count fits in u64");
    }

    fn compact_expiries_if_needed(&mut self, capacity: usize) {
        let max_expiry_records = capacity.saturating_mul(2).max(1);
        if self.expiries.len() <= max_expiry_records {
            return;
        }
        self.expiries.clear();
        self.expiries
            .extend(self.entries.iter().map(|(&ip, entry)| {
                Reverse(BanExpiry {
                    expires_at: entry.expires_at,
                    ip,
                    generation: entry.generation,
                })
            }));
    }
}

impl ShardedLimiter {
    fn new(rate_per_sec: Option<f64>, burst: f64, max_buckets: usize) -> Self {
        let max_buckets = max_buckets.max(1);
        let disabled = rate_per_sec.is_none();
        let shard_count = if disabled {
            1
        } else {
            max_buckets
                .div_ceil(MIN_BUCKETS_PER_SHARD)
                .min(DEFAULT_RATE_LIMITER_SHARDS)
                .max(1)
        };
        let rate_per_sec = rate_per_sec.unwrap_or(0.0);
        let base_capacity = max_buckets / shard_count;
        let extra_capacity = max_buckets % shard_count;
        let shards = (0..shard_count)
            .map(|index| {
                let shard_capacity = if disabled {
                    max_buckets
                } else {
                    base_capacity + usize::from(index < extra_capacity)
                };
                Mutex::new(InnerLimiter::new(
                    rate_per_sec,
                    burst,
                    shard_capacity.max(1),
                ))
            })
            .collect();

        Self { disabled, shards }
    }

    fn shard_for(&self, key: &str) -> &Mutex<InnerLimiter> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let shard_count = u64::try_from(self.shards.len()).expect("shard count fits in u64");
        let index =
            usize::try_from(hasher.finish() % shard_count).expect("shard index fits in usize");
        &self.shards[index]
    }
}

impl InnerLimiter {
    fn new(rate_per_sec: f64, burst: f64, max_buckets: usize) -> Self {
        Self {
            rate_per_sec,
            burst,
            buckets: HashMap::new(),
            order: VecDeque::new(),
            max_buckets,
        }
    }

    fn insert_full_bucket(&mut self, key: &str, now: Instant) {
        if self.buckets.len() >= self.max_buckets {
            if let Some(oldest) = self.order.pop_front() {
                self.buckets.remove(&oldest);
            }
        }
        let key_owned = key.to_string();
        self.order.push_back(key_owned.clone());
        self.buckets.insert(
            key_owned,
            TokenBucket {
                tokens: self.burst,
                last: now,
            },
        );
    }

    fn refill_bucket(rate_per_sec: f64, burst: f64, bucket: &mut TokenBucket, now: Instant) {
        let elapsed = now.saturating_duration_since(bucket.last).as_secs_f64();
        if elapsed > 0.0 {
            bucket.tokens = (bucket.tokens + elapsed * rate_per_sec).min(burst);
            bucket.last = now;
        }
    }

    fn allow_cost(&mut self, key: &str, cost: u64, now: Instant) -> bool {
        let burst = self.burst;
        let required = (cost.max(1) as f64).min(f64::MAX);
        if required > burst {
            return false;
        }

        self.allow_required(key, required, now)
    }

    fn allow_cost_capped_to_burst(&mut self, key: &str, cost: u64, now: Instant) -> bool {
        let required = (cost.max(1) as f64).min(self.burst);
        self.allow_required(key, required, now)
    }

    fn allow_required(&mut self, key: &str, required: f64, now: Instant) -> bool {
        let burst = self.burst;
        let rate_per_sec = self.rate_per_sec;
        let bucket = match self.buckets.get_mut(key) {
            Some(bucket) => bucket,
            None => {
                self.insert_full_bucket(key, now);
                self.buckets
                    .get_mut(key)
                    .expect("inserted rate-limit bucket must be present")
            }
        };
        Self::refill_bucket(rate_per_sec, burst, bucket, now);
        if bucket.tokens >= required {
            bucket.tokens -= required;
            true
        } else {
            false
        }
    }

    fn allow_repeated(&mut self, key: &str, count: usize, now: Instant) -> bool {
        if count == 0 {
            return true;
        }

        let required = count as f64;
        if required > self.burst {
            return false;
        }

        self.allow_required(key, required, now)
    }
}

impl RateLimiter {
    /// Create a new limiter. If `rate_per_sec` is None or 0, the limiter allows all.
    pub fn new(rate_per_sec: Option<u32>, burst: Option<u32>) -> Self {
        Self::new_with_capacity(rate_per_sec, burst, DEFAULT_MAX_BUCKETS)
    }

    /// Create a limiter from an exact requests-per-minute rate.
    ///
    /// Fractional per-second refill is preserved, so rates below 60/minute do
    /// not get rounded up to one request per second.
    pub fn new_per_minute(rate_per_minute: Option<u32>, burst: Option<u32>) -> Self {
        let rate = rate_per_minute.and_then(|value| {
            (value > 0).then_some(f64::from(value) / Duration::from_secs(60).as_secs_f64())
        });
        let burst = burst.unwrap_or_else(|| rate_per_minute.unwrap_or(0)).max(1) as f64;
        Self {
            inner: Arc::new(ShardedLimiter::new(rate, burst, DEFAULT_MAX_BUCKETS)),
        }
    }

    /// Create a new limiter configured with `u64`-sized token buckets.
    pub fn new_u64(rate_per_sec: Option<u64>, burst: Option<u64>) -> Self {
        let rate = rate_per_sec.and_then(|v| if v == 0 { None } else { Some(v as f64) });
        let burst = burst.unwrap_or_else(|| rate_per_sec.unwrap_or(0)).max(1) as f64;
        Self {
            inner: Arc::new(ShardedLimiter::new(rate, burst, DEFAULT_MAX_BUCKETS)),
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
            inner: Arc::new(ShardedLimiter::new(rate, burst, max_buckets)),
        }
    }

    /// Returns true if allowed (consumed 1 token), false if limited.
    pub async fn allow(&self, key: &str) -> bool {
        self.allow_cost(key, 1).await
    }

    /// Returns true if allowed after consuming `cost` tokens, false if limited.
    #[allow(clippy::unused_async)]
    pub async fn allow_cost(&self, key: &str, cost: u64) -> bool {
        if self.inner.disabled {
            return true;
        }
        self.inner
            .shard_for(key)
            .lock()
            .allow_cost(key, cost, Instant::now())
    }

    /// Consume a weighted cost capped to this limiter's configured burst.
    ///
    /// This preserves relative weighting whenever the burst can accommodate it while ensuring a
    /// positive, otherwise-valid burst cannot make an endpoint permanently unserviceable. A
    /// disabled limiter still allows without allocating a bucket.
    #[allow(clippy::unused_async)]
    pub async fn allow_cost_capped_to_burst(&self, key: &str, cost: u64) -> bool {
        if self.inner.disabled {
            return true;
        }
        self.inner
            .shard_for(key)
            .lock()
            .allow_cost_capped_to_burst(key, cost, Instant::now())
    }

    /// Atomically consumes `count` tokens for one key.
    ///
    /// Rejection leaves the bucket unchanged, including when `count` exceeds
    /// the configured burst.
    #[allow(clippy::unused_async)]
    pub async fn allow_repeated(&self, key: &str, count: usize) -> bool {
        if self.inner.disabled || count == 0 {
            return true;
        }
        self.inner
            .shard_for(key)
            .lock()
            .allow_repeated(key, count, Instant::now())
    }

    #[cfg(test)]
    #[allow(clippy::unused_async)]
    pub(crate) async fn bucket_count(&self) -> usize {
        self.inner
            .shards
            .iter()
            .map(|shard| shard.lock().buckets.len())
            .sum()
    }
}

/// Internal header recording the remote IP the connection was accepted from.
pub const REMOTE_ADDR_HEADER: &str = "x-iroha-remote-addr";
/// Standard proxy header carrying the client/proxy address chain.
pub const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
const MAX_FORWARDED_FOR_HOPS: usize = 32;

/// Resolve the effective client IP for downstream policy decisions.
///
/// The canonical remote address header is preferred because ingress middleware
/// overwrites it with the accepted socket address or a trusted proxy-forwarded
/// client IP. Falling back to the transport address keeps direct handler
/// invocations working in narrow tests.
pub fn effective_remote_ip(headers: &HeaderMap, remote: Option<IpAddr>) -> Option<IpAddr> {
    headers
        .get(REMOTE_ADDR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .or(remote)
}

/// Resolve the remote IP that ingress middleware should inject.
///
/// If the transport peer belongs to a configured trusted proxy CIDR, the
/// `X-Forwarded-For` chain is evaluated from right to left and the first
/// untrusted address is used as the client IP. This remains safe when a proxy
/// appends to a client-supplied chain: the proxy-observed client address is
/// encountered before any attacker-selected prefix. Malformed or oversized
/// chains fail closed to the accepted transport peer.
///
/// The internal [`REMOTE_ADDR_HEADER`] is deliberately ignored here. Ingress
/// middleware writes that header only after this function returns, so accepting
/// it as input would let clients behind a trusted proxy spoof their policy IP.
pub fn ingress_remote_ip(
    headers: &HeaderMap,
    remote: Option<IpAddr>,
    trusted_proxies: &[IpNet],
) -> Option<IpAddr> {
    let remote_ip = remote?;
    if !cidr_contains(trusted_proxies, remote_ip) {
        return Some(remote_ip);
    }
    let Some(forwarded) = parse_forwarded_for_chain(headers) else {
        return Some(remote_ip);
    };
    Some(
        forwarded
            .iter()
            .rev()
            .copied()
            .find(|ip| !cidr_contains(trusted_proxies, *ip))
            .unwrap_or_else(|| forwarded[0]),
    )
}

fn parse_forwarded_for_chain(headers: &HeaderMap) -> Option<Vec<IpAddr>> {
    let mut addresses = Vec::new();
    for value in headers.get_all(FORWARDED_FOR_HEADER).iter() {
        let value = value.to_str().ok()?;
        for component in value.split(',') {
            if addresses.len() == MAX_FORWARDED_FOR_HOPS {
                return None;
            }
            let component = component.trim();
            if component.is_empty() {
                return None;
            }
            addresses.push(component.parse().ok()?);
        }
    }
    (!addresses.is_empty()).then_some(addresses)
}

/// Derive a rate-limit key from headers and optional hint:
/// - Prefer `X-API-Token` if present and token usage is enabled
/// - Else the effective client IP resolved by ingress middleware
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
    if let Some(ip) = effective_remote_ip(headers, remote) {
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
/// Uses the effective client IP resolved by ingress middleware.
pub fn is_allowed_by_cidr(headers: &HeaderMap, remote: Option<IpAddr>, allow: &[IpNet]) -> bool {
    let candidate_ip = effective_remote_ip(headers, remote);
    candidate_ip.map_or(false, |ip| cidr_contains(allow, ip))
}

/// Returns true if a forwarded header is present and the TCP peer belongs to a
/// trusted proxy CIDR.
pub fn has_trusted_forwarded_header(
    headers: &HeaderMap,
    remote: Option<IpAddr>,
    trusted_proxies: &[IpNet],
    header_name: &'static str,
) -> bool {
    let Some(remote_ip) = remote else {
        return false;
    };
    if !cidr_contains(trusted_proxies, remote_ip) {
        return false;
    }
    headers
        .get(header_name)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !value.trim().is_empty())
}

/// Configuration for the pre-authentication connection gate.
#[derive(Debug, Clone)]
pub struct PreAuthConfig {
    pub max_total: Option<usize>,
    pub max_per_ip: Option<usize>,
    pub rate_per_ip: Option<u32>,
    pub burst_per_ip: Option<u32>,
    pub ban_duration: Option<Duration>,
    pub ban_capacity: NonZeroUsize,
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
    bans: ExpiringBanStore,
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
            ban_capacity,
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
            bans: ExpiringBanStore::new(ban_capacity.get()),
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
            ban_capacity: DEFAULT_PREAUTH_BAN_CAPACITY,
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
        self.bans.is_banned_at(ip, Instant::now())
    }

    fn note_ban(&self, ip: IpAddr) {
        if let Some(duration) = self.ban_duration {
            self.bans.ban_for_at(ip, duration, Instant::now());
        }
    }

    fn release_ip(&self, ip: IpAddr) {
        if let Entry::Occupied(mut entry) = self.active_per_ip.entry(ip) {
            if *entry.get() > 1 {
                *entry.get_mut() -= 1;
            } else {
                entry.remove();
            }
        }
    }

    fn release_scheme(&self, scheme: &str) {
        if let Entry::Occupied(mut entry) = self.active_per_scheme.entry(scheme.to_owned()) {
            if *entry.get() > 1 {
                *entry.get_mut() -= 1;
            } else {
                entry.remove();
            }
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

    fn churn_ip(index: u64) -> IpAddr {
        let prefix = u128::from(0x2001_0db8_u32) << 96;
        IpAddr::V6(Ipv6Addr::from(prefix | u128::from(index)))
    }

    #[test]
    fn preauth_ban_store_caps_unique_ipv6_churn() {
        const CAPACITY: usize = 32;
        let store = ExpiringBanStore::new(CAPACITY);
        let now = Instant::now();

        for index in 0..10_000 {
            store.ban_for_at(churn_ip(index), Duration::from_secs(60), now);
        }

        assert_eq!(store.entry_count(), CAPACITY);
        assert!(store.expiry_count() <= CAPACITY * 2);
    }

    #[test]
    fn preauth_ban_store_purges_expiry_on_unrelated_lookup() {
        let store = ExpiringBanStore::new(4);
        let now = Instant::now();
        let banned = churn_ip(1);

        store.ban_for_at(banned, Duration::from_secs(1), now);
        assert!(store.is_banned_at(banned, now));
        assert!(!store.is_banned_at(churn_ip(2), now + Duration::from_secs(2)));
        assert_eq!(store.entry_count(), 0);
        assert_eq!(store.expiry_count(), 0);
    }

    #[test]
    fn preauth_ban_store_evicts_earliest_expiry_then_lowest_ip() {
        let store = ExpiringBanStore::new(2);
        let now = Instant::now();
        let lower_ip = churn_ip(1);
        let higher_ip = churn_ip(2);
        let replacement = churn_ip(3);

        store.ban_for_at(higher_ip, Duration::from_secs(10), now);
        store.ban_for_at(lower_ip, Duration::from_secs(10), now);
        store.ban_for_at(replacement, Duration::from_secs(20), now);

        assert!(!store.is_banned_at(lower_ip, now));
        assert!(store.is_banned_at(higher_ip, now));
        assert!(store.is_banned_at(replacement, now));

        let earliest = churn_ip(4);
        store.ban_for_at(earliest, Duration::from_secs(5), now);
        assert!(!store.is_banned_at(higher_ip, now));
        assert!(store.is_banned_at(earliest, now));
        store.ban_for_at(churn_ip(5), Duration::from_secs(30), now);
        assert!(!store.is_banned_at(earliest, now));
    }

    #[test]
    fn preauth_ban_store_refresh_ignores_stale_heap_records_and_compacts() {
        let store = ExpiringBanStore::new(2);
        let now = Instant::now();
        let ip = churn_ip(1);

        store.ban_for_at(ip, Duration::from_secs(1), now);
        for seconds in 2..=100 {
            store.ban_for_at(ip, Duration::from_secs(seconds), now);
        }

        assert_eq!(store.entry_count(), 1);
        assert!(store.expiry_count() <= 4);
        assert!(store.is_banned_at(ip, now + Duration::from_secs(2)));
        assert!(!store.is_banned_at(churn_ip(2), now + Duration::from_secs(101)));
        assert_eq!(store.entry_count(), 0);
    }

    #[test]
    fn preauth_ban_store_stale_expiry_cannot_remove_a_refreshed_ban() {
        let store = ExpiringBanStore::new(8);
        let now = Instant::now();
        let ip = churn_ip(1);

        store.ban_for_at(ip, Duration::from_secs(1), now);
        store.ban_for_at(ip, Duration::from_secs(10), now);
        assert_eq!(store.expiry_count(), 2);

        assert!(store.is_banned_at(ip, now + Duration::from_secs(2)));
        assert_eq!(store.entry_count(), 1);
        assert_eq!(store.expiry_count(), 1);
    }

    #[test]
    fn preauth_ban_store_ignores_zero_duration() {
        let store = ExpiringBanStore::new(1);
        let now = Instant::now();

        store.ban_for_at(churn_ip(1), Duration::ZERO, now);

        assert_eq!(store.entry_count(), 0);
        assert_eq!(store.expiry_count(), 0);
    }

    #[test]
    fn preauth_ban_store_remains_usable_after_unwind() {
        let store = ExpiringBanStore::new(1);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = store.state.lock();
            panic!("exercise lock release during unwind");
        }));
        assert!(unwound.is_err());

        let now = Instant::now();
        let ip = churn_ip(1);
        store.ban_for_at(ip, Duration::from_secs(1), now);
        assert!(store.is_banned_at(ip, now));
    }

    #[test]
    fn preauth_ban_store_preserves_capacity_under_concurrent_churn() {
        const CAPACITY: usize = 64;
        let store = Arc::new(ExpiringBanStore::new(CAPACITY));
        let now = Instant::now();
        let mut workers = Vec::new();

        for worker in 0_u64..8 {
            let store = Arc::clone(&store);
            workers.push(std::thread::spawn(move || {
                for index in 0_u64..1_000 {
                    store.ban_for_at(
                        churn_ip(worker * 1_000 + index),
                        Duration::from_secs(60),
                        now,
                    );
                }
            }));
        }
        for worker in workers {
            worker.join().expect("ban-store worker must not panic");
        }

        assert_eq!(store.entry_count(), CAPACITY);
        assert!(store.expiry_count() <= CAPACITY * 2);
    }

    #[test]
    fn preauth_gate_applies_configured_ban_capacity() {
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: None,
            max_per_ip: None,
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: Some(Duration::from_secs(60)),
            ban_capacity: NonZeroUsize::new(1).expect("test capacity is non-zero"),
            allow_nets: Vec::new(),
            scheme_limits: Vec::new(),
        });
        let first = churn_ip(1);
        let second = churn_ip(2);

        gate.inner.note_ban(first);
        gate.inner.note_ban(second);

        assert_eq!(gate.inner.bans.entry_count(), 1);
        assert!(!gate.inner.is_banned(first));
        assert!(gate.inner.is_banned(second));
    }

    #[tokio::test]
    async fn limiter_allows_then_limits() {
        let limiter = RateLimiter::new(Some(2), Some(2));
        // First two immediate requests allowed
        assert!(limiter.allow("a").await);
        assert!(limiter.allow("a").await);
        // Third should be limited
        assert!(!limiter.allow("a").await);
    }

    #[test]
    fn per_minute_rates_preserve_fractional_refill_boundaries() {
        for rate_per_minute in [1_u32, 59] {
            let limiter = RateLimiter::new_per_minute(Some(rate_per_minute), Some(1));
            let configured = limiter.inner.shards[0].lock().rate_per_sec;
            let expected = f64::from(rate_per_minute) / 60.0;
            assert!((configured - expected).abs() < f64::EPSILON);

            let mut inner = InnerLimiter::new(expected, 1.0, 1);
            let start = Instant::now();
            assert!(inner.allow_cost("boundary", 1, start));
            let refill_period = 60.0 / f64::from(rate_per_minute);
            assert!(!inner.allow_cost(
                "boundary",
                1,
                start + Duration::from_secs_f64(refill_period * 0.99),
            ));
            assert!(inner.allow_cost(
                "boundary",
                1,
                start + Duration::from_secs_f64(refill_period),
            ));
        }
    }

    #[tokio::test]
    async fn limiter_respects_costs() {
        let limiter = RateLimiter::new(Some(10), Some(10));
        assert!(limiter.allow_cost("cost", 5).await);
        assert!(limiter.allow_cost("cost", 4).await);
        // Bucket should be drained beyond burst
        assert!(!limiter.allow_cost("cost", 3).await);
    }

    #[tokio::test]
    async fn limiter_rejects_impossible_cost_without_tracking_key() {
        let limiter = RateLimiter::new(Some(10), Some(10));

        assert!(!limiter.allow_cost("too-large", 11).await);
        assert_eq!(
            limiter.bucket_count().await,
            0,
            "requests larger than burst should not allocate an unserviceable bucket"
        );
        assert!(limiter.allow_cost("too-large", 1).await);
        assert_eq!(limiter.bucket_count().await, 1);
    }

    #[tokio::test]
    async fn capped_cost_consumes_the_exact_available_burst() {
        let limiter = RateLimiter::new(Some(10), Some(3));

        assert!(limiter.allow_cost_capped_to_burst("capped", 8).await);
        assert!(
            !limiter.allow("capped").await,
            "an oversized weighted request must consume the full configured burst"
        );
    }

    #[tokio::test]
    async fn capped_cost_preserves_disabled_limiter_behavior() {
        let limiter = RateLimiter::new(None, Some(1));

        assert!(
            limiter
                .allow_cost_capped_to_burst("disabled", u64::MAX)
                .await
        );
        assert_eq!(limiter.bucket_count().await, 0);
    }

    #[test]
    fn capped_cost_preserves_fractional_refill_timing() {
        let mut limiter = InnerLimiter::new(2.0, 3.0, 1);
        let start = Instant::now();

        assert!(limiter.allow_cost_capped_to_burst("refill", 8, start));
        assert!(!limiter.allow_cost("refill", 1, start + Duration::from_millis(499)));
        assert!(limiter.allow_cost("refill", 1, start + Duration::from_millis(500)));
    }

    #[tokio::test]
    async fn limiter_existing_key_reuses_bucket() {
        let limiter = RateLimiter::new(Some(10), Some(10));

        assert!(limiter.allow("same").await);
        assert!(limiter.allow_cost("same", 2).await);
        assert!(limiter.allow_repeated("same", 3).await);
        assert_eq!(
            limiter.bucket_count().await,
            1,
            "repeated checks for an existing key should stay on one bucket"
        );
    }

    #[tokio::test]
    async fn limiter_allow_repeated_is_atomic() {
        let limiter = RateLimiter::new(Some(1), Some(2));

        assert!(!limiter.allow_repeated("batch", 3).await);
        assert!(limiter.allow("batch").await);
        assert!(limiter.allow("batch").await);
        assert!(!limiter.allow("batch").await);

        let limiter = RateLimiter::new(Some(1), Some(3));
        assert!(limiter.allow_repeated("batch", 2).await);
        assert!(limiter.allow("batch").await);
        assert!(!limiter.allow("batch").await);
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

    #[test]
    fn trusted_forwarded_header_requires_proxy_membership() {
        let trusted = parse_cidrs(&["127.0.0.0/8".to_owned()]);
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-client-cert", "cert=present".parse().unwrap());

        assert!(has_trusted_forwarded_header(
            &headers,
            Some("127.0.0.1".parse().unwrap()),
            &trusted,
            "x-forwarded-client-cert",
        ));
        assert!(!has_trusted_forwarded_header(
            &headers,
            Some("198.51.100.10".parse().unwrap()),
            &trusted,
            "x-forwarded-client-cert",
        ));
        assert!(!has_trusted_forwarded_header(
            &HeaderMap::new(),
            Some("127.0.0.1".parse().unwrap()),
            &trusted,
            "x-forwarded-client-cert",
        ));
    }

    #[tokio::test]
    async fn limiter_caps_bucket_growth() {
        let limiter = RateLimiter::new_with_capacity(Some(1), Some(1), 2);
        assert!(limiter.allow("a").await);
        assert!(limiter.allow("b").await);
        assert!(limiter.bucket_count().await <= 2);

        assert!(limiter.allow("c").await);
        // Capacity is 2, so one bucket must have been evicted.
        assert!(limiter.bucket_count().await <= 2);

        // Previously inserted keys should still be serviced without panicking.
        assert!(limiter.allow("a").await);
        assert!(limiter.bucket_count().await <= 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn limiter_allows_distinct_keys_concurrently() {
        let limiter = RateLimiter::new(Some(10_000), Some(10_000));
        let mut handles = Vec::new();

        for index in 0..128 {
            let limiter = limiter.clone();
            handles.push(tokio::spawn(async move {
                limiter.allow(&format!("client-{index}")).await
            }));
        }

        for handle in handles {
            assert!(handle.await.expect("limiter task should finish"));
        }
        assert!(limiter.bucket_count().await <= DEFAULT_MAX_BUCKETS);
    }

    #[tokio::test]
    async fn preauth_gate_limits_global_and_per_ip() {
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: Some(1),
            max_per_ip: Some(1),
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            ban_capacity: DEFAULT_PREAUTH_BAN_CAPACITY,
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
            ban_capacity: DEFAULT_PREAUTH_BAN_CAPACITY,
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
            ban_capacity: DEFAULT_PREAUTH_BAN_CAPACITY,
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
            ban_capacity: DEFAULT_PREAUTH_BAN_CAPACITY,
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

    #[tokio::test]
    async fn preauth_permit_releases_every_counter_during_unwind() {
        let gate = PreAuthGate::new(PreAuthConfig {
            max_total: Some(1),
            max_per_ip: Some(1),
            rate_per_ip: None,
            burst_per_ip: None,
            ban_duration: None,
            ban_capacity: DEFAULT_PREAUTH_BAN_CAPACITY,
            allow_nets: Vec::new(),
            scheme_limits: vec![SchemeLimit {
                name: "http".to_owned(),
                max_connections: 1,
            }],
        });
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        let permit = gate
            .acquire(Some(ip), Some("http"))
            .await
            .expect("first connection acquires every counter");

        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _permit = permit;
            panic!("exercise pre-auth permit unwind");
        }));
        assert!(unwound.is_err());

        gate.acquire(Some(ip), Some("http"))
            .await
            .expect("unwind releases global, IP, and scheme counters exactly once");
    }

    #[test]
    fn preauth_counter_release_is_atomic_with_concurrent_reacquire() {
        const ITERATIONS: usize = 20_000;

        let gate = PreAuthGate::disabled();
        let inner = Arc::clone(&gate.inner);
        let ip: IpAddr = "203.0.113.55".parse().expect("valid test address");
        let scheme = "http".to_owned();
        inner.active_per_ip.insert(ip, 1);
        inner.active_per_scheme.insert(scheme.clone(), 1);

        let start = Arc::new(std::sync::Barrier::new(3));
        let finish = Arc::new(std::sync::Barrier::new(3));

        let release_worker = {
            let inner = Arc::clone(&inner);
            let start = Arc::clone(&start);
            let finish = Arc::clone(&finish);
            let scheme = scheme.clone();
            std::thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    start.wait();
                    inner.release_ip(ip);
                    inner.release_scheme(&scheme);
                    finish.wait();
                }
            })
        };
        let acquire_worker = {
            let inner = Arc::clone(&inner);
            let start = Arc::clone(&start);
            let finish = Arc::clone(&finish);
            let scheme = scheme.clone();
            std::thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    start.wait();
                    *inner.active_per_ip.entry(ip).or_insert(0) += 1;
                    *inner.active_per_scheme.entry(scheme.clone()).or_insert(0) += 1;
                    finish.wait();
                }
            })
        };

        for iteration in 0..ITERATIONS {
            start.wait();
            finish.wait();
            assert_eq!(
                inner.active_per_ip.get(&ip).map(|entry| *entry),
                Some(1),
                "IP counter lost a concurrent acquisition at iteration {iteration}"
            );
            assert_eq!(
                inner.active_per_scheme.get(&scheme).map(|entry| *entry),
                Some(1),
                "scheme counter lost a concurrent acquisition at iteration {iteration}"
            );
        }

        release_worker
            .join()
            .expect("release worker must not panic");
        acquire_worker
            .join()
            .expect("acquire worker must not panic");
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
    fn key_from_headers_prefers_injected_header() {
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_ADDR_HEADER, "203.0.113.55".parse().unwrap());
        assert_eq!(
            key_from_headers(
                &headers,
                Some("198.51.100.1".parse().unwrap()),
                Some("hint"),
                true
            ),
            "203.0.113.55"
        );
    }

    #[test]
    fn ingress_remote_ip_uses_trusted_forwarded_for_chain() {
        let mut headers = HeaderMap::new();
        headers.insert(FORWARDED_FOR_HEADER, "203.0.113.55".parse().unwrap());
        let trusted = parse_cidrs(&["127.0.0.0/8".into()]);
        assert_eq!(
            ingress_remote_ip(&headers, Some("127.0.0.1".parse().unwrap()), &trusted),
            Some("203.0.113.55".parse().unwrap())
        );
    }

    #[test]
    fn ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(FORWARDED_FOR_HEADER, "203.0.113.55".parse().unwrap());
        let trusted = parse_cidrs(&["127.0.0.0/8".into()]);
        assert_eq!(
            ingress_remote_ip(&headers, Some("198.51.100.10".parse().unwrap()), &trusted),
            Some("198.51.100.10".parse().unwrap())
        );
    }

    #[test]
    fn ingress_remote_ip_rejects_client_spoofed_internal_header() {
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_ADDR_HEADER, "203.0.113.55".parse().unwrap());
        let trusted = parse_cidrs(&["127.0.0.0/8".into()]);
        assert_eq!(
            ingress_remote_ip(&headers, Some("127.0.0.1".parse().unwrap()), &trusted),
            Some("127.0.0.1".parse().unwrap())
        );
    }

    #[test]
    fn ingress_remote_ip_ignores_attacker_prefix_before_proxy_observation() {
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED_FOR_HEADER,
            "192.0.2.10, 198.51.100.42".parse().unwrap(),
        );
        let trusted = parse_cidrs(&["127.0.0.0/8".into()]);
        assert_eq!(
            ingress_remote_ip(&headers, Some("127.0.0.1".parse().unwrap()), &trusted),
            Some("198.51.100.42".parse().unwrap())
        );
    }

    #[test]
    fn ingress_remote_ip_falls_back_to_proxy_for_malformed_chain() {
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED_FOR_HEADER,
            "203.0.113.55, not-an-ip".parse().unwrap(),
        );
        let trusted = parse_cidrs(&["127.0.0.0/8".into()]);
        assert_eq!(
            ingress_remote_ip(&headers, Some("127.0.0.1".parse().unwrap()), &trusted),
            Some("127.0.0.1".parse().unwrap())
        );
    }

    #[test]
    fn ingress_remote_ip_falls_back_to_proxy_for_oversized_chain() {
        let mut headers = HeaderMap::new();
        let forwarded = std::iter::repeat_n("203.0.113.55", MAX_FORWARDED_FOR_HOPS + 1)
            .collect::<Vec<_>>()
            .join(", ");
        headers.insert(FORWARDED_FOR_HEADER, forwarded.parse().unwrap());
        let trusted = parse_cidrs(&["127.0.0.0/8".into()]);
        assert_eq!(
            ingress_remote_ip(&headers, Some("127.0.0.1".parse().unwrap()), &trusted),
            Some("127.0.0.1".parse().unwrap())
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
    fn is_allowed_by_cidr_prefers_effective_remote_ip() {
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
    fn is_allowed_by_cidr_prefers_injected_header() {
        let allow = vec![parse_cidr("203.0.113.0/24").unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_ADDR_HEADER, "203.0.113.55".parse().unwrap());
        assert!(is_allowed_by_cidr(
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
