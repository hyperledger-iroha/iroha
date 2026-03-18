//! Replay cache for data availability ingest.
//!
//! The DA ingest pipeline must reject duplicate manifests and stale sequence numbers so
//! storage operators cannot replay previously accepted blobs. The replay cache keeps a
//! bounded, per-lane/per-epoch window of recently seen manifest fingerprints and
//! exposes deterministic outcomes that higher layers can map to admission errors.

use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use iroha_data_model::nexus::LaneId;
use parking_lot::Mutex;

/// Identifier for a `(lane, epoch)` pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaneEpoch {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Sequencer epoch associated with the manifest.
    pub epoch: u64,
}

impl LaneEpoch {
    /// Construct a new `(lane, epoch)` handle.
    #[must_use]
    pub const fn new(lane_id: LaneId, epoch: u64) -> Self {
        Self { lane_id, epoch }
    }
}

/// Blake3 fingerprint used for DA replay detection. The ingest path computes this over a
/// canonical Norito manifest template with `storage_ticket` and `issued_at_unix` zeroed
/// before de-duplicating entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplayFingerprint([u8; blake3::OUT_LEN]);

impl ReplayFingerprint {
    /// Construct a fingerprint from a raw Blake3 hash output.
    ///
    /// # Panics
    /// Panics if the provided slice does not have the expected length.
    #[must_use]
    pub fn from_hash_bytes(bytes: &[u8]) -> Self {
        assert_eq!(
            bytes.len(),
            blake3::OUT_LEN,
            "fingerprint must match blake3 output length"
        );
        let mut buf = [0u8; blake3::OUT_LEN];
        buf.copy_from_slice(bytes);
        Self(buf)
    }

    /// Construct a fingerprint from a [`blake3::Hash`].
    #[must_use]
    pub fn from_hash(hash: blake3::Hash) -> Self {
        Self(hash.into())
    }

    /// Access the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; blake3::OUT_LEN] {
        &self.0
    }
}

impl From<[u8; blake3::OUT_LEN]> for ReplayFingerprint {
    fn from(bytes: [u8; blake3::OUT_LEN]) -> Self {
        Self(bytes)
    }
}

impl From<ReplayFingerprint> for [u8; blake3::OUT_LEN] {
    fn from(value: ReplayFingerprint) -> Self {
        value.0
    }
}

/// High-level identifier used when inserting items into the replay cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplayKey {
    /// Lane and epoch this manifest is scoped to.
    pub lane_epoch: LaneEpoch,
    /// Monotonic sequence number advertised alongside the manifest.
    pub sequence: u64,
    /// Canonical fingerprint derived from the manifest contents.
    pub fingerprint: ReplayFingerprint,
}

impl ReplayKey {
    /// Helper to create a new key.
    #[must_use]
    pub const fn new(lane_epoch: LaneEpoch, sequence: u64, fingerprint: ReplayFingerprint) -> Self {
        Self {
            lane_epoch,
            sequence,
            fingerprint,
        }
    }
}

/// Configuration for [`ReplayCache`]. This governs eviction, TTL, and sequence windows.
#[derive(Clone, Copy, Debug)]
pub struct ReplayCacheConfig {
    /// Maximum number of manifests tracked per `(lane, epoch)` window.
    pub max_entries_per_lane: NonZeroUsize,
    /// How long a manifest stays live in the cache after its last observation.
    pub ttl: Duration,
    /// Maximum allowed distance from the highest observed sequence number before a new
    /// manifest is considered stale.
    pub max_sequence_lag: u64,
}

impl ReplayCacheConfig {
    /// Default TTL used for DA manifest replay detection (15 minutes).
    pub const DEFAULT_TTL: Duration = Duration::from_mins(15);
    /// Default capacity per `(lane, epoch)` window (4096 entries).
    pub const DEFAULT_CAPACITY: usize = 4096;
    /// Default sequence lag tolerance (4096 slots behind the high-water mark).
    pub const DEFAULT_SEQUENCE_LAG: u64 = 4096;

