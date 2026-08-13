//! Fail-closed in-memory replay protection shared by Torii authentication paths.
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    num::NonZeroUsize,
    sync::Mutex,
    time::{Duration, Instant},
};
use iroha_crypto::Hash;
const REPLAY_KEY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:torii:replay-cache:v1\0";
/// Reason a nonce could not be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InsertError {
    /// The nonce is already live in the cache.
    Replay,
    /// All configured slots contain live nonces.
    Capacity,
    /// The configured lifetime cannot be represented by the monotonic clock.
    LifetimeOverflow,
}
#[derive(Debug)]
struct Inner {
    ttl: Duration,
    capacity: NonZeroUsize,
    entries: HashMap<Hash, Instant>,
    expirations: BinaryHeap<Reverse<(Instant, Hash)>>,
}
impl Inner {
    fn prune_expired(&mut self, now: Instant) {
        while let Some(Reverse((expires_at, _))) = self.expirations.peek() {
            if *expires_at > now {
                break;
            }
            let Some(Reverse((expires_at, key))) = self.expirations.pop() else {
                break;
            };
            if self.entries.get(&key) == Some(&expires_at) {
                self.entries.remove(&key);
            }
        }
    }
}
/// Bounded replay cache that never evicts an unexpired nonce.
#[derive(Debug)]
pub(crate) struct ReplayCache {
    inner: Mutex<Inner>,
}
impl ReplayCache {
    /// Create a cache with a fixed nonce lifetime and capacity.
    pub(crate) fn new(ttl: Duration, capacity: NonZeroUsize) -> Self {
        debug_assert!(!ttl.is_zero());
        Self {
            inner: Mutex::new(Inner {
                ttl,
                capacity,
                entries: HashMap::new(),
                expirations: BinaryHeap::new(),
            }),
        }
    }
    /// Reconfigure future inserts without discarding live replay evidence.
    pub(crate) fn configure(&self, ttl: Duration, capacity: NonZeroUsize) {
        debug_assert!(!ttl.is_zero());
        let now = Instant::now();
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.prune_expired(now);
        inner.ttl = ttl;
        inner.capacity = capacity;
    }
    /// Admit a new nonce, rejecting replay and saturated live state.
    pub(crate) fn check_and_insert(&self, key: String) -> Result<(), InsertError> {
        self.check_and_insert_at(key, Instant::now())
    }
    /// Admit a caller-domain-separated fixed-size replay digest.
    ///
    /// Protocol paths that can stream their structured replay identity should
    /// use this seam instead of first formatting an attacker-sized `String`.
    pub(crate) fn check_and_insert_digest(&self, key: Hash) -> Result<(), InsertError> {
        self.check_and_insert_digest_at(key, Instant::now())
    }
    fn check_and_insert_at(&self, key: String, now: Instant) -> Result<(), InsertError> {
        // Callers may include an attacker-sized authority representation in
        // this key. Retain only a domain-separated fixed-size digest; keeping
        // the original String in both indices would turn the count cap into a
        // large variable-byte cache.
        let key = Hash::new_from_chunks(&[REPLAY_KEY_DIGEST_DOMAIN_V1, key.as_bytes()]);
        self.check_and_insert_digest_at(key, now)
    }
    fn check_and_insert_digest_at(&self, key: Hash, now: Instant) -> Result<(), InsertError> {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.prune_expired(now);
        if inner.entries.contains_key(&key) {
            return Err(InsertError::Replay);
        }
        if inner.entries.len() >= inner.capacity.get() {
            return Err(InsertError::Capacity);
        }
        let expires_at = now
            .checked_add(inner.ttl)
            .ok_or(InsertError::LifetimeOverflow)?;
        inner.entries.insert(key.clone(), expires_at);
        inner.expirations.push(Reverse((expires_at, key)));
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_cache_preserves_every_live_nonce() {
        let cache = ReplayCache::new(
            Duration::from_secs(300),
            NonZeroUsize::new(2).expect("non-zero"),
        );
        cache
            .check_and_insert("protected-a".to_owned())
            .expect("first nonce");
        cache
            .check_and_insert("protected-b".to_owned())
            .expect("second nonce");
        assert_eq!(
            cache.check_and_insert("overflow".to_owned()),
            Err(InsertError::Capacity)
        );
        assert_eq!(
            cache.check_and_insert("protected-a".to_owned()),
            Err(InsertError::Replay)
        );
        assert_eq!(
            cache.check_and_insert("protected-b".to_owned()),
            Err(InsertError::Replay)
        );
    }
    #[test]
    fn expired_nonce_releases_capacity_without_sleeping() {
        let cache = ReplayCache::new(
            Duration::from_secs(1),
            NonZeroUsize::new(1).expect("non-zero"),
        );
        let start = Instant::now();
        cache
            .check_and_insert_at("expired".to_owned(), start)
            .expect("first nonce");
        cache
            .check_and_insert_at("replacement".to_owned(), start + Duration::from_secs(1))
            .expect("expired nonce must release its slot");
        assert_eq!(
            cache.check_and_insert_at("expired".to_owned(), start + Duration::from_secs(1)),
            Err(InsertError::Capacity)
        );
    }
    #[test]
    fn shrinking_capacity_never_evicts_live_evidence() {
        let cache = ReplayCache::new(
            Duration::from_secs(300),
            NonZeroUsize::new(2).expect("non-zero"),
        );
        cache
            .check_and_insert("protected-a".to_owned())
            .expect("first nonce");
        cache
            .check_and_insert("protected-b".to_owned())
            .expect("second nonce");
        cache.configure(
            Duration::from_secs(300),
            NonZeroUsize::new(1).expect("non-zero"),
        );
        assert_eq!(
            cache.check_and_insert("protected-a".to_owned()),
            Err(InsertError::Replay)
        );
        assert_eq!(
            cache.check_and_insert("protected-b".to_owned()),
            Err(InsertError::Replay)
        );
        assert_eq!(
            cache.check_and_insert("new".to_owned()),
            Err(InsertError::Capacity)
        );
    }
    #[test]
    fn retained_replay_keys_are_fixed_size_digests() {
        let cache = ReplayCache::new(
            Duration::from_secs(300),
            NonZeroUsize::new(1).expect("non-zero"),
        );
        cache
            .check_and_insert("x".repeat(1024 * 1024))
            .expect("large transient key should hash into one cache slot");
        let inner = cache
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let retained_key = inner.entries.keys().next().expect("one retained digest");
        assert_eq!(std::mem::size_of_val(retained_key), Hash::LENGTH);
        let Reverse((_expires_at, queued_key)) = inner
            .expirations
            .peek()
            .expect("one retained expiration digest");
        assert_eq!(std::mem::size_of_val(queued_key), Hash::LENGTH);
    }
    #[test]
    fn structured_digest_admission_preserves_one_shot_semantics() {
        let cache = ReplayCache::new(
            Duration::from_secs(300),
            NonZeroUsize::new(2).expect("non-zero"),
        );
        let first = Hash::new(b"structured-a");
        let second = Hash::new(b"structured-b");
        cache
            .check_and_insert_digest(first.clone())
            .expect("first structured digest");
        assert_eq!(
            cache.check_and_insert_digest(first),
            Err(InsertError::Replay)
        );
        cache
            .check_and_insert_digest(second)
            .expect("a distinct structured digest uses the second slot");
    }
}
