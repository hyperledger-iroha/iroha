//! Concurrency tracking for SoraFS stream tokens.
use dashmap::{DashMap, mapref::entry::Entry};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
const MAX_TOKEN_ID_BYTES: usize = 128;
/// Tracks active stream requests per token to enforce concurrency budgets.
#[derive(Clone, Default)]
pub struct StreamTokenConcurrencyTracker {
    inner: Arc<StreamTokenConcurrencyInner>,
}
impl StreamTokenConcurrencyTracker {
    /// Attempt to acquire a concurrency slot for `token_id`.
    ///
    /// Returns a guard that releases the slot when dropped. Zero limits and malformed token IDs
    /// fail closed rather than creating an unbounded concurrency grant.
    ///
    /// # Errors
    ///
    /// Returns [`ConcurrencyLimitExceeded`] when `max_streams` would be exceeded for `token_id`.
    pub fn try_acquire(
        &self,
        token_id: &str,
        max_streams: u16,
    ) -> Result<Option<StreamTokenConcurrencyPermit>, ConcurrencyLimitExceeded> {
        if max_streams == 0 || !valid_token_id(token_id) {
            return Err(ConcurrencyLimitExceeded);
        }
        let inner = Arc::clone(&self.inner);
        let counter = inner.try_acquire(token_id, max_streams)?;
        Ok(Some(StreamTokenConcurrencyPermit {
            inner,
            token_id: token_id.to_owned(),
            counter,
        }))
    }
}
#[derive(Debug, Default)]
struct StreamTokenConcurrencyInner {
    counters: DashMap<String, Arc<TokenCounter>>,
}
impl StreamTokenConcurrencyInner {
    fn try_acquire(
        &self,
        token_id: &str,
        max_streams: u16,
    ) -> Result<Arc<TokenCounter>, ConcurrencyLimitExceeded> {
        let entry = self
            .counters
            .entry(token_id.to_owned())
            .or_insert_with(|| Arc::new(TokenCounter::default()));
        entry.try_acquire(max_streams)?;
        Ok(Arc::clone(entry.value()))
    }
    fn release(&self, token_id: &str, counter: &Arc<TokenCounter>) {
        match self.counters.entry(token_id.to_owned()) {
            Entry::Occupied(entry) if Arc::ptr_eq(entry.get(), counter) => {
                if counter.release() {
                    entry.remove();
                }
            }
            Entry::Occupied(_) | Entry::Vacant(_) => {
                counter.release();
            }
        }
    }
}
fn valid_token_id(token_id: &str) -> bool {
    !token_id.is_empty()
        && token_id.len() <= MAX_TOKEN_ID_BYTES
        && token_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
#[derive(Debug, Default)]
struct TokenCounter {
    active: AtomicU32,
}
impl TokenCounter {
    fn try_acquire(&self, max_streams: u16) -> Result<(), ConcurrencyLimitExceeded> {
        loop {
            let current = self.active.load(Ordering::Relaxed);
            if current >= u32::from(max_streams) {
                return Err(ConcurrencyLimitExceeded);
            }
            if self
                .active
                .compare_exchange(current, current + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
    fn release(&self) -> bool {
        self.active.fetch_sub(1, Ordering::Release) == 1
    }
}
/// Guard that releases a concurrency slot when dropped.
#[derive(Debug)]
pub struct StreamTokenConcurrencyPermit {
    inner: Arc<StreamTokenConcurrencyInner>,
    token_id: String,
    counter: Arc<TokenCounter>,
}
impl Drop for StreamTokenConcurrencyPermit {
    fn drop(&mut self) {
        self.inner.release(&self.token_id, &self.counter);
    }
}
/// Error returned when the token's concurrency budget would be exceeded.
#[derive(Debug, Clone, Copy)]
pub struct ConcurrencyLimitExceeded;
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Arc, Barrier,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    };
    #[test]
    fn zero_limit_and_noncanonical_ids_fail_closed_without_state() {
        let tracker = StreamTokenConcurrencyTracker::default();
        assert!(tracker.try_acquire("token", 0).is_err());
        assert!(tracker.try_acquire("", 1).is_err());
        assert!(tracker.try_acquire("token with spaces", 1).is_err());
        assert!(
            tracker
                .try_acquire(&"x".repeat(MAX_TOKEN_ID_BYTES + 1), 1)
                .is_err()
        );
        assert!(tracker.inner.counters.is_empty());
    }
    #[test]
    fn permits_enforce_limit_and_remove_idle_counter() {
        let tracker = StreamTokenConcurrencyTracker::default();
        let first = tracker
            .try_acquire("token", 2)
            .expect("first permit")
            .expect("bounded permit");
        let second = tracker
            .try_acquire("token", 2)
            .expect("second permit")
            .expect("bounded permit");
        assert!(tracker.try_acquire("token", 2).is_err());
        drop(first);
        assert!(tracker.try_acquire("token", 2).is_ok());
        drop(second);
        drop(tracker.try_acquire("other", 1).expect("permit"));
        assert!(tracker.inner.counters.len() <= 1);
    }
    #[test]
    fn last_release_cannot_split_one_token_across_counters() {
        const THREADS: usize = 8;
        const ITERATIONS: usize = 2_000;
        let tracker = Arc::new(StreamTokenConcurrencyTracker::default());
        let start = Arc::new(Barrier::new(THREADS));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();
        for _ in 0..THREADS {
            let tracker = Arc::clone(&tracker);
            let start = Arc::clone(&start);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            workers.push(thread::spawn(move || {
                start.wait();
                for _ in 0..ITERATIONS {
                    if let Ok(Some(permit)) = tracker.try_acquire("shared-token", 1) {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(current, Ordering::SeqCst);
                        thread::yield_now();
                        active.fetch_sub(1, Ordering::SeqCst);
                        drop(permit);
                    } else {
                        thread::yield_now();
                    }
                }
            }));
        }
        for worker in workers {
            worker.join().expect("worker must not panic");
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
        assert!(tracker.inner.counters.is_empty());
    }
}
