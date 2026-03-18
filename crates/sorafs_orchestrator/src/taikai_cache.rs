//! Taikai cache hierarchy primitives used by the SoraNet distribution plane.
//!
//! This module seeds roadmap item **SNNet-14 – Taikai cache hierarchy &
//! QoS enforcement** with a deterministic three-tier cache, QoS token buckets,
//! and shard placement helpers. The implementation focuses on predictable
//! behaviour and unit coverage so follow-up work (CAA gossip, pull/push
//! orchestration, exit hedging, observability) can build on top of a solid
//! foundation.

use std::{
    collections::{HashMap, VecDeque},
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use blake3::hash as blake3_hash;
use iroha_crypto::{KeyPair, PublicKey, Signature};
use iroha_data_model::taikai::{
    GuardDirectoryId, TaikaiEventId, TaikaiRenditionId, TaikaiSegmentEnvelopeV1, TaikaiStreamId,
};
use iroha_telemetry::metrics::global_or_default;
use norito::{
    core::Error as NoritoError,
    derive::{NoritoDeserialize, NoritoSerialize},
    to_bytes,
};
use rand::RngCore;
use thiserror::Error;

/// Cache tiers tracked by the hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, Hash)]
pub enum CacheTierKind {
    Hot,
    Warm,
    Cold,
}

impl CacheTierKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Cold => "cold",
        }
    }
}

/// QoS class assigned to cached segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, NoritoSerialize, NoritoDeserialize)]
pub enum QosClass {
    Priority,
    Standard,
    Bulk,
}

impl QosClass {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Priority => "priority",
            Self::Standard => "standard",
            Self::Bulk => "bulk",
        }
    }
}

/// Logical identifier for a Taikai segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, NoritoSerialize, NoritoDeserialize)]
pub struct SegmentKey {
    event_id: TaikaiEventId,
    stream_id: TaikaiStreamId,
    rendition_id: TaikaiRenditionId,
    segment_sequence: u64,
}

impl SegmentKey {
    #[must_use]
    pub fn new(
        event_id: TaikaiEventId,
        stream_id: TaikaiStreamId,
        rendition_id: TaikaiRenditionId,
        segment_sequence: u64,
    ) -> Self {
        Self {
            event_id,
            stream_id,
            rendition_id,
            segment_sequence,
        }
    }

    #[must_use]
    pub fn from_envelope(envelope: &TaikaiSegmentEnvelopeV1) -> Self {
        Self {
            event_id: envelope.event_id.clone(),
            stream_id: envelope.stream_id.clone(),
            rendition_id: envelope.rendition_id.clone(),
            segment_sequence: envelope.segment_sequence,
        }
    }

    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.segment_sequence
    }
}

/// Cached segment (metadata + payload).
#[derive(Debug, Clone)]
pub struct CachedSegment {
    envelope: TaikaiSegmentEnvelopeV1,
    payload: Arc<[u8]>,
    qos: QosClass,
}

impl CachedSegment {
    #[must_use]
    pub fn new(envelope: TaikaiSegmentEnvelopeV1, payload: Arc<[u8]>, qos: QosClass) -> Self {
        Self {
            envelope,
            payload,
            qos,
        }
    }

    #[must_use]
    pub fn key(&self) -> SegmentKey {
        SegmentKey::from_envelope(&self.envelope)
    }

    #[must_use]
    pub fn size_bytes(&self) -> u64 {
        u64::try_from(self.payload.len()).expect("payload length fits into u64")
    }

    #[must_use]
    pub fn qos(&self) -> QosClass {
        self.qos
    }

    #[must_use]
    pub fn envelope(&self) -> &TaikaiSegmentEnvelopeV1 {
        &self.envelope
    }

    #[must_use]
    pub fn payload(&self) -> Arc<[u8]> {
        Arc::clone(&self.payload)
    }
}

/// Cache configuration values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaikaiCacheConfig {
    pub hot_capacity_bytes: u64,
    pub hot_retention: Duration,
    pub warm_capacity_bytes: u64,
    pub warm_retention: Duration,
    pub cold_capacity_bytes: u64,
    pub cold_retention: Duration,
    pub qos: QosConfig,
    pub reliability: ReliabilityTuning,
}

impl Default for TaikaiCacheConfig {
    fn default() -> Self {
        Self {
            hot_capacity_bytes: 256 * 1024 * 1024,
            hot_retention: Duration::from_secs(120),
            warm_capacity_bytes: 4 * 1024 * 1024 * 1024,
            warm_retention: Duration::from_secs(600),
            cold_capacity_bytes: 16 * 1024 * 1024 * 1024,
            cold_retention: Duration::from_secs(86_400),
            qos: QosConfig::balanced(),
            reliability: ReliabilityTuning::default(),
        }
    }
}

impl TaikaiCacheConfig {
    #[must_use]
    pub(crate) fn reliability_config(&self) -> ReliabilityConfig {
        let open_secs = self.reliability.open_secs.max(1);
        ReliabilityConfig::new(
            self.reliability.failures_to_trip.max(1),
            Duration::from_secs(open_secs),
        )
    }
}

/// QoS configuration shared by the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosConfig {
    pub priority_rate_bps: u64,
    pub standard_rate_bps: u64,
    pub bulk_rate_bps: u64,
    pub burst_multiplier: u32,
}

impl QosConfig {
    #[must_use]
    pub fn balanced() -> Self {
        Self {
            priority_rate_bps: 50 * 1024 * 1024,
            standard_rate_bps: 20 * 1024 * 1024,
            bulk_rate_bps: 8 * 1024 * 1024,
            burst_multiplier: 3,
        }
    }

    fn params_for(&self, class: QosClass) -> (u64, u64) {
        let rate = match class {
            QosClass::Priority => self.priority_rate_bps,
            QosClass::Standard => self.standard_rate_bps,
            QosClass::Bulk => self.bulk_rate_bps,
        };
        let capacity = rate.saturating_mul(self.burst_multiplier.into());
        (rate, capacity)
    }
}

/// Reliability tuning applied to shard selection and circuit breakers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReliabilityTuning {
    pub failures_to_trip: u32,
    pub open_secs: u64,
}

impl ReliabilityTuning {
    #[must_use]
    pub fn new(failures_to_trip: u32, open_secs: u64) -> Self {
        Self {
            failures_to_trip,
            open_secs,
        }
    }
}

impl Default for ReliabilityTuning {
    fn default() -> Self {
        Self {
            failures_to_trip: ReliabilityConfig::default().failures_to_trip,
            open_secs: ReliabilityConfig::default().open_for.as_secs(),
        }
    }
}

/// Reliability settings applied to shard selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReliabilityConfig {
    failures_to_trip: u32,
    open_for: Duration,
}

impl ReliabilityConfig {
    fn new(failures_to_trip: u32, open_for: Duration) -> Self {
        let failures_to_trip = failures_to_trip.max(1);
        let open_for = if open_for.is_zero() {
            Duration::from_millis(1)
        } else {
            open_for
        };
        Self {
            failures_to_trip,
            open_for,
        }
    }
}

impl Default for ReliabilityConfig {
    fn default() -> Self {
        // Trip after three consecutive failures and hold the circuit open for 2 seconds.
        Self::new(3, Duration::from_secs(2))
    }
}

/// QoS enforcement error.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum QosError {
    #[error("QoS class {class:?} depleted {needed} bytes; {available} bytes available")]
    RateLimited {
        class: QosClass,
        needed: u64,
        available: u64,
    },
}

#[derive(Debug)]
struct TokenBucket {
    capacity: f64,
    refill_rate: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u64, refill_rate: u64, now: Instant) -> Self {
        let capacity = capacity as f64;
        let refill_rate = refill_rate as f64;
        Self {
            capacity,
            refill_rate,
            tokens: capacity,
            last_refill: now,
        }
    }

    fn refill(&mut self, now: Instant) {
        if now <= self.last_refill {
            return;
        }
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
    }

    fn try_consume(&mut self, amount: u64, now: Instant) -> Result<(), QosError> {
        self.refill(now);
        let amount = amount as f64;
        if self.tokens >= amount {
            self.tokens -= amount;
            Ok(())
        } else {
            Err(QosError::RateLimited {
                class: QosClass::Priority, // overwritten by caller
                needed: amount as u64,
                available: self.tokens.trunc() as u64,
            })
        }
    }
}

#[derive(Debug)]
struct QosEnforcer {
    buckets: HashMap<QosClass, TokenBucket>,
}

impl QosEnforcer {
    fn new(config: &QosConfig, now: Instant) -> Self {
        let mut buckets = HashMap::new();
        for class in [QosClass::Priority, QosClass::Standard, QosClass::Bulk] {
            let (rate, capacity) = config.params_for(class);
            buckets.insert(class, TokenBucket::new(capacity, rate, now));
        }
        Self { buckets }
    }

    fn try_acquire(&mut self, class: QosClass, amount: u64, now: Instant) -> Result<(), QosError> {
        let bucket = self.buckets.get_mut(&class).expect("QoS bucket must exist");
        match bucket.try_consume(amount, now) {
            Ok(()) => Ok(()),
            Err(QosError::RateLimited {
                needed, available, ..
            }) => Err(QosError::RateLimited {
                class,
                needed,
                available,
            }),
        }
    }
}

#[derive(Debug, Default)]
struct CircuitState {
    failures: u32,
    open_until: Option<Instant>,
}

impl CircuitState {
    fn refresh(&mut self, now: Instant) -> bool {
        match self.open_until {
            Some(deadline) if deadline <= now => {
                self.open_until = None;
                self.failures = 0;
                true
            }
            _ => false,
        }
    }

    fn is_open(&self, now: Instant) -> bool {
        matches!(self.open_until, Some(deadline) if deadline > now)
    }

    fn record_failure(&mut self, config: &ReliabilityConfig, now: Instant) -> bool {
        self.failures = self.failures.saturating_add(1);
        if self.failures >= config.failures_to_trip {
            self.open_until = Some(now + config.open_for);
            self.failures = 0;
            return true;
        }
        false
    }

