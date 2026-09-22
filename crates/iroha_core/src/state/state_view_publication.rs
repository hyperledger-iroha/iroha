//! Separate State generation completion from notification after physical release.
//!
//! The enclosing operation creates this owner before taking its physical fences.
//! A generation guard only closes the visibility interval; this original owner
//! notifies waiters when the entire operation releases it. No callback runs from
//! a generation guard's destructor, including during unwinding.

use std::sync::atomic::{AtomicU64, Ordering};

/// Original State notification, retained beyond every enclosed publication fence.
pub(super) struct StateViewPublication<'state> {
    generation: &'state AtomicU64,
    publication: &'state tokio::sync::Notify,
    changed: bool,
}

/// One active visibility interval borrowing its original notification owner.
pub(super) struct StateViewGenerationWriteGuard<'scope> {
    generation: &'scope AtomicU64,
    changed: &'scope mut bool,
}

impl<'state> StateViewPublication<'state> {
    /// Retain exact original State controls without allocating or notifying.
    pub(super) fn new(
        generation: &'state AtomicU64,
        publication: &'state tokio::sync::Notify,
    ) -> Self {
        Self {
            generation,
            publication,
            changed: false,
        }
    }

    /// Begin while the caller holds the physical State writer. The borrow keeps
    /// notification alive and prevents overlapping guards from this same owner.
    pub(super) fn begin(&mut self) -> StateViewGenerationWriteGuard<'_> {
        let before = self.generation.load(Ordering::Acquire);
        assert_eq!(before % 2, 0, "State publication generation already active");
        // Refuse exhaustion before changing the generation: an even generation
        // must never wrap into an identity observed by an earlier reader.
        before
            .checked_add(2)
            .expect("State publication generation exhausted");
        self.generation
            .compare_exchange(before, before + 1, Ordering::AcqRel, Ordering::Acquire)
            .expect("State publication requires its original exclusive writer");
        StateViewGenerationWriteGuard {
            generation: self.generation,
            changed: &mut self.changed,
        }
    }
}

impl Drop for StateViewGenerationWriteGuard<'_> {
    fn drop(&mut self) {
        let previous = self.generation.fetch_add(1, Ordering::AcqRel);
        // Mark before the diagnostic: even invariant unwind preserves the
        // original enclosing notification owner, never a callee-local callback.
        *self.changed = true;
        debug_assert_eq!(previous % 2, 1, "State generation must finish from odd");
    }
}

impl Drop for StateViewPublication<'_> {
    fn drop(&mut self) {
        if self.changed {
            self.publication.notify_waiters();
        }
    }
}

#[cfg(test)]
#[path = "state_view_publication_tests.rs"]
mod tests;
