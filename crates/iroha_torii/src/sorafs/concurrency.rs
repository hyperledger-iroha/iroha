//! Concurrency tracking for SoraFS stream tokens.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use dashmap::DashMap;

/// Tracks active stream requests per token to enforce concurrency budgets.
#[derive(Clone, Default)]
pub struct StreamTokenConcurrencyTracker {
    inner: Arc<StreamTokenConcurrencyInner>,
}

impl StreamTokenConcurrencyTracker {
    /// Attempt to acquire a concurrency slot for `token_id`.
    ///
    /// Returns `Ok(None)` when the limit is `0` (unlimited), or a guard that releases the slot
    /// when dropped. Returns [`ConcurrencyLimitExceeded`] if the limit would be exceeded.
    ///
    /// # Errors
    ///
    /// Returns [`ConcurrencyLimitExceeded`] when `max_streams` would be exceeded for `token_id`.
    pub fn try_acquire(
        &self,
        token_id: &str,
        max_streams: u16,
    ) -> Result<Option<StreamTokenConcurrencyPermit>, ConcurrencyLimitExceeded> {
        if max_streams == 0 {
            return Ok(None);
        }

        let inner = Arc::clone(&self.inner);
        let counter = inner.counter_for(token_id);
        counter.try_acquire(max_streams)?;
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
    fn counter_for(&self, token_id: &str) -> Arc<TokenCounter> {
        self.counters
            .entry(token_id.to_owned())
            .or_insert_with(|| Arc::new(TokenCounter::default()))
            .clone()
    }

    fn release(&self, token_id: &str, counter: &Arc<TokenCounter>) {
        if counter.release() {
            self.counters
                .remove_if(token_id, |_, value| Arc::ptr_eq(value, counter));
        }
    }
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