    fn record_success(&mut self) -> bool {
        let was_open = self.open_until.take().is_some();
        self.failures = 0;
        was_open
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReliabilityStats {
    open_circuits: u64,
    failovers: u64,
}

#[derive(Debug, Default)]
struct ShardReliability {
    config: ReliabilityConfig,
    states: HashMap<TaikaiShardId, CircuitState>,
    failovers: u64,
}

impl ShardReliability {
    fn new(config: ReliabilityConfig) -> Self {
        Self {
            config,
            states: HashMap::new(),
            failovers: 0,
        }
    }

    fn select(&mut self, ring: &TaikaiShardRing, key: &SegmentKey, now: Instant) -> ShardSelection {
        let Some((start_index, preferred)) = ring.locate_with_index(key) else {
            return ShardSelection {
                preferred: None,
                selected: None,
                failover: false,
            };
        };

        let ring_len = ring.len();
        let mut selected = None;
        for offset in 0..ring_len {
            let index = (start_index + offset) % ring_len;
            let shard = ring.shard_at(index).expect("shard exists at index");
            let state = self.states.entry(shard).or_default();
            let _ = state.refresh(now);
            if !state.is_open(now) {
                selected = Some(shard);
                if offset > 0 {
                    self.failovers = self.failovers.saturating_add(1);
                }
                break;
            }
        }

        ShardSelection {
            preferred: Some(preferred),
            selected,
            failover: selected.is_some_and(|shard| Some(shard) != Some(preferred)),
        }
    }

    fn record_failure(&mut self, shard: TaikaiShardId, now: Instant) -> bool {
        let state = self.states.entry(shard).or_default();
        state.refresh(now);
        state.record_failure(&self.config, now)
    }

    fn record_success(&mut self, shard: TaikaiShardId, now: Instant) -> bool {
        let state = self.states.entry(shard).or_default();
        state.refresh(now);
        state.record_success()
    }

    fn shard_open(&mut self, shard: TaikaiShardId, now: Instant) -> bool {
        let state = self.states.entry(shard).or_default();
        let _ = state.refresh(now);
        state.is_open(now)
    }

    fn stats(&mut self, now: Instant) -> ReliabilityStats {
        let mut open_circuits = 0;
        for (shard, state) in self.states.iter_mut() {
            let _ = state.refresh(now);
            let open = state.is_open(now);
            set_taikai_shard_open(*shard, open);
            if open {
                open_circuits += 1;
            }
        }
        ReliabilityStats {
            open_circuits,
            failovers: self.failovers,
        }
    }
}

#[derive(Debug)]
struct CacheEntry {
    segment: Arc<CachedSegment>,
    size_bytes: u64,
    inserted_at: Instant,
    last_accessed: Instant,
}

impl CacheEntry {
    fn new(segment: Arc<CachedSegment>, now: Instant) -> Self {
        let size_bytes = segment.size_bytes();
        Self {
            segment,
            size_bytes,
            inserted_at: now,
            last_accessed: now,
        }
    }
}

#[derive(Debug)]
struct CacheTier {
    _kind: CacheTierKind,
    capacity_bytes: u64,
    retention: Duration,
    entries: HashMap<SegmentKey, CacheEntry>,
    order: VecDeque<SegmentKey>,
    used_bytes: u64,
}

impl CacheTier {
    fn new(kind: CacheTierKind, capacity_bytes: u64, retention: Duration) -> Self {
        Self {
            _kind: kind,
            capacity_bytes,
            retention,
            entries: HashMap::new(),
            order: VecDeque::new(),
            used_bytes: 0,
        }
    }

    fn retention(&self) -> Option<Duration> {
        if self.retention.is_zero() {
            None
        } else {
            Some(self.retention)
        }
    }

    fn insert(&mut self, segment: Arc<CachedSegment>, now: Instant) -> CacheTierInsertResult {
        let expired = self.collect_expired(now);
        if self.capacity_bytes == 0 {
            return CacheTierInsertResult {
                inserted: false,
                demoted: Vec::new(),
                expired,
            };
        }

        let key = segment.key();
        let size_bytes = segment.size_bytes();
        let mut demoted = Vec::new();

        if let Some(entry) = self.entries.get_mut(&key) {
            self.used_bytes = self
                .used_bytes
                .saturating_sub(entry.size_bytes)
                .saturating_add(size_bytes);
            entry.segment = segment;
            entry.size_bytes = size_bytes;
            entry.last_accessed = now;
            entry.inserted_at = now;
            let _ = entry;
            self.touch(&key);
            return CacheTierInsertResult {
                inserted: true,
                demoted,
                expired,
            };
        }

        if size_bytes > self.capacity_bytes {
            return CacheTierInsertResult {
                inserted: false,
                demoted,
                expired,
            };
        }

        while self.used_bytes + size_bytes > self.capacity_bytes {
            if let Some((victim_key, victim_segment)) = self.evict_lru() {
                demoted.push((victim_key, victim_segment));
            } else {
                break;
            }
        }

        if self.used_bytes + size_bytes > self.capacity_bytes {
            return CacheTierInsertResult {
                inserted: false,
                demoted,
                expired,
            };
        }

        self.order.push_back(key.clone());
        self.used_bytes += size_bytes;
        self.entries.insert(key, CacheEntry::new(segment, now));
        CacheTierInsertResult {
            inserted: true,
            demoted,
            expired,
        }
    }

    fn get(&mut self, key: &SegmentKey, now: Instant) -> Option<Arc<CachedSegment>> {
        self.collect_expired(now);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = now;
            let segment = entry.segment.clone();
            let _ = entry;
            self.touch(key);
            Some(segment)
        } else {
            None
        }
    }

    fn collect_expired(&mut self, now: Instant) -> Vec<SegmentKey> {
        let Some(retention) = self.retention() else {
            return Vec::new();
        };
        let mut expired = Vec::new();
        while let Some(front) = self.order.front() {
            let remove = match self.entries.get(front) {
                Some(entry) => now
                    .checked_duration_since(entry.last_accessed)
                    .is_some_and(|elapsed| elapsed >= retention),
                None => true,
            };
            if remove {
                let key = self.order.pop_front().expect("front existed");
                if let Some(entry) = self.entries.remove(&key) {
                    self.used_bytes = self.used_bytes.saturating_sub(entry.size_bytes);
                    expired.push(key);
                }
            } else {
                break;
            }
        }
        expired
    }

    fn touch(&mut self, key: &SegmentKey) {
        if let Some(pos) = self.order.iter().position(|candidate| candidate == key)
            && pos + 1 != self.order.len()
            && let Some(entry) = self.order.remove(pos)
        {
            self.order.push_back(entry);
        }
    }

    fn evict_lru(&mut self) -> Option<(SegmentKey, Arc<CachedSegment>)> {
        if let Some(key) = self.order.pop_front()
            && let Some(entry) = self.entries.remove(&key)
        {
            self.used_bytes = self.used_bytes.saturating_sub(entry.size_bytes);
            return Some((key, entry.segment));
        }
        None
    }
}

struct CacheTierInsertResult {
    inserted: bool,
    demoted: Vec<(SegmentKey, Arc<CachedSegment>)>,
    expired: Vec<SegmentKey>,
}

/// Eviction metadata.
#[derive(Debug, Clone)]
pub struct CacheEviction {
    pub from: CacheTierKind,
    pub key: SegmentKey,
    pub reason: CacheEvictionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEvictionReason {
    Expired,
    Capacity,
}

impl CacheEvictionReason {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Expired => "expired",
            Self::Capacity => "capacity",
        }
    }
}