    /// Construct a configuration using workspace defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_entries_per_lane: NonZeroUsize::new(Self::DEFAULT_CAPACITY)
                .expect("DEFAULT_CAPACITY must be non-zero"),
            ttl: Self::DEFAULT_TTL,
            max_sequence_lag: Self::DEFAULT_SEQUENCE_LAG,
        }
    }

    /// Override the per-lane capacity.
    #[must_use]
    pub fn with_max_entries_per_lane(mut self, capacity: NonZeroUsize) -> Self {
        self.max_entries_per_lane = capacity;
        self
    }

    /// Override the TTL.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Override the maximum sequence lag.
    #[must_use]
    pub fn with_max_sequence_lag(mut self, lag: u64) -> Self {
        self.max_sequence_lag = lag;
        self
    }
}

impl Default for ReplayCacheConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Result returned when inserting a manifest fingerprint into the replay cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayInsertOutcome {
    /// Manifest is fresh and accepted into the cache.
    Fresh {
        /// Snapshot captured after inserting the manifest.
        snapshot: ReplayEntrySnapshot,
    },
    /// Manifest was already observed; the cache updates its metadata.
    Duplicate {
        /// Snapshot captured after registering the duplicate manifest.
        snapshot: ReplayEntrySnapshot,
    },
    /// Sequence number fell outside the permitted lag window.
    StaleSequence {
        /// Highest sequence observed for this `(lane, epoch)` window.
        highest_observed: u64,
    },
    /// The manifest reused a sequence number but had a conflicting fingerprint.
    ConflictingFingerprint {
        /// Fingerprint that was already registered under the same sequence number.
        expected: ReplayFingerprint,
        /// Fingerprint that the caller attempted to insert.
        observed: ReplayFingerprint,
    },
}

/// Snapshot describing the state of a cached manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayEntrySnapshot {
    /// Timestamp when the manifest was first observed.
    pub first_seen: Instant,
    /// Timestamp of the most recent observation.
    pub last_seen: Instant,
    /// Number of times the manifest has been observed.
    pub hit_count: u32,
    /// Sequence number assigned to the manifest.
    pub sequence: u64,
}

/// Concurrency-safe cache tracking recently observed DA manifest fingerprints.
#[derive(Debug)]
pub struct ReplayCache {
    config: ReplayCacheConfig,
    inner: Mutex<ReplayCacheInner>,
}

