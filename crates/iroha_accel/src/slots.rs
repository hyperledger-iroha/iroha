//! Cardinality admission for opaque native owners, separate from byte budgets.

use parking_lot::RwLock;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// One original finite cardinality owner; live permits survive a limit decrease.
#[derive(Debug)]
pub(crate) struct Slots {
    limit: RwLock<usize>,
    used: AtomicUsize,
    peak: AtomicUsize,
}

impl Slots {
    pub(crate) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit: RwLock::new(limit),
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        })
    }

    pub(crate) fn set_limit(&self, limit: usize) {
        *self.limit.write() = limit;
    }

    pub(crate) fn try_acquire(self: &Arc<Self>) -> Option<Slot> {
        let limit = self.limit.try_read()?;
        let previous = self
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                (used < *limit).then(|| used + 1)
            })
            .ok()?;
        self.peak.fetch_max(previous + 1, Ordering::Relaxed);
        Some(Slot {
            owner: Arc::clone(self),
        })
    }

    pub(crate) fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }
    pub(crate) fn peak(&self) -> usize {
        self.peak.load(Ordering::Acquire)
    }
}

/// Move-only permission retained until the actual native owner is reclaimed.
pub(crate) struct Slot {
    owner: Arc<Slots>,
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.owner.used.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrink_does_not_forgive_original_live_owners() {
        let slots = Slots::new(2);
        let first = slots.try_acquire().unwrap();
        let second = slots.try_acquire().unwrap();
        slots.set_limit(1);
        assert_eq!(slots.used(), 2);
        assert!(slots.try_acquire().is_none());
        drop(first);
        assert!(slots.try_acquire().is_none());
        drop(second);
        assert!(slots.try_acquire().is_some());
        assert_eq!(slots.peak(), 2);
    }

    #[test]
    fn admission_refuses_while_configuration_is_being_written() {
        let slots = Slots::new(1);
        let _writer = slots.limit.write();
        assert!(slots.try_acquire().is_none());
        assert_eq!(slots.used(), 0);
    }

    #[test]
    fn zero_refuses_and_growth_uses_the_same_owner() {
        let slots = Slots::new(0);
        assert!(slots.try_acquire().is_none());
        slots.set_limit(1);
        let permit = slots.try_acquire().unwrap();
        let borrower = Arc::clone(&slots);
        drop(slots);
        assert_eq!(borrower.used(), 1);
        drop(permit);
        assert_eq!(borrower.used(), 0);
    }
}