/// Promotion metadata captured during cache hits.
#[derive(Debug, Clone)]
pub struct CachePromotion {
    pub from: CacheTierKind,
    pub to: CacheTierKind,
    pub key: SegmentKey,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TierStats {
    pub hot: u64,
    pub warm: u64,
    pub cold: u64,
}

impl TierStats {
    pub(crate) fn increment(&mut self, tier: CacheTierKind) {
        match tier {
            CacheTierKind::Hot => self.hot += 1,
            CacheTierKind::Warm => self.warm += 1,
            CacheTierKind::Cold => self.cold += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReasonStats {
    pub expired: u64,
    pub capacity: u64,
}

impl ReasonStats {
    pub(crate) fn increment(&mut self, reason: CacheEvictionReason) {
        match reason {
            CacheEvictionReason::Expired => self.expired += 1,
            CacheEvictionReason::Capacity => self.capacity += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvictionStats {
    pub hot: ReasonStats,
    pub warm: ReasonStats,
    pub cold: ReasonStats,
}

impl EvictionStats {
    pub(crate) fn increment(&mut self, tier: CacheTierKind, reason: CacheEvictionReason) {
        match tier {
            CacheTierKind::Hot => self.hot.increment(reason),
            CacheTierKind::Warm => self.warm.increment(reason),
            CacheTierKind::Cold => self.cold.increment(reason),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PromotionStats {
    pub warm_to_hot: u64,
    pub cold_to_warm: u64,
    pub cold_to_hot: u64,
}

impl PromotionStats {
    pub(crate) fn increment(&mut self, from: CacheTierKind, to: CacheTierKind) {
        match (from, to) {
            (CacheTierKind::Warm, CacheTierKind::Hot) => self.warm_to_hot += 1,
            (CacheTierKind::Cold, CacheTierKind::Warm) => self.cold_to_warm += 1,
            (CacheTierKind::Cold, CacheTierKind::Hot) => self.cold_to_hot += 1,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QosStats {
    pub priority: u64,
    pub standard: u64,
    pub bulk: u64,
}

impl QosStats {
    pub(crate) fn increment(&mut self, class: QosClass) {
        match class {
            QosClass::Priority => self.priority += 1,
            QosClass::Standard => self.standard += 1,
            QosClass::Bulk => self.bulk += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaikaiCacheStatsSnapshot {
    pub hits: TierStats,
    pub misses: u64,
    pub inserts: TierStats,
    pub evictions: EvictionStats,
    pub promotions: PromotionStats,
    pub qos_denials: QosStats,
}

impl TaikaiCacheStatsSnapshot {
    pub(crate) fn record_hit(&mut self, tier: CacheTierKind) {
        self.hits.increment(tier);
    }

    pub(crate) fn record_miss(&mut self) {
        self.misses += 1;
    }

    pub(crate) fn record_insert(&mut self, tier: CacheTierKind) {
        self.inserts.increment(tier);
    }

    pub(crate) fn record_eviction(&mut self, tier: CacheTierKind, reason: CacheEvictionReason) {
        self.evictions.increment(tier, reason);
    }

    pub(crate) fn record_promotion(&mut self, from: CacheTierKind, to: CacheTierKind) {
        self.promotions.increment(from, to);
    }

    pub(crate) fn record_qos_denial(&mut self, class: QosClass) {
        self.qos_denials.increment(class);
    }
}

/// Insertion outcome summary.
#[derive(Debug, Default, Clone)]
pub struct TaikaiCacheInsertOutcome {
    pub inserted_into: Vec<CacheTierKind>,
    pub evicted: Vec<CacheEviction>,
}

impl TaikaiCacheInsertOutcome {
    fn record_insert(&mut self, tier: CacheTierKind) {
        self.inserted_into.push(tier);
    }

    fn record_capacity_evictions(
        &mut self,
        tier: CacheTierKind,
        entries: Vec<(SegmentKey, Arc<CachedSegment>)>,
    ) -> Vec<(SegmentKey, Arc<CachedSegment>)> {
        for (key, _) in &entries {
            self.evicted.push(CacheEviction {
                from: tier,
                key: key.clone(),
                reason: CacheEvictionReason::Capacity,
            });
        }
        entries
    }

    fn record_expirations(&mut self, tier: CacheTierKind, keys: Vec<SegmentKey>) {
        for key in keys {
            self.evicted.push(CacheEviction {
                from: tier,
                key,
                reason: CacheEvictionReason::Expired,
            });
        }
    }
}

/// Result returned by [`TaikaiCache::get`].
#[derive(Debug, Clone, Default)]
pub struct TaikaiCacheQueryOutcome {
    pub segment: Option<Arc<CachedSegment>>,
    pub hit_tier: Option<CacheTierKind>,
    pub promotions: Vec<CachePromotion>,
}

/// Hierarchical Taikai cache.
#[derive(Debug)]
pub struct TaikaiCache {
    hot: CacheTier,
    warm: CacheTier,
    cold: CacheTier,
    qos: QosEnforcer,
    stats: TaikaiCacheStatsSnapshot,
}

impl TaikaiCache {
    #[must_use]
    pub fn new(config: TaikaiCacheConfig) -> Self {
        let now = Instant::now();
        Self {
            hot: CacheTier::new(
                CacheTierKind::Hot,
                config.hot_capacity_bytes,
                config.hot_retention,
            ),
            warm: CacheTier::new(
                CacheTierKind::Warm,
                config.warm_capacity_bytes,
                config.warm_retention,
            ),
            cold: CacheTier::new(
                CacheTierKind::Cold,
                config.cold_capacity_bytes,
                config.cold_retention,
            ),
            qos: QosEnforcer::new(&config.qos, now),
            stats: TaikaiCacheStatsSnapshot::default(),
        }
    }

    pub fn try_acquire_qos(
        &mut self,
        class: QosClass,
        bytes: u64,
        now: Instant,
    ) -> Result<(), QosError> {
        match self.qos.try_acquire(class, bytes, now) {
            Ok(()) => Ok(()),
            Err(err @ QosError::RateLimited { class, .. }) => {
                record_taikai_qos_denial(class);
                self.stats.record_qos_denial(class);
                Err(err)
            }
        }
    }

    pub fn insert(&mut self, segment: CachedSegment, now: Instant) -> TaikaiCacheInsertOutcome {
        let shared = Arc::new(segment);
        self.insert_shared(shared, now)
    }

    fn insert_shared(
        &mut self,
        segment: Arc<CachedSegment>,
        now: Instant,
    ) -> TaikaiCacheInsertOutcome {
        let mut outcome = TaikaiCacheInsertOutcome::default();

        let CacheTierInsertResult {
            inserted,
            demoted,
            expired,
        } = self.hot.insert(segment.clone(), now);
        outcome.record_expirations(CacheTierKind::Hot, expired);
        let mut to_demote = outcome.record_capacity_evictions(CacheTierKind::Hot, demoted);
        if inserted {
            outcome.record_insert(CacheTierKind::Hot);
            self.stats.record_insert(CacheTierKind::Hot);
        }

        while let Some((key, demoted_segment)) = to_demote.pop() {
            let CacheTierInsertResult {
                inserted: warm_inserted,
                demoted: warm_demoted,
                expired: warm_expired,
            } = self.warm.insert(demoted_segment.clone(), now);
            outcome.record_expirations(CacheTierKind::Warm, warm_expired);
            let mut next_demote =
                outcome.record_capacity_evictions(CacheTierKind::Warm, warm_demoted);
            if warm_inserted {
                outcome.record_insert(CacheTierKind::Warm);
                self.stats.record_insert(CacheTierKind::Warm);
            }
            if !warm_inserted {
                next_demote.push((key, demoted_segment.clone()));
            }
            while let Some((_cold_key, cold_segment)) = next_demote.pop() {
                let CacheTierInsertResult {
                    inserted: cold_inserted,
                    demoted: cold_demoted,
                    expired: cold_expired,
                } = self.cold.insert(cold_segment.clone(), now);
                outcome.record_expirations(CacheTierKind::Cold, cold_expired);
                let _ = outcome.record_capacity_evictions(CacheTierKind::Cold, cold_demoted);
                if cold_inserted {
                    outcome.record_insert(CacheTierKind::Cold);
                    self.stats.record_insert(CacheTierKind::Cold);
                }
            }
        }

        record_taikai_cache_insert_metrics(&segment, &outcome);
        for eviction in &outcome.evicted {
            self.stats.record_eviction(eviction.from, eviction.reason);
        }
        outcome
    }

    pub fn get(&mut self, key: &SegmentKey, now: Instant) -> TaikaiCacheQueryOutcome {
        let mut outcome = TaikaiCacheQueryOutcome::default();

        if let Some(segment) = self.hot.get(key, now) {
            outcome.hit_tier = Some(CacheTierKind::Hot);
            outcome.segment = Some(segment);
            self.stats.record_hit(CacheTierKind::Hot);
        } else if let Some(segment) = self.warm.get(key, now) {
            outcome.hit_tier = Some(CacheTierKind::Warm);
            outcome.promotions.push(CachePromotion {
                from: CacheTierKind::Warm,
                to: CacheTierKind::Hot,
                key: key.clone(),
            });
            let shared = segment.clone();
            self.insert_shared(shared.clone(), now);
            outcome.segment = Some(segment);
            self.stats.record_hit(CacheTierKind::Warm);
        } else if let Some(segment) = self.cold.get(key, now) {
            outcome.hit_tier = Some(CacheTierKind::Cold);
            outcome.promotions.push(CachePromotion {
                from: CacheTierKind::Cold,
                to: CacheTierKind::Warm,
                key: key.clone(),
            });
            outcome.promotions.push(CachePromotion {
                from: CacheTierKind::Warm,
                to: CacheTierKind::Hot,
                key: key.clone(),
            });
            let shared = segment.clone();
            self.warm.insert(shared.clone(), now);
            self.insert_shared(shared.clone(), now);
            outcome.segment = Some(segment);
            self.stats.record_hit(CacheTierKind::Cold);
        } else {
            self.stats.record_miss();
        }

        record_taikai_cache_query_metrics(&outcome);
        for promotion in &outcome.promotions {
            self.stats.record_promotion(promotion.from, promotion.to);
        }
        outcome
    }

    pub fn purge_expired(&mut self, now: Instant) -> Vec<CacheEviction> {
        let mut evicted = Vec::new();
        for (tier, cache) in [
            (CacheTierKind::Hot, &mut self.hot),
            (CacheTierKind::Warm, &mut self.warm),
            (CacheTierKind::Cold, &mut self.cold),
        ] {
            let expired = cache.collect_expired(now);
            for key in expired {
                self.stats
                    .record_eviction(tier, CacheEvictionReason::Expired);
                evicted.push(CacheEviction {
                    from: tier,
                    key,
                    reason: CacheEvictionReason::Expired,
                });
            }
        }
        record_taikai_cache_evictions(&evicted);
        evicted
    }

    #[must_use]
    pub fn stats(&self) -> TaikaiCacheStatsSnapshot {
        self.stats
    }
}

/// Cache shard identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct TaikaiShardId(pub u16);

/// Consistent hash ring.
#[derive(Debug, Clone)]
pub struct TaikaiShardRing {
    replicas: NonZeroUsize,
    ring: Vec<(u64, TaikaiShardId)>,
}

impl TaikaiShardRing {
    #[must_use]
    pub fn new(shards: Vec<TaikaiShardId>) -> Self {
        Self::with_replicas(shards, NonZeroUsize::new(128).unwrap())
    }

    #[must_use]
    pub fn with_replicas(shards: Vec<TaikaiShardId>, replicas: NonZeroUsize) -> Self {
        let mut ring = Vec::with_capacity(shards.len() * replicas.get());
        for shard in shards {
            for replica in 0..replicas.get() {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                shard.hash(&mut hasher);
                replica.hash(&mut hasher);
                ring.push((hasher.finish(), shard));
            }
        }
        ring.sort_by_key(|entry| entry.0);
        Self { replicas, ring }
    }

    #[must_use]
    pub fn locate(&self, key: &SegmentKey) -> Option<TaikaiShardId> {
        self.locate_with_index(key).map(|(_, shard)| shard)
    }

    #[must_use]
    pub fn locate_with_index(&self, key: &SegmentKey) -> Option<(usize, TaikaiShardId)> {
        if self.ring.is_empty() {
            return None;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        let index = match self.ring.binary_search_by(|probe| probe.0.cmp(&hash)) {
            Ok(index) => index,
            Err(insert_pos) => {
                if insert_pos >= self.ring.len() {
                    0
                } else {
                    insert_pos
                }
            }
        };
        Some((index, self.ring[index].1))
    }

    #[must_use]
    pub fn shard_at(&self, index: usize) -> Option<TaikaiShardId> {
        self.ring.get(index).map(|entry| entry.1)
    }

    #[must_use]
    pub fn replicas(&self) -> NonZeroUsize {
        self.replicas
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

/// Identifier assigned to a pull batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaikaiPullBatchId(u64);

impl TaikaiPullBatchId {
    fn next(prev: &mut u64) -> Self {
        *prev = prev.wrapping_add(1).max(1);
        Self(*prev)
    }
}

/// Pull request tracked by the Taikai cache queue.
#[derive(Debug, Clone)]
pub struct TaikaiPullRequest {
    pub key: SegmentKey,
    pub qos: QosClass,
    pub size_bytes: u64,
    pub payload_digest: Option<[u8; 32]>,
}

impl TaikaiPullRequest {
    #[must_use]
    pub fn new(
        key: SegmentKey,
        qos: QosClass,
        size_bytes: u64,
        payload_digest: Option<[u8; 32]>,
    ) -> Self {
        Self {
            key,
            qos,
            size_bytes,
            payload_digest,
        }
    }
}

/// Batched pull issued to cache peers or exits.
#[derive(Debug, Clone)]
pub struct TaikaiPullBatch {
    pub id: TaikaiPullBatchId,
    pub shard: Option<TaikaiShardId>,
    pub qos: QosClass,
    pub total_bytes: u64,
    pub segments: Vec<TaikaiPullRequest>,
    pub hedged: bool,
}

impl TaikaiPullBatch {
    fn new(
        id: TaikaiPullBatchId,
        shard: Option<TaikaiShardId>,
        qos: QosClass,
        segments: Vec<TaikaiPullRequest>,
        hedged: bool,
    ) -> Self {
        let total_bytes = segments.iter().map(|req| req.size_bytes).sum();
        Self {
            id,
            shard,
            qos,
            total_bytes,
            segments,
            hedged,
        }
    }
}

/// Ticket returned to callers so they can mark pull completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaikaiPullTicket {
    id: TaikaiPullBatchId,
}

impl From<TaikaiPullBatchId> for TaikaiPullTicket {
    fn from(id: TaikaiPullBatchId) -> Self {
        Self { id }
    }
}

impl TaikaiPullTicket {
    #[must_use]
    pub fn id(self) -> TaikaiPullBatchId {
        self.id
    }
}

/// Queue statistics surfaced to operators and tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaikaiPullQueueStats {
    pub pending_segments: u64,
    pub pending_bytes: u64,
    pub pending_batches: u64,
    pub in_flight_batches: u64,
    pub hedged_batches: u64,
    pub shaper_denials: QosStats,
    pub dropped_segments: u64,
    pub failovers: u64,
    pub open_circuits: u64,
}

/// Configuration tuning batching/back-pressure behaviour of the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaikaiPullQueueConfig {
    pub max_batch_segments: usize,
    pub max_batch_bytes: u64,
    pub max_in_flight_batches: usize,
    pub hedge_after: Duration,
    pub max_backlog_segments: usize,
}

impl TaikaiPullQueueConfig {
    #[must_use]
    pub fn tuned_for_cache(cache: &TaikaiCacheConfig) -> Self {
        let hot_quarter = cache.hot_capacity_bytes.saturating_div(4).max(1);
        Self {
            max_batch_segments: 8,
            max_batch_bytes: hot_quarter.min(16 * 1024 * 1024),
            max_in_flight_batches: 4,
            hedge_after: Duration::from_millis(125),
            max_backlog_segments: 256,
        }
    }
}

impl Default for TaikaiPullQueueConfig {
    fn default() -> Self {
        Self {
            max_batch_segments: 8,
            max_batch_bytes: 2 * 1024 * 1024,
            max_in_flight_batches: 4,
            hedge_after: Duration::from_millis(125),
            max_backlog_segments: 256,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingPull {
    shard: Option<TaikaiShardId>,
    request: TaikaiPullRequest,
}

#[derive(Debug, Clone, Copy)]
struct ShardSelection {
    preferred: Option<TaikaiShardId>,
    selected: Option<TaikaiShardId>,
    failover: bool,
}

#[derive(Debug)]
struct InFlightBatch {
    batch: TaikaiPullBatch,
    issued_at: Instant,
}

/// Error raised when the queue cannot accept more work.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TaikaiQueueError {
    #[error("taikai pull queue backlog exceeded limit ({limit} segments)")]
    Backpressure { limit: usize },
    #[error("taikai pull queue is unavailable")]
    Unavailable,
}

/// Pull queue that coalesces cache requests before dispatch.
pub struct TaikaiPullQueue {
    config: TaikaiPullQueueConfig,
    shard_ring: TaikaiShardRing,
    reliability: ShardReliability,
    shaper: QosEnforcer,
    pending: VecDeque<PendingPull>,
    in_flight: HashMap<TaikaiPullBatchId, InFlightBatch>,
    next_batch_id: u64,
    stats: TaikaiPullQueueStats,
}

impl TaikaiPullQueue {
    #[must_use]
    pub(crate) fn new(
        config: TaikaiPullQueueConfig,
        qos: QosConfig,
        shard_ring: TaikaiShardRing,
        reliability: ReliabilityConfig,
    ) -> Self {
        let now = Instant::now();
        Self {
            config,
            shard_ring,
            reliability: ShardReliability::new(reliability),
            shaper: QosEnforcer::new(&qos, now),
            pending: VecDeque::new(),
            in_flight: HashMap::new(),
            next_batch_id: 0,
            stats: TaikaiPullQueueStats::default(),
        }
    }

    pub fn set_shard_ring(&mut self, ring: TaikaiShardRing) {
        self.shard_ring = ring;
    }

    fn record_shaper_denial(&mut self, class: QosClass) {
        record_taikai_qos_denial(class);
        self.stats.shaper_denials.increment(class);
    }

    fn requeue_front(&mut self, drained: Vec<PendingPull>) {
        for pending in drained.into_iter().rev() {
            self.pending.push_front(pending);
        }
    }

    pub fn enqueue(&mut self, request: TaikaiPullRequest) -> Result<(), TaikaiQueueError> {
        self.enqueue_at(request, Instant::now())
    }

    pub fn enqueue_at(
        &mut self,
        request: TaikaiPullRequest,
        now: Instant,
    ) -> Result<(), TaikaiQueueError> {
        if self.pending.len() >= self.config.max_backlog_segments {
            self.stats.dropped_segments += 1;
            record_taikai_queue_event("backpressure_drop", Some(request.qos));
            return Err(TaikaiQueueError::Backpressure {
                limit: self.config.max_backlog_segments,
            });
        }
        let selection = self.reliability.select(&self.shard_ring, &request.key, now);
        if selection.failover {
            record_taikai_shard_failover(selection.preferred, selection.selected);
        }
        let shard = selection.selected;
        self.stats.pending_segments += 1;
        self.stats.pending_bytes += request.size_bytes;
        self.pending.push_back(PendingPull { shard, request });
        Ok(())
    }

    pub fn issue_ready_batch(&mut self, now: Instant) -> Option<TaikaiPullBatch> {
        if self.pending.is_empty() || self.in_flight.len() >= self.config.max_in_flight_batches {
            return None;
        }
        let mut segments = Vec::new();
        let mut drained = Vec::new();
        let mut shard: Option<TaikaiShardId> = None;
        let mut qos = QosClass::Bulk;
        let mut total_bytes = 0_u64;
        while let Some(pending) = self.pending.front() {
            if segments.len() >= self.config.max_batch_segments
                || total_bytes >= self.config.max_batch_bytes
            {
                break;
            }
            let matches_shard = match (shard, pending.shard) {
                (None, hint) => {
                    shard = hint;
                    true
                }
                (Some(existing), Some(candidate)) => existing == candidate,
                (Some(_), None) => false,
            };
            if !matches_shard {
                break;
            }
            let pending = self.pending.pop_front().expect("front");
            qos = merge_qos(qos, pending.request.qos);
            total_bytes = total_bytes.saturating_add(pending.request.size_bytes);
            segments.push(pending.request.clone());
            drained.push(pending);
        }

        if segments.is_empty() {
            return None;
        }

        if let Err(QosError::RateLimited { class, .. }) =
            self.shaper.try_acquire(qos, total_bytes, now)
        {
            self.record_shaper_denial(class);
            self.requeue_front(drained);
            return None;
        }

        self.stats.pending_segments = self
            .stats
            .pending_segments
            .saturating_sub(segments.len() as u64);
        self.stats.pending_bytes = self.stats.pending_bytes.saturating_sub(total_bytes);
        let id = TaikaiPullBatchId::next(&mut self.next_batch_id);
        let batch = TaikaiPullBatch::new(id, shard, qos, segments, false);
        self.in_flight.insert(
            id,
            InFlightBatch {
                batch: batch.clone(),
                issued_at: now,
            },
        );
        self.stats.in_flight_batches += 1;
        record_taikai_queue_event("issued", Some(batch.qos));
        Some(batch)
    }

    pub fn issue_specific(&mut self, key: &SegmentKey, now: Instant) -> Option<TaikaiPullBatch> {
        if self.pending.is_empty() || self.in_flight.len() >= self.config.max_in_flight_batches {
            return None;
        }
        let index = self
            .pending
            .iter()
            .position(|entry| entry.request.key == *key)?;
        let PendingPull { shard, request } = self.pending.remove(index)?;
        let class = request.qos;
        let size_bytes = request.size_bytes;
        if let Err(QosError::RateLimited { class: denied, .. }) =
            self.shaper.try_acquire(class, size_bytes, now)
        {
            self.record_shaper_denial(denied);
            self.pending.insert(index, PendingPull { shard, request });
            return None;
        }
        self.stats.pending_segments = self.stats.pending_segments.saturating_sub(1);
        self.stats.pending_bytes = self.stats.pending_bytes.saturating_sub(request.size_bytes);
        let id = TaikaiPullBatchId::next(&mut self.next_batch_id);
        let batch = TaikaiPullBatch::new(id, shard, request.qos, vec![request], false);
        self.in_flight.insert(
            id,
            InFlightBatch {
                batch: batch.clone(),
                issued_at: now,
            },
        );
        self.stats.in_flight_batches += 1;
        record_taikai_queue_event("issued", Some(batch.qos));
        Some(batch)
    }

    pub fn cancel_pending(&mut self, key: &SegmentKey) -> bool {
        if self.pending.is_empty() {
            return false;
        }
        if let Some(index) = self
            .pending
            .iter()
            .position(|entry| entry.request.key == *key)
        {
            let Some(entry) = self.pending.remove(index) else {
                return false;
            };
            self.stats.pending_segments = self.stats.pending_segments.saturating_sub(1);
            self.stats.pending_bytes = self
                .stats
                .pending_bytes
                .saturating_sub(entry.request.size_bytes);
            return true;
        }
        false
    }

    pub fn hedge_overdue_batches(&mut self, now: Instant) -> Vec<TaikaiPullBatch> {
        let mut hedged = Vec::new();
        let mut denied = Vec::new();
        for state in self.in_flight.values_mut() {
            if now
                .checked_duration_since(state.issued_at)
                .is_some_and(|elapsed| elapsed >= self.config.hedge_after)
            {
                let qos = state.batch.qos;
                if let Err(QosError::RateLimited { class, .. }) =
                    self.shaper.try_acquire(qos, state.batch.total_bytes, now)
                {
                    denied.push(class);
                    continue;
                }
                let mut clone = state.batch.clone();
                clone.hedged = true;
                hedged.push(clone);
                state.issued_at = now;
                self.stats.hedged_batches += 1;
                record_taikai_queue_event("hedged", Some(qos));
            }
        }
        for class in denied {
            self.record_shaper_denial(class);
        }
        hedged
    }

    pub fn complete(&mut self, ticket: TaikaiPullTicket) -> bool {
        self.complete_at(ticket, Instant::now())
    }

    pub fn complete_at(&mut self, ticket: TaikaiPullTicket, now: Instant) -> bool {
        if let Some(state) = self.in_flight.remove(&ticket.id) {
            self.stats.in_flight_batches = self.stats.in_flight_batches.saturating_sub(1);
            if let Some(shard) = state.batch.shard {
                let _ = self.reliability.record_success(shard, now);
                let is_open = self.reliability.shard_open(shard, now);
                set_taikai_shard_open(shard, is_open);
            }
            return true;
        }
        false
    }

    pub fn fail_at(&mut self, ticket: TaikaiPullTicket, now: Instant) -> bool {
        if let Some(state) = self.in_flight.remove(&ticket.id) {
            self.stats.in_flight_batches = self.stats.in_flight_batches.saturating_sub(1);
            if let Some(shard) = state.batch.shard {
                let _ = self.reliability.record_failure(shard, now);
                let is_open = self.reliability.shard_open(shard, now);
                set_taikai_shard_open(shard, is_open);
            }
            return true;
        }
        false
    }

    #[must_use]
    pub fn stats(&mut self) -> TaikaiPullQueueStats {
        let pending_batches = if self.pending.is_empty() {
            0
        } else {
            self.pending.len().div_ceil(self.config.max_batch_segments) as u64
        };
        let reliability = self.reliability.stats(Instant::now());
        self.stats.open_circuits = reliability.open_circuits;
        self.stats.failovers = reliability.failovers;
        let snapshot = TaikaiPullQueueStats {
            pending_segments: self.stats.pending_segments,
            pending_bytes: self.stats.pending_bytes,
            pending_batches,
            in_flight_batches: self.stats.in_flight_batches,
            hedged_batches: self.stats.hedged_batches,
            shaper_denials: self.stats.shaper_denials,
            dropped_segments: self.stats.dropped_segments,
            failovers: self.stats.failovers,
            open_circuits: self.stats.open_circuits,
        };
        record_taikai_queue_depth("pending_segments", snapshot.pending_segments);
        record_taikai_queue_depth("pending_bytes", snapshot.pending_bytes);
        record_taikai_queue_depth("pending_batches", snapshot.pending_batches);
        record_taikai_queue_depth("in_flight_batches", snapshot.in_flight_batches);
        record_taikai_queue_depth("open_circuits", snapshot.open_circuits);
        snapshot
    }
}

fn merge_qos(existing: QosClass, next: QosClass) -> QosClass {
    match (existing, next) {
        (QosClass::Priority, _) | (_, QosClass::Priority) => QosClass::Priority,
        (QosClass::Standard, _) | (_, QosClass::Standard) => QosClass::Standard,
        _ => QosClass::Bulk,
    }
}

/// Shared Taikai cache handle exposing both cache and queue to orchestrator users.
#[derive(Clone)]
pub struct TaikaiCacheHandle {
    cache: Arc<Mutex<TaikaiCache>>,
    queue: Arc<Mutex<TaikaiPullQueue>>,
    queue_config: TaikaiPullQueueConfig,
}

impl TaikaiCacheHandle {
    #[must_use]
    pub fn from_config(config: TaikaiCacheConfig) -> Self {
        let queue_config = TaikaiPullQueueConfig::tuned_for_cache(&config);
        Self::with_queue_config(config, queue_config)
    }

    #[must_use]
    pub fn with_queue_config(
        config: TaikaiCacheConfig,
        queue_config: TaikaiPullQueueConfig,
    ) -> Self {
        let reliability = config.reliability_config();
        let queue = TaikaiPullQueue::new(
            queue_config,
            config.qos.clone(),
            TaikaiShardRing::new(Vec::new()),
            reliability,
        );
        Self {
            cache: Arc::new(Mutex::new(TaikaiCache::new(config))),
            queue: Arc::new(Mutex::new(queue)),
            queue_config,
        }
    }

    #[must_use]
    pub fn cache(&self) -> Arc<Mutex<TaikaiCache>> {
        Arc::clone(&self.cache)
    }

    #[must_use]
    pub fn queue(&self) -> Arc<Mutex<TaikaiPullQueue>> {
        Arc::clone(&self.queue)
    }

    fn lock_queue(&self) -> Result<MutexGuard<'_, TaikaiPullQueue>, TaikaiQueueError> {
        self.queue.lock().map_err(|_| TaikaiQueueError::Unavailable)
    }

    #[must_use]
    pub fn queue_stats(&self) -> TaikaiPullQueueStats {
        self.queue
            .lock()
            .map(|mut queue| queue.stats())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn pull_queue_config(&self) -> TaikaiPullQueueConfig {
        self.queue_config
    }

    pub fn configure_shards(&self, shards: Vec<TaikaiShardId>) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.set_shard_ring(TaikaiShardRing::new(shards));
        }
    }

    /// Enqueue a pull request, returning a back-pressure signal when the queue
    /// reaches its configured backlog.
    pub fn enqueue_pull(&self, request: TaikaiPullRequest) -> Result<(), TaikaiQueueError> {
        let mut queue = self.lock_queue()?;
        queue.enqueue(request)
    }

    /// Issue a batch that contains the specific segment key if capacity allows.
    pub fn issue_specific_at(
        &self,
        key: &SegmentKey,
        now: Instant,
    ) -> Result<Option<TaikaiPullBatch>, TaikaiQueueError> {
        let mut queue = self.lock_queue()?;
        Ok(queue.issue_specific(key, now))
    }

    /// Remove a pending pull request from the queue without issuing it.
    #[must_use]
    pub fn cancel_pending(&self, key: &SegmentKey) -> bool {
        self.queue
            .lock()
            .map(|mut queue| queue.cancel_pending(key))
            .unwrap_or(false)
    }

    /// Issue the next ready pull batch using the provided timestamp.
    pub fn issue_ready_batch_at(
        &self,
        now: Instant,
    ) -> Result<Option<TaikaiPullBatch>, TaikaiQueueError> {
        let mut queue = self.lock_queue()?;
        Ok(queue.issue_ready_batch(now))
    }

    /// Convenience helper that issues a batch using `Instant::now()`.
    pub fn issue_ready_batch(&self) -> Result<Option<TaikaiPullBatch>, TaikaiQueueError> {
        self.issue_ready_batch_at(Instant::now())
    }

    /// Re-issues overdue batches (hedging) using the supplied timestamp.
    pub fn hedge_overdue_batches_at(
        &self,
        now: Instant,
    ) -> Result<Vec<TaikaiPullBatch>, TaikaiQueueError> {
        let mut queue = self.lock_queue()?;
        Ok(queue.hedge_overdue_batches(now))
    }

    /// Convenience helper that hedges overdue batches using `Instant::now()`.
    pub fn hedge_overdue_batches(&self) -> Result<Vec<TaikaiPullBatch>, TaikaiQueueError> {
        self.hedge_overdue_batches_at(Instant::now())
    }

    /// Marks a previously issued batch as complete, freeing an in-flight slot.
    pub fn complete_batch(&self, ticket: TaikaiPullTicket) -> Result<bool, TaikaiQueueError> {
        let mut queue = self.lock_queue()?;
        Ok(queue.complete_at(ticket, Instant::now()))
    }

    /// Marks a previously issued batch as failed, updating reliability gates.
    pub fn fail_batch(&self, ticket: TaikaiPullTicket) -> Result<bool, TaikaiQueueError> {
        let mut queue = self.lock_queue()?;
        Ok(queue.fail_at(ticket, Instant::now()))
    }
}

fn record_taikai_cache_insert_metrics(
    segment: &Arc<CachedSegment>,
    outcome: &TaikaiCacheInsertOutcome,
) {
    if outcome.inserted_into.is_empty() && outcome.evicted.is_empty() {
        return;
    }
    let metrics = global_or_default();
    let bytes = segment.size_bytes();
    for tier in &outcome.inserted_into {
        metrics.record_taikai_cache_insert(tier.label(), bytes);
    }
    for eviction in &outcome.evicted {
        metrics.record_taikai_cache_eviction(eviction.from.label(), eviction.reason.label());
    }
}

fn record_taikai_cache_query_metrics(outcome: &TaikaiCacheQueryOutcome) {
    let metrics = global_or_default();
    match (&outcome.segment, outcome.hit_tier) {
        (Some(segment), Some(tier)) => {
            let label = tier.label();
            metrics.record_taikai_cache_query("hit", label);
            metrics.record_taikai_cache_bytes("hit", label, segment.size_bytes());
        }
        _ => metrics.record_taikai_cache_query("miss", "none"),
    }
    for promotion in &outcome.promotions {
        metrics.record_taikai_cache_promotion(promotion.from.label(), promotion.to.label());
    }
}

fn record_taikai_cache_evictions(evicted: &[CacheEviction]) {
    if evicted.is_empty() {
        return;
    }
    let metrics = global_or_default();
    for entry in evicted {
        metrics.record_taikai_cache_eviction(entry.from.label(), entry.reason.label());
    }
}

fn record_taikai_qos_denial(class: QosClass) {
    let metrics = global_or_default();
    metrics.inc_taikai_qos_denied(class.label());
}

fn record_taikai_queue_event(event: &str, class: Option<QosClass>) {
    let metrics = global_or_default();
    let class_label = class.map(QosClass::label).unwrap_or("none");
    metrics.inc_taikai_queue_event(event, class_label);
}

fn record_taikai_queue_depth(state: &str, value: u64) {
    let metrics = global_or_default();
    let clamped = i64::try_from(value).unwrap_or(i64::MAX);
    metrics.set_taikai_queue_depth(state, clamped);
}

fn shard_label(shard: TaikaiShardId) -> String {
    shard.0.to_string()
}

fn record_taikai_shard_failover(preferred: Option<TaikaiShardId>, selected: Option<TaikaiShardId>) {
    let metrics = global_or_default();
    let preferred = preferred
        .map(shard_label)
        .unwrap_or_else(|| "none".to_owned());
    let selected = selected
        .map(shard_label)
        .unwrap_or_else(|| "none".to_owned());
    metrics.inc_taikai_shard_failover(&preferred, &selected);
}

fn set_taikai_shard_open(shard: TaikaiShardId, open: bool) {
    let metrics = global_or_default();
    metrics.set_taikai_shard_circuit_open(&shard_label(shard), open);
}

/// Cache admission action recorded in gossip announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum CacheAdmissionAction {
    /// Cache admitted the referenced segment.
    Admit,
    /// Cache evicted the referenced segment.
    Evict,
}

/// Version used by cache admission records.
pub const CACHE_ADMISSION_VERSION_V1: u16 = 1;

/// Signed cache admission record shared across the gossip plane.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct CacheAdmissionRecord {
    pub version: u16,
    pub shard_id: TaikaiShardId,
    pub issuer: GuardDirectoryId,
    pub action: CacheAdmissionAction,
    pub tier: CacheTierKind,
    pub qos: QosClass,
    pub segment: SegmentKey,
    pub payload_len: u64,
    pub payload_digest: [u8; 32],
    pub issued_unix_ms: u64,
    pub expires_unix_ms: u64,
}

impl CacheAdmissionRecord {
    /// Build a cache admission record from a cached segment.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when the TTL overflows or timestamp addition wraps.
    pub fn from_segment(
        shard_id: TaikaiShardId,
        issuer: GuardDirectoryId,
        segment: &CachedSegment,
        tier: CacheTierKind,
        action: CacheAdmissionAction,
        issued_unix_ms: u64,
        ttl: Duration,
    ) -> Result<Self, CacheAdmissionError> {
        let ttl_ms = ttl
            .as_millis()
            .try_into()
            .map_err(|_| CacheAdmissionError::TtlOverflow)?;
        let expires_unix_ms = issued_unix_ms
            .checked_add(ttl_ms)
            .ok_or(CacheAdmissionError::TimestampOverflow)?;
        let payload = segment.payload();
        let digest = blake3_hash(payload.as_ref());
        Ok(Self {
            version: CACHE_ADMISSION_VERSION_V1,
            shard_id,
            issuer,
            action,
            tier,
            qos: segment.qos(),
            segment: segment.key(),
            payload_len: segment.size_bytes(),
            payload_digest: digest.into(),
            issued_unix_ms,
            expires_unix_ms,
        })
    }

    #[must_use]
    pub fn expires_unix_ms(&self) -> u64 {
        self.expires_unix_ms
    }

    #[must_use]
    pub fn issued_unix_ms(&self) -> u64 {
        self.issued_unix_ms
    }

    #[must_use]
    pub fn segment(&self) -> &SegmentKey {
        &self.segment
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, CacheAdmissionError> {
        to_bytes(self).map_err(CacheAdmissionError::Serialization)
    }
}

/// Signed cache admission envelope.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct CacheAdmissionEnvelope {
    body: CacheAdmissionRecord,
    signer: PublicKey,
    signature: Signature,
}

impl CacheAdmissionEnvelope {
    /// Sign a cache admission record with the provided key pair.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError::Serialization`] when canonicalisation fails.
    pub fn sign(
        body: CacheAdmissionRecord,
        key_pair: &KeyPair,
    ) -> Result<Self, CacheAdmissionError> {
        let canonical = body.canonical_bytes()?;
        let signature = Signature::new(key_pair.private_key(), &canonical);
        Ok(Self {
            body,
            signer: key_pair.public_key().clone(),
            signature,
        })
    }

    /// Verify the signature and expiry of the envelope.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when the signature fails or the envelope expired.
    pub fn verify(&self, now_unix_ms: u64) -> Result<(), CacheAdmissionError> {
        if now_unix_ms > self.body.expires_unix_ms {
            return Err(CacheAdmissionError::Expired {
                now_unix_ms,
                expires_unix_ms: self.body.expires_unix_ms,
            });
        }
        let canonical = self.body.canonical_bytes()?;
        self.signature
            .verify(self.signer(), &canonical)
            .map_err(|_| CacheAdmissionError::InvalidSignature)
    }

    #[must_use]
    pub fn body(&self) -> &CacheAdmissionRecord {
        &self.body
    }

    #[must_use]
    pub fn into_parts(self) -> (CacheAdmissionRecord, PublicKey, Signature) {
        (self.body, self.signer, self.signature)
    }

    #[must_use]
    pub fn from_parts(body: CacheAdmissionRecord, signer: PublicKey, signature: Signature) -> Self {
        Self {
            body,
            signer,
            signature,
        }
    }

    #[must_use]
    pub fn signer(&self) -> &PublicKey {
        &self.signer
    }

    #[must_use]
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

/// Version used by cache admission gossip envelopes.
pub const CACHE_ADMISSION_GOSSIP_VERSION_V1: u16 = 1;

/// Signed cache admission gossip payload shared across CAA peers.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct CacheAdmissionGossipBody {
    pub version: u16,
    pub envelope: CacheAdmissionEnvelope,
    pub nonce: [u8; 16],
    pub issued_unix_ms: u64,
    pub expires_unix_ms: u64,
}

impl CacheAdmissionGossipBody {
    /// Create a gossip body using the provided TTL and a random nonce.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when TTL is zero or the expiry overflows.
    pub fn new(
        envelope: CacheAdmissionEnvelope,
        issued_unix_ms: u64,
        ttl: Duration,
    ) -> Result<Self, CacheAdmissionError> {
        let mut rng = rand::rng();
        Self::with_nonce(envelope, issued_unix_ms, ttl, &mut rng)
    }

    /// Create a gossip body using an explicit nonce generator (deterministic in tests).
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when TTL is zero or the expiry overflows.
    pub fn with_nonce(
        envelope: CacheAdmissionEnvelope,
        issued_unix_ms: u64,
        ttl: Duration,
        rng: &mut impl RngCore,
    ) -> Result<Self, CacheAdmissionError> {
        if ttl.is_zero() {
            return Err(CacheAdmissionError::InvalidTtl);
        }
        let envelope_expiry = envelope.body().expires_unix_ms();
        let ttl_ms: u64 = ttl
            .as_millis()
            .try_into()
            .map_err(|_| CacheAdmissionError::TtlOverflow)?;
        let mut nonce = [0u8; 16];
        rng.fill_bytes(&mut nonce);
        let expires_unix_ms = issued_unix_ms
            .checked_add(ttl_ms)
            .ok_or(CacheAdmissionError::TimestampOverflow)?
            .min(envelope_expiry);
        Ok(Self {
            version: CACHE_ADMISSION_GOSSIP_VERSION_V1,
            envelope,
            nonce,
            issued_unix_ms,
            expires_unix_ms,
        })
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, CacheAdmissionError> {
        to_bytes(self).map_err(CacheAdmissionError::Serialization)
    }

    #[must_use]
    pub fn expires_unix_ms(&self) -> u64 {
        self.expires_unix_ms
    }

    #[must_use]
    pub fn issued_unix_ms(&self) -> u64 {
        self.issued_unix_ms
    }

    #[must_use]
    pub fn envelope(&self) -> &CacheAdmissionEnvelope {
        &self.envelope
    }
}

/// Gossip envelope carrying a signed admission/eviction announcement.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct CacheAdmissionGossip {
    body: CacheAdmissionGossipBody,
    signer: PublicKey,
    signature: Signature,
}

impl CacheAdmissionGossip {
    /// Sign a gossip body with the provided governance key pair.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when canonicalisation fails.
    pub fn sign(
        body: CacheAdmissionGossipBody,
        key_pair: &KeyPair,
    ) -> Result<Self, CacheAdmissionError> {
        let canonical = body.canonical_bytes()?;
        let signature = Signature::new(key_pair.private_key(), &canonical);
        Ok(Self {
            body,
            signer: key_pair.public_key().clone(),
            signature,
        })
    }

    /// Verify the gossip signature, expiry, and embedded admission envelope.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when the signature fails or the gossip body expired.
    pub fn verify(&self, now_unix_ms: u64) -> Result<(), CacheAdmissionError> {
        if now_unix_ms > self.body.expires_unix_ms {
            return Err(CacheAdmissionError::Expired {
                now_unix_ms,
                expires_unix_ms: self.body.expires_unix_ms,
            });
        }
        self.body.envelope.verify(now_unix_ms)?;
        let canonical = self.body.canonical_bytes()?;
        self.signature
            .verify(self.signer(), &canonical)
            .map_err(|_| CacheAdmissionError::InvalidSignature)
    }

    /// Compute a deterministic digest for replay tracking.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError::Serialization`] when canonicalisation fails.
    pub fn digest(&self) -> Result<[u8; 32], CacheAdmissionError> {
        let canonical = self.body.canonical_bytes()?;
        Ok(blake3_hash(&canonical).into())
    }

    #[must_use]
    pub fn body(&self) -> &CacheAdmissionGossipBody {
        &self.body
    }

    #[must_use]
    pub fn signer(&self) -> &PublicKey {
        &self.signer
    }

    #[must_use]
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    #[must_use]
    pub fn into_parts(self) -> (CacheAdmissionGossipBody, PublicKey, Signature) {
        (self.body, self.signer, self.signature)
    }

    #[must_use]
    pub fn from_parts(
        body: CacheAdmissionGossipBody,
        signer: PublicKey,
        signature: Signature,
    ) -> Self {
        Self {
            body,
            signer,
            signature,
        }
    }
}

/// Deterministic replay filter for cache admission gossip entries.
#[derive(Debug)]
pub struct CacheAdmissionReplayFilter {
    ttl_ms: u64,
    capacity: usize,
    window: VecDeque<(u64, [u8; 32])>,
    index: HashMap<[u8; 32], u64>,
}

impl CacheAdmissionReplayFilter {
    /// Create a replay filter with the provided TTL and capacity.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when TTL or capacity are zero, or TTL overflows.
    pub fn new(ttl: Duration, capacity: usize) -> Result<Self, CacheAdmissionError> {
        if ttl.is_zero() {
            return Err(CacheAdmissionError::InvalidReplayWindow);
        }
        if capacity == 0 {
            return Err(CacheAdmissionError::InvalidReplayCapacity);
        }
        let ttl_ms: u64 = ttl
            .as_millis()
            .try_into()
            .map_err(|_| CacheAdmissionError::TtlOverflow)?;
        Ok(Self {
            ttl_ms,
            capacity,
            window: VecDeque::with_capacity(capacity),
            index: HashMap::with_capacity(capacity),
        })
    }

    /// Observe a gossip entry, returning `true` when it is accepted and `false` on replay.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when canonicalisation fails or timestamps overflow.
    pub fn observe(
        &mut self,
        gossip: &CacheAdmissionGossip,
        now_unix_ms: u64,
    ) -> Result<bool, CacheAdmissionError> {
        self.evict_expired(now_unix_ms);
        let digest = gossip.digest()?;
        if let Some(expiry) = self.index.get(&digest)
            && *expiry > now_unix_ms
        {
            return Ok(false);
        }
        let expires_at = now_unix_ms
            .checked_add(self.ttl_ms)
            .ok_or(CacheAdmissionError::TimestampOverflow)?;
        self.window.push_back((expires_at, digest));
        self.index.insert(digest, expires_at);
        if self.window.len() > self.capacity
            && let Some((_, evicted)) = self.window.pop_front()
        {
            self.index.remove(&evicted);
        }
        Ok(true)
    }

    fn evict_expired(&mut self, now_unix_ms: u64) {
        while let Some((expires, digest)) = self.window.front().copied() {
            if expires > now_unix_ms {
                break;
            }
            self.window.pop_front();
            self.index.remove(&digest);
        }
    }
}

/// Errors produced by cache admission gossip helpers.
#[derive(Debug, Error)]
pub enum CacheAdmissionError {
    #[error("failed to serialize cache admission record: {0}")]
    Serialization(NoritoError),
    #[error("cache admission TTL must be greater than zero")]
    InvalidTtl,
    #[error("cache admission replay window must be greater than zero")]
    InvalidReplayWindow,
    #[error("cache admission replay filter capacity must be greater than zero")]
    InvalidReplayCapacity,
    #[error("cache admission TTL exceeds representable range")]
    TtlOverflow,
    #[error("cache admission timestamp overflowed")]
    TimestampOverflow,
    #[error("cache admission signature verification failed")]
    InvalidSignature,
    #[error("cache admission envelope expired at {expires_unix_ms}, now={now_unix_ms}")]
    Expired {
        now_unix_ms: u64,
        expires_unix_ms: u64,
    },
}

/// Default replay TTL for cache admission gossip entries.
pub const CACHE_ADMISSION_REPLAY_DEFAULT_TTL: Duration = Duration::from_secs(30);
/// Default replay filter capacity for cache admission gossip entries.
pub const CACHE_ADMISSION_REPLAY_DEFAULT_CAPACITY: usize = 1_024;

/// Tracks cache admission gossip and keeps the pull queue's shard ring in sync.
pub struct CacheAdmissionTracker {
    handle: TaikaiCacheHandle,
    replay: CacheAdmissionReplayFilter,
    active_shards: HashMap<TaikaiShardId, u64>,
}

impl CacheAdmissionTracker {
    /// Create a tracker with the provided replay filter.
    #[must_use]
    pub fn new(handle: TaikaiCacheHandle, replay: CacheAdmissionReplayFilter) -> Self {
        Self {
            handle,
            replay,
            active_shards: HashMap::new(),
        }
    }

    /// Create a tracker using default replay TTL and capacity.
    #[must_use]
    pub fn with_defaults(handle: TaikaiCacheHandle) -> Self {
        let replay = CacheAdmissionReplayFilter::new(
            CACHE_ADMISSION_REPLAY_DEFAULT_TTL,
            CACHE_ADMISSION_REPLAY_DEFAULT_CAPACITY,
        )
        .expect("default replay filter parameters are valid");
        Self::new(handle, replay)
    }

    /// Ingest a verified gossip entry, updating the shard ring when accepted.
    ///
    /// Returns `Ok(true)` when the gossip is accepted, `Ok(false)` on replay.
    ///
    /// # Errors
    ///
    /// Returns [`CacheAdmissionError`] when verification or replay bookkeeping fails.
    pub fn ingest(
        &mut self,
        gossip: &CacheAdmissionGossip,
        now_unix_ms: u64,
    ) -> Result<bool, CacheAdmissionError> {
        gossip.verify(now_unix_ms)?;
        if !self.replay.observe(gossip, now_unix_ms)? {
            let _ = self.evict_expired(now_unix_ms);
            return Ok(false);
        }
        let body = gossip.body();
        let record = body.envelope().body();
        self.active_shards
            .insert(record.shard_id, body.expires_unix_ms());
        let _ = self.evict_expired(now_unix_ms);
        self.refresh_ring();
        Ok(true)
    }

    /// Evict expired shard entries and refresh the shard ring when it shrinks.
    #[must_use]
    pub fn evict_expired(&mut self, now_unix_ms: u64) -> bool {
        let before = self.active_shards.len();
        self.active_shards
            .retain(|_, expires| *expires > now_unix_ms);
        let changed = self.active_shards.len() != before;
        if changed {
            self.refresh_ring();
        }
        changed
    }

    /// Return the currently active shard identifiers.
    #[must_use]
    pub fn active_shards(&self) -> Vec<TaikaiShardId> {
        let mut shards: Vec<_> = self.active_shards.keys().copied().collect();
        shards.sort_unstable();
        shards
    }

    fn refresh_ring(&self) {
        self.handle.configure_shards(self.active_shards());
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_data_model::{
        da::types::{BlobDigest, StorageTicketId},
        name::Name,
        taikai::{
            SegmentDuration, SegmentTimestamp, TaikaiCarPointer, TaikaiIngestPointer,
            TaikaiInstrumentation, TaikaiTrackMetadata,
        },
    };
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    fn digest(value: u8) -> BlobDigest {
        let mut bytes = [0u8; 32];
        bytes.fill(value);
        BlobDigest::new(bytes)
    }

    fn storage_ticket(value: u8) -> StorageTicketId {
        StorageTicketId::new([value; 32])
    }

    fn sample_envelope(sequence: u64) -> TaikaiSegmentEnvelopeV1 {
        let event_id = TaikaiEventId::new(Name::from_str("global-keynote").expect("valid name"));
        let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").expect("valid name"));
        let rendition_id = TaikaiRenditionId::new(Name::from_str("1080p").expect("valid name"));
        let track = TaikaiTrackMetadata::video(
            iroha_data_model::taikai::TaikaiCodec::HevcMain10,
            6_000,
            iroha_data_model::taikai::TaikaiResolution::new(1920, 1080),
        );
        let car = TaikaiCarPointer::new("bagcq", digest(0xAA), 65_536);
        let ingest =
            TaikaiIngestPointer::new(digest(0xBB), storage_ticket(0xCC), digest(0xDD), 24, car);
        TaikaiSegmentEnvelopeV1 {
            metadata: iroha_data_model::da::types::ExtraMetadata::default(),
            instrumentation: TaikaiInstrumentation::default(),
            event_id,
            stream_id,
            rendition_id,
            track,
            segment_sequence: sequence,
            segment_start_pts: SegmentTimestamp::new(90_000 * sequence),
            segment_duration: SegmentDuration::new(2_000_000),
            wallclock_unix_ms: 1_726_000_000_000,
            ingest,
            version: TaikaiSegmentEnvelopeV1::VERSION,
        }
    }

    fn dummy_segment(sequence: u64, len: usize, qos: QosClass) -> CachedSegment {
        let envelope = sample_envelope(sequence);
        let payload = vec![0u8; len];
        CachedSegment::new(envelope, Arc::<[u8]>::from(payload), qos)
    }

    fn cache_admission_gossip(
        shard: TaikaiShardId,
        sequence: u64,
        issued_ms: u64,
        ttl: Duration,
    ) -> CacheAdmissionGossip {
        let issuer = GuardDirectoryId::new("soranet/cache");
        let cached = dummy_segment(sequence, 512, QosClass::Priority);
        let key_pair = KeyPair::from_seed(vec![0xAB; 32], iroha_crypto::Algorithm::Ed25519);
        let record = CacheAdmissionRecord::from_segment(
            shard,
            issuer,
            &cached,
            CacheTierKind::Hot,
            CacheAdmissionAction::Admit,
            issued_ms,
            ttl,
        )
        .expect("record");
        let envelope = CacheAdmissionEnvelope::sign(record, &key_pair).expect("envelope");
        let mut rng = StdRng::seed_from_u64(sequence);
        let body =
            CacheAdmissionGossipBody::with_nonce(envelope, issued_ms, ttl, &mut rng).unwrap();
        CacheAdmissionGossip::sign(body, &key_pair).expect("gossip")
    }

    #[test]
    fn hot_eviction_demotes_to_warm() {
        let mut cache = TaikaiCache::new(TaikaiCacheConfig {
            hot_capacity_bytes: 512,
            hot_retention: Duration::from_secs(10),
            warm_capacity_bytes: 2_048,
            warm_retention: Duration::from_mins(10),
            cold_capacity_bytes: 8_192,
            cold_retention: Duration::from_hours(1),
            qos: QosConfig::balanced(),
            reliability: ReliabilityTuning::default(),
        });

        let base = Instant::now();
        let seg_a = dummy_segment(1, 256, QosClass::Priority);
        let seg_b = dummy_segment(2, 256, QosClass::Priority);
        let seg_c = dummy_segment(3, 256, QosClass::Priority);

        cache.insert(seg_a.clone(), base);
        cache.insert(seg_b.clone(), base + Duration::from_millis(1));
        let outcome = cache.insert(seg_c.clone(), base + Duration::from_millis(2));
        assert!(outcome.inserted_into.contains(&CacheTierKind::Hot));
        assert!(
            outcome
                .evicted
                .iter()
                .any(|ev| ev.from == CacheTierKind::Hot
                    && ev.key.sequence() == 1
                    && ev.reason == CacheEvictionReason::Capacity)
        );

        let key_a = seg_a.key();
        let query = cache.get(&key_a, base + Duration::from_millis(3));
        assert_eq!(query.hit_tier, Some(CacheTierKind::Warm));
        assert!(
            query
                .promotions
                .iter()
                .any(|promo| promo.from == CacheTierKind::Warm && promo.to == CacheTierKind::Hot)
        );
    }

    #[test]
    fn retention_purges_entries() {
        let mut cache = TaikaiCache::new(TaikaiCacheConfig {
            hot_capacity_bytes: 1_024,
            hot_retention: Duration::from_secs(1),
            warm_capacity_bytes: 0,
            warm_retention: Duration::from_secs(0),
            cold_capacity_bytes: 0,
            cold_retention: Duration::from_secs(0),
            qos: QosConfig::balanced(),
            reliability: ReliabilityTuning::default(),
        });

        let base = Instant::now();
        let seg = dummy_segment(42, 128, QosClass::Standard);
        cache.insert(seg.clone(), base);

        let evicted = cache.purge_expired(base + Duration::from_secs(2));
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0].from, CacheTierKind::Hot);
        assert_eq!(evicted[0].key.sequence(), 42);
        assert_eq!(evicted[0].reason, CacheEvictionReason::Expired);
    }

    #[test]
    fn qos_bucket_enforces_limits() {
        let mut cache = TaikaiCache::new(TaikaiCacheConfig {
            hot_capacity_bytes: 1_024,
            hot_retention: Duration::from_mins(1),
            warm_capacity_bytes: 1_024,
            warm_retention: Duration::from_mins(1),
            cold_capacity_bytes: 1_024,
            cold_retention: Duration::from_mins(1),
            qos: QosConfig {
                priority_rate_bps: 2_048,
                standard_rate_bps: 1_024,
                bulk_rate_bps: 512,
                burst_multiplier: 1,
            },
            reliability: ReliabilityTuning::default(),
        });

        let now = Instant::now();
        cache
            .try_acquire_qos(QosClass::Priority, 512, now)
            .expect("budget available");
        let err = cache
            .try_acquire_qos(QosClass::Priority, 2_000, now)
            .expect_err("rate limit enforced");
        assert!(matches!(err, QosError::RateLimited { .. }));
        cache
            .try_acquire_qos(QosClass::Priority, 512, now + Duration::from_secs(1))
            .expect("tokens refilled");
    }

    #[test]
    fn taikai_cache_records_metrics() {
        let metrics = global_or_default();
        let hit_before = metrics
            .sorafs_taikai_cache_query_total
            .with_label_values(&["hit", "warm"])
            .get();
        let miss_before = metrics
            .sorafs_taikai_cache_query_total
            .with_label_values(&["miss", "none"])
            .get();
        let insert_hot_before = metrics
            .sorafs_taikai_cache_insert_total
            .with_label_values(&["hot"])
            .get();
        let eviction_before = metrics
            .sorafs_taikai_cache_evictions_total
            .with_label_values(&["hot", "capacity"])
            .get();
        let promotion_before = metrics
            .sorafs_taikai_cache_promotions_total
            .with_label_values(&["warm", "hot"])
            .get();
        let qos_before = metrics
            .sorafs_taikai_qos_denied_total
            .with_label_values(&["priority"])
            .get();

        let mut cache = TaikaiCache::new(TaikaiCacheConfig {
            hot_capacity_bytes: 512,
            hot_retention: Duration::from_secs(1),
            warm_capacity_bytes: 512,
            warm_retention: Duration::from_secs(1),
            cold_capacity_bytes: 1_024,
            cold_retention: Duration::from_secs(2),
            qos: QosConfig {
                priority_rate_bps: 256,
                standard_rate_bps: 256,
                bulk_rate_bps: 256,
                burst_multiplier: 1,
            },
            reliability: ReliabilityTuning::default(),
        });

        let base = Instant::now();
        let seg_a = dummy_segment(10, 256, QosClass::Priority);
        let seg_b = dummy_segment(11, 256, QosClass::Priority);
        let seg_c = dummy_segment(12, 256, QosClass::Priority);

        cache.insert(seg_a.clone(), base);
        cache.insert(seg_b.clone(), base + Duration::from_millis(1));
        cache.insert(seg_c.clone(), base + Duration::from_millis(2));

        // Warm hit + promotion
        let _ = cache.get(&seg_a.key(), base + Duration::from_millis(3));
        // Miss
        let missing = dummy_segment(99, 64, QosClass::Bulk).key();
        let _ = cache.get(&missing, base + Duration::from_millis(4));

        // Expire and purge
        let _ = cache.purge_expired(base + Duration::from_secs(3));

        // Trigger QoS denial
        let _ = cache
            .try_acquire_qos(QosClass::Priority, 10_000, base)
            .expect_err("rate limit enforced");

        assert!(
            metrics
                .sorafs_taikai_cache_query_total
                .with_label_values(&["hit", "warm"])
                .get()
                > hit_before
        );
        assert!(
            metrics
                .sorafs_taikai_cache_query_total
                .with_label_values(&["miss", "none"])
                .get()
                > miss_before
        );
        assert!(
            metrics
                .sorafs_taikai_cache_insert_total
                .with_label_values(&["hot"])
                .get()
                > insert_hot_before
        );
        assert!(
            metrics
                .sorafs_taikai_cache_evictions_total
                .with_label_values(&["hot", "capacity"])
                .get()
                > eviction_before
        );
        assert!(
            metrics
                .sorafs_taikai_cache_promotions_total
                .with_label_values(&["warm", "hot"])
                .get()
                > promotion_before
        );
        assert!(
            metrics
                .sorafs_taikai_qos_denied_total
                .with_label_values(&["priority"])
                .get()
                > qos_before
        );
    }

    #[test]
    fn taikai_cache_stats_capture_activity() {
        let mut cache = TaikaiCache::new(TaikaiCacheConfig {
            hot_capacity_bytes: 256,
            hot_retention: Duration::from_secs(30),
            warm_capacity_bytes: 256,
            warm_retention: Duration::from_secs(30),
            cold_capacity_bytes: 256,
            cold_retention: Duration::from_secs(30),
            qos: QosConfig {
                priority_rate_bps: 128,
                standard_rate_bps: 128,
                bulk_rate_bps: 64,
                burst_multiplier: 1,
            },
            reliability: ReliabilityTuning::default(),
        });
        let now = Instant::now();
        let seg_a = dummy_segment(1, 128, QosClass::Priority);
        let seg_b = dummy_segment(2, 128, QosClass::Priority);
        cache.insert(seg_a.clone(), now);
        cache.insert(seg_b.clone(), now);

        cache.get(&seg_a.key(), now);
        let missing = dummy_segment(42, 64, QosClass::Standard).key();
        cache.get(&missing, now);
        assert!(
            cache
                .try_acquire_qos(QosClass::Priority, 1_024, now)
                .is_err()
        );

        let stats = cache.stats();
        assert_eq!(stats.inserts.hot, 2);
        assert_eq!(stats.hits.hot, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.qos_denials.priority, 1);
    }

    #[test]
    fn consistent_hash_ring_moves_subset_of_keys() {
        let shards: Vec<TaikaiShardId> = (0..4).map(TaikaiShardId).collect();
        let ring = TaikaiShardRing::new(shards.clone());
        assert_eq!(ring.replicas().get(), 128);
        assert!(!ring.is_empty());

        let keys: Vec<SegmentKey> = (0..64)
            .map(|seq| dummy_segment(seq, 128, QosClass::Standard).key())
            .collect();
        let initial: Vec<_> = keys.iter().map(|key| ring.locate(key)).collect();

        let mut expanded_shards = shards.clone();
        expanded_shards.push(TaikaiShardId(4));
        let expanded = TaikaiShardRing::new(expanded_shards);

        let reassigned = keys
            .iter()
            .zip(initial.iter())
            .filter(|(key, before)| expanded.locate(key) != **before)
            .count();
        assert!(reassigned <= keys.len() / 3);
    }

    #[test]
    fn taikai_pull_queue_coalesces_batches() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 2,
                max_batch_bytes: 1_024,
                max_in_flight_batches: 2,
                hedge_after: Duration::from_millis(50),
                max_backlog_segments: 8,
            },
            QosConfig::balanced(),
            TaikaiShardRing::new(vec![TaikaiShardId(1)]),
            ReliabilityConfig::default(),
        );
        queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(200, 128, QosClass::Priority).key(),
                QosClass::Priority,
                256,
                None,
            ))
            .expect("enqueue");
        queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(201, 128, QosClass::Standard).key(),
                QosClass::Standard,
                256,
                None,
            ))
            .expect("enqueue");
        queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(202, 128, QosClass::Bulk).key(),
                QosClass::Bulk,
                256,
                None,
            ))
            .expect("enqueue");
        let now = Instant::now();
        let first = queue.issue_ready_batch(now).expect("first batch");
        assert_eq!(first.segments.len(), 2);
        assert_eq!(first.qos, QosClass::Priority);
        let second = queue.issue_ready_batch(now).expect("second batch");
        assert_eq!(second.segments.len(), 1);
        assert_eq!(second.qos, QosClass::Bulk);
        assert!(queue.issue_ready_batch(now).is_none());
    }

    #[test]
    fn taikai_pull_queue_enforces_backpressure() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 1,
                max_batch_bytes: 256,
                max_in_flight_batches: 1,
                hedge_after: Duration::from_millis(20),
                max_backlog_segments: 2,
            },
            QosConfig::balanced(),
            TaikaiShardRing::new(Vec::new()),
            ReliabilityConfig::default(),
        );
        queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(300, 64, QosClass::Priority).key(),
                QosClass::Priority,
                128,
                None,
            ))
            .expect("enqueue");
        queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(301, 64, QosClass::Priority).key(),
                QosClass::Priority,
                128,
                None,
            ))
            .expect("enqueue");
        let err = queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(302, 64, QosClass::Priority).key(),
                QosClass::Priority,
                128,
                None,
            ))
            .expect_err("backpressure");
        assert!(matches!(err, TaikaiQueueError::Backpressure { .. }));
    }

    #[test]
    fn taikai_pull_queue_issues_batches_without_rate_limits() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 1,
                max_batch_bytes: 1_024,
                max_in_flight_batches: 2,
                hedge_after: Duration::from_millis(20),
                max_backlog_segments: 4,
            },
            QosConfig::balanced(),
            TaikaiShardRing::new(vec![TaikaiShardId(9)]),
            ReliabilityConfig::default(),
        );
        let metrics = global_or_default();
        let issued_before = metrics
            .sorafs_taikai_queue_events_total
            .with_label_values(&["issued", "priority"])
            .get();

        let now = Instant::now();
        queue
            .enqueue_at(
                TaikaiPullRequest::new(
                    dummy_segment(600, 128, QosClass::Priority).key(),
                    QosClass::Priority,
                    400,
                    None,
                ),
                now,
            )
            .expect("enqueue first");
        queue
            .enqueue_at(
                TaikaiPullRequest::new(
                    dummy_segment(601, 128, QosClass::Priority).key(),
                    QosClass::Priority,
                    400,
                    None,
                ),
                now,
            )
            .expect("enqueue second");

        let first = queue.issue_ready_batch(now).expect("first batch");
        assert_eq!(first.segments.len(), 1);

        let second = queue.issue_ready_batch(now).expect("second batch");
        assert_eq!(second.segments.len(), 1);
        assert!(
            metrics
                .sorafs_taikai_queue_events_total
                .with_label_values(&["issued", "priority"])
                .get()
                >= issued_before + 2
        );
        let stats = queue.stats();
        assert_eq!(stats.pending_segments, 0);
    }

    #[test]
    fn taikai_pull_queue_shapes_batches_and_requeues() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 1,
                max_batch_bytes: 512,
                max_in_flight_batches: 2,
                hedge_after: Duration::from_millis(20),
                max_backlog_segments: 4,
            },
            QosConfig {
                priority_rate_bps: 256,
                standard_rate_bps: 256,
                bulk_rate_bps: 256,
                burst_multiplier: 1,
            },
            TaikaiShardRing::new(vec![TaikaiShardId(9)]),
            ReliabilityConfig::default(),
        );
        let now = Instant::now();
        queue
            .enqueue_at(
                TaikaiPullRequest::new(
                    dummy_segment(610, 128, QosClass::Priority).key(),
                    QosClass::Priority,
                    256,
                    None,
                ),
                now,
            )
            .expect("enqueue first");
        queue
            .enqueue_at(
                TaikaiPullRequest::new(
                    dummy_segment(611, 128, QosClass::Priority).key(),
                    QosClass::Priority,
                    256,
                    None,
                ),
                now,
            )
            .expect("enqueue second");

        let first = queue.issue_ready_batch(now).expect("first batch");
        assert_eq!(first.segments.len(), 1);

        assert!(queue.issue_ready_batch(now).is_none());
        let mut stats = queue.stats();
        assert_eq!(stats.pending_segments, 1);
        assert_eq!(stats.shaper_denials.priority, 1);

        let second = queue
            .issue_ready_batch(now + Duration::from_secs(1))
            .expect("second batch after refill");
        assert_eq!(second.segments.len(), 1);
        stats = queue.stats();
        assert_eq!(stats.pending_segments, 0);
    }

    #[test]
    fn taikai_pull_queue_hedges_after_timeout() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 1,
                max_batch_bytes: 256,
                max_in_flight_batches: 1,
                hedge_after: Duration::from_millis(5),
                max_backlog_segments: 4,
            },
            QosConfig::balanced(),
            TaikaiShardRing::new(Vec::new()),
            ReliabilityConfig::default(),
        );
        queue
            .enqueue(TaikaiPullRequest::new(
                dummy_segment(400, 64, QosClass::Standard).key(),
                QosClass::Standard,
                128,
                None,
            ))
            .expect("enqueue");
        let now = Instant::now();
        let batch = queue.issue_ready_batch(now).expect("batch");
        assert!(!batch.hedged);
        assert!(
            queue
                .hedge_overdue_batches(now + Duration::from_millis(2))
                .is_empty()
        );
        let hedged = queue.hedge_overdue_batches(now + Duration::from_millis(10));
        assert_eq!(hedged.len(), 1);
        assert!(hedged[0].hedged);
        assert_eq!(
            hedged[0].segments[0].key.sequence(),
            batch.segments[0].key.sequence()
        );
    }

    #[test]
    fn taikai_pull_queue_hedging_respects_shaper_budget() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 1,
                max_batch_bytes: 512,
                max_in_flight_batches: 1,
                hedge_after: Duration::from_millis(1),
                max_backlog_segments: 2,
            },
            QosConfig {
                priority_rate_bps: 256,
                standard_rate_bps: 256,
                bulk_rate_bps: 256,
                burst_multiplier: 1,
            },
            TaikaiShardRing::new(vec![TaikaiShardId(5)]),
            ReliabilityConfig::default(),
        );
        let now = Instant::now();
        queue
            .enqueue_at(
                TaikaiPullRequest::new(
                    dummy_segment(620, 128, QosClass::Priority).key(),
                    QosClass::Priority,
                    256,
                    None,
                ),
                now,
            )
            .expect("enqueue");

        let _ = queue.issue_ready_batch(now).expect("batch issued");

        assert!(
            queue
                .hedge_overdue_batches(now + Duration::from_millis(2))
                .is_empty()
        );
        let stats = queue.stats();
        assert_eq!(stats.shaper_denials.priority, 1);
        assert_eq!(stats.hedged_batches, 0);

        let hedged = queue.hedge_overdue_batches(now + Duration::from_secs(1));
        assert_eq!(hedged.len(), 1);
        assert!(hedged[0].hedged);
    }

    #[test]
    fn taikai_pull_queue_trips_circuit_and_failsover() {
        let metrics = global_or_default();
        set_taikai_shard_open(TaikaiShardId(1), false);
        set_taikai_shard_open(TaikaiShardId(2), false);
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig::default(),
            QosConfig::balanced(),
            TaikaiShardRing::new(vec![TaikaiShardId(1), TaikaiShardId(2)]),
            ReliabilityConfig::default(),
        );
        let now = Instant::now();
        let request = TaikaiPullRequest::new(
            SegmentKey::from_envelope(&sample_envelope(888)),
            QosClass::Priority,
            512,
            None,
        );

        let mut preferred = None;
        for _ in 0..ReliabilityConfig::default().failures_to_trip {
            queue.enqueue_at(request.clone(), now).expect("enqueue");
            let batch = queue.issue_ready_batch(now).expect("batch");
            preferred.get_or_insert(batch.shard.expect("shard selected"));
            let ticket = TaikaiPullTicket::from(batch.id);
            assert!(queue.fail_at(ticket, now));
        }

        let preferred = preferred.expect("preferred shard discovered");
        let preferred_label = shard_label(preferred);
        assert_eq!(
            metrics
                .sorafs_taikai_shard_circuits_open
                .with_label_values(&[&preferred_label])
                .get(),
            1
        );

        queue.enqueue_at(request, now).expect("enqueue");
        let batch = queue.issue_ready_batch(now).expect("failover batch");
        let selected = batch.shard.expect("shard selected");
        assert_ne!(selected, preferred);

        let stats = queue.stats();
        assert!(stats.failovers >= 1);
        assert_eq!(stats.open_circuits, 1);
    }

    #[test]
    fn taikai_cache_handle_exposes_queue_api() {
        let handle = TaikaiCacheHandle::from_config(TaikaiCacheConfig::default());
        handle.configure_shards(vec![TaikaiShardId(7)]);
        let request = TaikaiPullRequest::new(
            SegmentKey::from_envelope(&sample_envelope(777)),
            QosClass::Priority,
            768,
            None,
        );
        handle.enqueue_pull(request).expect("enqueue succeeds");

        let now = Instant::now();
        let batch = handle
            .issue_ready_batch_at(now)
            .expect("queue accessible")
            .expect("batch emitted");
        assert_eq!(batch.shard, Some(TaikaiShardId(7)));
        assert_eq!(batch.total_bytes, 768);
        assert!(!batch.hedged);

        assert!(
            handle
                .hedge_overdue_batches_at(now + Duration::from_millis(50))
                .expect("queue accessible")
                .is_empty()
        );

        let hedged = handle
            .hedge_overdue_batches_at(now + Duration::from_millis(250))
            .expect("queue accessible");
        assert_eq!(hedged.len(), 1);
        assert!(hedged[0].hedged);

        let ticket = TaikaiPullTicket::from(batch.id);
        assert!(handle.complete_batch(ticket).expect("queue accessible"));
        assert!(!handle.complete_batch(ticket).expect("queue accessible"));
    }

    #[test]
    fn taikai_cache_handle_propagates_backpressure() {
        let handle = TaikaiCacheHandle::from_config(TaikaiCacheConfig::default());
        let limit = 256;
        for seq in 0..limit {
            let envelope = sample_envelope(seq as u64);
            let request = TaikaiPullRequest::new(
                SegmentKey::from_envelope(&envelope),
                QosClass::Bulk,
                64,
                None,
            );
            handle.enqueue_pull(request).expect("within backlog");
        }
        let err = handle
            .enqueue_pull(TaikaiPullRequest::new(
                SegmentKey::from_envelope(&sample_envelope(9_999)),
                QosClass::Bulk,
                64,
                None,
            ))
            .expect_err("backpressure triggered");
        assert!(matches!(err, TaikaiQueueError::Backpressure { .. }));
    }

    #[test]
    fn cache_admission_tracker_updates_shard_ring() {
        let handle = TaikaiCacheHandle::from_config(TaikaiCacheConfig::default());
        let replay =
            CacheAdmissionReplayFilter::new(Duration::from_millis(250), 8).expect("replay filter");
        let mut tracker = CacheAdmissionTracker::new(handle.clone(), replay);

        let issued_ms = 1_726_000_500_000;
        let ttl = Duration::from_millis(200);
        let gossip = cache_admission_gossip(TaikaiShardId(7), 64, issued_ms, ttl);
        assert!(
            tracker
                .ingest(&gossip, issued_ms)
                .expect("ingestion succeeds")
        );
        assert_eq!(tracker.active_shards(), vec![TaikaiShardId(7)]);

        let request = TaikaiPullRequest::new(
            SegmentKey::from_envelope(&sample_envelope(900)),
            QosClass::Priority,
            128,
            None,
        );
        handle.enqueue_pull(request).expect("enqueue");
        let now = Instant::now();
        let batch = handle
            .issue_ready_batch_at(now)
            .expect("queue available")
            .expect("batch emitted");
        assert_eq!(batch.shard, Some(TaikaiShardId(7)));
        let _ = handle.complete_batch(TaikaiPullTicket::from(batch.id));

        let expire_at = issued_ms
            .saturating_add(u64::try_from(ttl.as_millis()).expect("ttl fits in u64"))
            .saturating_add(1);
        assert!(tracker.evict_expired(expire_at));
        assert!(tracker.active_shards().is_empty());

        let request = TaikaiPullRequest::new(
            SegmentKey::from_envelope(&sample_envelope(901)),
            QosClass::Priority,
            64,
            None,
        );
        handle.enqueue_pull(request).expect("enqueue second");
        let batch = handle
            .issue_ready_batch_at(now)
            .expect("queue available")
            .expect("batch emitted");
        assert_eq!(batch.shard, None);
    }

    #[test]
    fn taikai_pull_queue_issue_specific_respects_shaper() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig {
                max_batch_segments: 1,
                max_batch_bytes: 512,
                max_in_flight_batches: 2,
                hedge_after: Duration::from_millis(10),
                max_backlog_segments: 4,
            },
            QosConfig {
                priority_rate_bps: 256,
                standard_rate_bps: 256,
                bulk_rate_bps: 256,
                burst_multiplier: 1,
            },
            TaikaiShardRing::new(vec![TaikaiShardId(6)]),
            ReliabilityConfig::default(),
        );
        let first = dummy_segment(630, 64, QosClass::Priority);
        let second = dummy_segment(631, 64, QosClass::Priority);
        let first_key = first.key();
        let second_key = second.key();
        let now = Instant::now();

        queue
            .enqueue_at(
                TaikaiPullRequest::new(first_key.clone(), QosClass::Priority, 256, None),
                now,
            )
            .expect("enqueue first");
        queue
            .enqueue_at(
                TaikaiPullRequest::new(second_key.clone(), QosClass::Priority, 256, None),
                now,
            )
            .expect("enqueue second");

        let _ = queue.issue_ready_batch(now).expect("first batch issued");
        assert!(queue.issue_specific(&second_key, now).is_none());
        let stats = queue.stats();
        assert_eq!(stats.shaper_denials.priority, 1);
        assert_eq!(stats.pending_segments, 1);

        let batch = queue
            .issue_specific(&second_key, now + Duration::from_secs(1))
            .expect("second batch issued after refill");
        assert_eq!(batch.segments.len(), 1);
        assert_eq!(queue.stats().pending_segments, 0);
    }

    #[test]
    fn taikai_pull_queue_issue_specific_removes_target() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig::default(),
            QosConfig::balanced(),
            TaikaiShardRing::new(vec![TaikaiShardId(3)]),
            ReliabilityConfig::default(),
        );
        let first = dummy_segment(500, 64, QosClass::Priority);
        let second = dummy_segment(501, 64, QosClass::Priority);
        let first_key = first.key();
        let second_key = second.key();
        queue
            .enqueue(TaikaiPullRequest::new(
                first_key.clone(),
                QosClass::Priority,
                64,
                None,
            ))
            .expect("enqueue first");
        queue
            .enqueue(TaikaiPullRequest::new(
                second_key.clone(),
                QosClass::Priority,
                64,
                None,
            ))
            .expect("enqueue second");
        let batch = queue
            .issue_specific(&second_key, Instant::now())
            .expect("issue second batch");
        assert_eq!(batch.segments.len(), 1);
        assert_eq!(batch.segments[0].key, second_key);
        assert_eq!(queue.stats().pending_segments, 1);
        assert!(queue.issue_specific(&second_key, Instant::now()).is_none());
        assert!(queue.issue_specific(&first_key, Instant::now()).is_some());
    }

    #[test]
    fn taikai_pull_queue_cancel_pending_drops_request() {
        let mut queue = TaikaiPullQueue::new(
            TaikaiPullQueueConfig::default(),
            QosConfig::balanced(),
            TaikaiShardRing::new(Vec::new()),
            ReliabilityConfig::default(),
        );
        let key = dummy_segment(777, 32, QosClass::Standard).key();
        queue
            .enqueue(TaikaiPullRequest::new(
                key.clone(),
                QosClass::Standard,
                32,
                None,
            ))
            .expect("enqueue");
        assert!(queue.cancel_pending(&key));
        assert_eq!(queue.stats().pending_segments, 0);
        assert!(!queue.cancel_pending(&key));
    }
}