impl ReplayCache {
    /// Construct a new replay cache using the provided configuration.
    #[must_use]
    pub fn new(config: ReplayCacheConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(ReplayCacheInner::default()),
        }
    }

    /// Insert a manifest fingerprint into the cache and obtain the resulting outcome.
    #[must_use]
    pub fn insert(&self, key: ReplayKey, now: Instant) -> ReplayInsertOutcome {
        let mut guard = self.inner.lock();
        guard.prune(now, &self.config);

        let lane_state = guard.lanes.entry(key.lane_epoch).or_default();

        if let Some(floor) = lane_state.stale_floor {
            if key.sequence <= floor {
                return ReplayInsertOutcome::StaleSequence {
                    highest_observed: lane_state.highest_sequence.max(floor),
                };
            }
        }

        if lane_state.highest_sequence >= key.sequence {
            let lag = lane_state.highest_sequence.saturating_sub(key.sequence);
            if lag > self.config.max_sequence_lag {
                return ReplayInsertOutcome::StaleSequence {
                    highest_observed: lane_state.highest_sequence,
                };
            }
        }

        if let Some(entry) = lane_state.entries.get_mut(&key.sequence) {
            if entry.fingerprint != key.fingerprint {
                return ReplayInsertOutcome::ConflictingFingerprint {
                    expected: entry.fingerprint,
                    observed: key.fingerprint,
                };
            }

            entry.hit_count = entry.hit_count.saturating_add(1);
            entry.last_seen = now;

            ReplayInsertOutcome::Duplicate {
                snapshot: entry.snapshot(key.sequence),
            }
        } else {
            let entry = Entry {
                fingerprint: key.fingerprint,
                first_seen: now,
                last_seen: now,
                hit_count: 1,
            };
            let updated = lane_state.highest_sequence.max(key.sequence);
            lane_state.highest_sequence = updated;
            lane_state.entries.insert(key.sequence, entry);
            lane_state.enforce_capacity(&self.config);

            ReplayInsertOutcome::Fresh {
                snapshot: lane_state
                    .entries
                    .get(&key.sequence)
                    .expect("entry must exist after insertion")
                    .snapshot(key.sequence),
            }
        }
    }

    /// Prime the replay cache with a known highest sequence for a `(lane, epoch)` window.
    /// This is useful when restoring state from persisted cursors after a restart.
    pub fn prime_lane_epoch(&self, lane_epoch: LaneEpoch, highest_sequence: u64) {
        let mut guard = self.inner.lock();
        let lane_state = guard.lanes.entry(lane_epoch).or_default();
        let primed = lane_state.highest_sequence.max(highest_sequence);
        lane_state.highest_sequence = primed;
        lane_state.stale_floor = Some(primed);
    }

    /// Drop cached manifests for a `(lane, epoch)` window. This is useful during epoch
    /// transitions or when replay state is reset via governance.
    pub fn clear_lane_epoch(&self, lane_epoch: LaneEpoch) {
        let mut guard = self.inner.lock();
        guard.lanes.remove(&lane_epoch);
    }

    /// Inspect the number of cached manifests for a `(lane, epoch)` window. Intended for
    /// diagnostics and testing only.
    #[must_use]
    pub fn len_for_lane_epoch(&self, lane_epoch: LaneEpoch) -> usize {
        let guard = self.inner.lock();
        guard
            .lanes
            .get(&lane_epoch)
            .map(|state| state.entries.len())
            .unwrap_or_default()
    }
}

#[derive(Default, Debug)]
struct ReplayCacheInner {
    lanes: BTreeMap<LaneEpoch, LaneState>,
}

impl ReplayCacheInner {
    fn prune(&mut self, now: Instant, config: &ReplayCacheConfig) {
        let mut empty_lanes = Vec::new();

        for (lane_epoch, state) in &mut self.lanes {
            if state.prune(now, config) {
                empty_lanes.push(*lane_epoch);
            }
        }

        for lane in empty_lanes {
            self.lanes.remove(&lane);
        }
    }
}

#[derive(Debug, Default)]
struct LaneState {
    highest_sequence: u64,
    stale_floor: Option<u64>,
    entries: BTreeMap<u64, Entry>,
}

impl LaneState {
    /// Returns `true` if the state became empty after pruning.
    fn prune(&mut self, now: Instant, config: &ReplayCacheConfig) -> bool {
        if self.entries.is_empty() {
            return self.stale_floor.is_none();
        }

        let ttl = config.ttl;
        let highest = self.highest_sequence;
        let max_lag = config.max_sequence_lag;

        self.entries.retain(|sequence, entry| {
            let expired = now
                .checked_duration_since(entry.last_seen)
                .is_some_and(|duration| duration > ttl);
            let too_far = highest.saturating_sub(*sequence) > max_lag;
            !(expired || too_far)
        });

        self.entries.is_empty()
    }

    fn enforce_capacity(&mut self, config: &ReplayCacheConfig) {
        let max_entries = config.max_entries_per_lane.get();
        while self.entries.len() > max_entries {
            if let Some((&sequence, _)) =
                self.entries.iter().min_by_key(|(_, entry)| entry.last_seen)
            {
                self.entries.remove(&sequence);
            } else {
                break;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    fingerprint: ReplayFingerprint,
    first_seen: Instant,
    last_seen: Instant,
    hit_count: u32,
}

impl Entry {
    fn snapshot(&self, sequence: u64) -> ReplayEntrySnapshot {
        ReplayEntrySnapshot {
            first_seen: self.first_seen,
            last_seen: self.last_seen,
            hit_count: self.hit_count,
            sequence,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        num::NonZeroUsize,
        thread,
        time::{Duration, Instant},
    };

    use iroha_data_model::nexus::LaneId;
    use proptest::{collection::vec, prelude::*};

    use super::*;

    fn fingerprint(seed: u8) -> ReplayFingerprint {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&[seed]);
        ReplayFingerprint::from_hash(hasher.finalize())
    }

    #[test]
    fn fresh_insert_is_recorded() {
        let cache = ReplayCache::new(ReplayCacheConfig::new());
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 42);
        let key = ReplayKey::new(lane_epoch, 1, fingerprint(1));
        let now = Instant::now();

        let outcome = cache.insert(key, now);

        match outcome {
            ReplayInsertOutcome::Fresh { snapshot } => {
                assert_eq!(snapshot.sequence, 1);
                assert_eq!(snapshot.hit_count, 1);
                assert_eq!(snapshot.first_seen, now);
                assert_eq!(snapshot.last_seen, now);
            }
            other => panic!("expected Fresh, got {other:?}"),
        }
        assert_eq!(cache.len_for_lane_epoch(lane_epoch), 1);
    }

    #[test]
    fn duplicate_updates_hit_count() {
        let cache = ReplayCache::new(ReplayCacheConfig::new());
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 7);
        let key = ReplayKey::new(lane_epoch, 5, fingerprint(5));

        let first = Instant::now();
        let second = first + Duration::from_secs(1);
        let third = second + Duration::from_secs(1);

        assert!(matches!(
            cache.insert(key, first),
            ReplayInsertOutcome::Fresh { .. }
        ));
        assert!(matches!(
            cache.insert(key, second),
            ReplayInsertOutcome::Duplicate {
                snapshot: ReplayEntrySnapshot {
                    hit_count: 2,
                    last_seen,
                    ..
                }
            } if last_seen == second
        ));
        assert!(matches!(
            cache.insert(key, third),
            ReplayInsertOutcome::Duplicate {
                snapshot: ReplayEntrySnapshot {
                    hit_count: 3,
                    last_seen,
                    ..
                }
            } if last_seen == third
        ));
    }

    #[test]
    fn conflicting_fingerprint_detected() {
        let cache = ReplayCache::new(ReplayCacheConfig::new());
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 1);

        let key_a = ReplayKey::new(lane_epoch, 10, fingerprint(10));
        let key_b = ReplayKey::new(lane_epoch, 10, fingerprint(11));

        let now = Instant::now();

        assert!(matches!(
            cache.insert(key_a, now),
            ReplayInsertOutcome::Fresh { .. }
        ));
        assert!(matches!(
            cache.insert(key_b, now),
            ReplayInsertOutcome::ConflictingFingerprint { .. }
        ));
    }

    #[test]
    fn stale_sequence_rejected() {
        let config = ReplayCacheConfig::new().with_max_sequence_lag(2);
        let cache = ReplayCache::new(config);
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 1);

        assert!(matches!(
            cache.insert(
                ReplayKey::new(lane_epoch, 5, fingerprint(1)),
                Instant::now()
            ),
            ReplayInsertOutcome::Fresh { .. }
        ));
        assert!(matches!(
            cache.insert(
                ReplayKey::new(lane_epoch, 2, fingerprint(2)),
                Instant::now()
            ),
            ReplayInsertOutcome::StaleSequence {
                highest_observed: 5
            }
        ));
    }

    #[test]
    fn ttl_eviction_clears_entries() {
        let config = ReplayCacheConfig::new()
            .with_ttl(Duration::from_millis(10))
            .with_max_entries_per_lane(NonZeroUsize::new(16).unwrap());
        let cache = ReplayCache::new(config);
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 1);
        let key = ReplayKey::new(lane_epoch, 1, fingerprint(1));

        let now = Instant::now();
        assert!(matches!(
            cache.insert(key, now),
            ReplayInsertOutcome::Fresh { .. }
        ));
        assert_eq!(cache.len_for_lane_epoch(lane_epoch), 1);

        thread::sleep(Duration::from_millis(20));
        let later = Instant::now();
        // Trigger prune by inserting a different sequence.
        assert!(matches!(
            cache.insert(ReplayKey::new(lane_epoch, 2, fingerprint(2)), later),
            ReplayInsertOutcome::Fresh { .. }
        ));
        assert_eq!(cache.len_for_lane_epoch(lane_epoch), 1);
    }

    #[test]
    fn capacity_enforced() {
        let capacity = NonZeroUsize::new(4).unwrap();
        let cache = ReplayCache::new(ReplayCacheConfig::new().with_max_entries_per_lane(capacity));
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 1);
        let base = Instant::now();

        for idx in 0_u64..6 {
            let idx_byte = u8::try_from(idx).expect("idx fits in u8");
            let key = ReplayKey::new(lane_epoch, idx, fingerprint(idx_byte));
            assert!(matches!(
                cache.insert(key, base + Duration::from_millis(idx)),
                ReplayInsertOutcome::Fresh { .. }
            ));
        }

        assert_eq!(cache.len_for_lane_epoch(lane_epoch), capacity.get());
    }

    proptest! {
        #[test]
        fn fresh_then_duplicate_for_replayed_sequences(
            sequences in vec(0u64..256, 1..32)
        ) {
            let cache = ReplayCache::new(
                ReplayCacheConfig::new().with_max_sequence_lag(u64::MAX)
            );
            let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 99);
            let mut seen = BTreeSet::new();
            let mut now = Instant::now();

            for sequence in sequences {
                now += Duration::from_micros(1);
                let key = ReplayKey::new(
                    lane_epoch,
                    sequence,
                    fingerprint((sequence & 0xFF) as u8)
                );
                let outcome = cache.insert(key, now);
                if seen.insert(sequence) {
                    let is_fresh = matches!(outcome, ReplayInsertOutcome::Fresh { .. });
                    prop_assert!(is_fresh);
                } else {
                    let is_duplicate = matches!(outcome, ReplayInsertOutcome::Duplicate { .. });
                    prop_assert!(is_duplicate);
                }
            }
        }
    }

    #[test]
    fn prime_restores_highest_sequence() {
        let cache = ReplayCache::new(ReplayCacheConfig::new());
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 5);
        cache.prime_lane_epoch(lane_epoch, 10);

        let outcome = cache.insert(
            ReplayKey::new(lane_epoch, 4, fingerprint(1)),
            Instant::now(),
        );

        match outcome {
            ReplayInsertOutcome::StaleSequence { highest_observed } => {
                assert_eq!(highest_observed, 10);
            }
            other => panic!("expected stale sequence, got {other:?}"),
        }
    }

    #[test]
    fn prime_enforces_floor_even_when_within_lag() {
        let cache = ReplayCache::new(ReplayCacheConfig::new().with_max_sequence_lag(4096));
        let lane_epoch = LaneEpoch::new(LaneId::SINGLE, 7);
        cache.prime_lane_epoch(lane_epoch, 50);

        let outcome = cache.insert(
            ReplayKey::new(lane_epoch, 49, fingerprint(2)),
            Instant::now(),
        );

        match outcome {
            ReplayInsertOutcome::StaleSequence { highest_observed } => {
                assert_eq!(highest_observed, 50);
            }
            other => panic!("expected stale sequence due to primed floor, got {other:?}"),
        }
    }
}
