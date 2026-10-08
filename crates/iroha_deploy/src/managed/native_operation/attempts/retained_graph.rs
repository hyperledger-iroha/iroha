//! Bounded borrowed revalidation of the original retained enrollment predecessor graph.
//!
//! No memoized result survives a call. Both passes read the same live native custody; local
//! History checks cannot descend recursively and no historical owner or receipt is discarded.
use super::*;

// The sole enrollment body owner admits at most 64 bodies, including this selected body.
// This is a graph traversal bound, separate from its cumulative paid reservation count.
const MAX_RETAINED_BODIES: usize = 64;

pub(super) fn validate(current: &History) -> Result<()> {
    validate_bounded(current, MAX_RETAINED_BODIES)
}

// A standalone body scope reserves one graph slot for its own not-yet-read History.
pub(super) fn validate_predecessor(prior: &VerifiedUnsignedClosure) -> Result<()> {
    validate_bounded(prior.retained_history(), MAX_RETAINED_BODIES - 1)?;
    prior.require_receipt()
}

fn validate_bounded(current: &History, maximum: usize) -> Result<()> {
    let mut predecessors = [None; MAX_RETAINED_BODIES - 1];
    let mut count = 0;
    let mut next = current.scope.predecessor();
    while let Some(prior) = next {
        if count + 1 >= maximum {
            return Err(invalid(
                "retained enrollment history graph exceeds its body bound",
            ));
        }
        let slot = predecessors
            .get_mut(count)
            .ok_or_else(|| invalid("retained enrollment history graph exceeds its body bound"))?;
        *slot = Some(prior);
        count += 1;
        next = prior.retained_history().scope.predecessor();
    }
    // The original local census already authenticates its own before/after state. With no
    // predecessor, preserve that common path without replaying it a second time.
    if count == 0 {
        return current.require_current_local(None);
    }
    // Authenticate all ancestors before the selected node, then recheck in reverse order.
    // References keep every original File/Arc owner live across both complete passes.
    // Each original traversal has its own full snapshot entry/exit fence. The opaque
    // borrowed pass shares only pointer-identical immutable prefixes; local attempt,
    // inventory, receipt and native handle checks still run in their original order.
    current.scope.with_snapshot_read_pass(&mut |pass| {
        for prior in predecessors[..count].iter().rev().flatten() {
            prior.require_retained_local(pass)?;
        }
        current.require_current_local(pass)
    })?;
    current.scope.with_snapshot_read_pass(&mut |pass| {
        current.require_current_local(pass)?;
        for prior in predecessors[..count].iter().flatten() {
            prior.require_retained_local(pass)?;
        }
        Ok(())
    })
}

// Instrument the actual local census only in tests. The fixed scratch records borrowed object
// addresses, not decoded authority or selected paths. It can refuse excess work in a test, but
// cannot manufacture successful validation.
#[cfg(test)]
#[derive(Default)]
pub(in crate::managed) struct TestRetainedHistoryCensus {
    pub(in crate::managed) visits: usize,
    pub(in crate::managed) distinct_histories: usize,
}

#[cfg(test)]
struct VisitCounter {
    maximum: usize,
    census: TestRetainedHistoryCensus,
    histories: [usize; MAX_RETAINED_BODIES],
}

#[cfg(test)]
std::thread_local! {
    static VALIDATION_VISITS: std::cell::RefCell<Option<VisitCounter>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
pub(super) fn record_validation_visit(history: &History) -> Result<()> {
    VALIDATION_VISITS.with(|state| {
        if let Some(counter) = state.borrow_mut().as_mut() {
            counter.census.visits = counter
                .census
                .visits
                .checked_add(1)
                .ok_or_else(|| invalid("test retained-history visit count overflow"))?;
            if counter.census.visits > counter.maximum {
                return Err(invalid(
                    "test retained-history validation exceeded its visit bound",
                ));
            }
            let address = std::ptr::from_ref(history) as usize;
            let count = counter.census.distinct_histories;
            if !counter.histories[..count].contains(&address) {
                *counter.histories.get_mut(count).ok_or_else(|| {
                    invalid("test retained-history graph exceeds its body bound")
                })? = address;
                counter.census.distinct_histories += 1;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
impl History {
    // Counts actual local validation calls, including accidental nested full-walker reentry.
    // Only a test-local observer is installed; no successful custody result is manufactured.
    pub(in crate::managed) fn test_require_current(
        &self,
        maximum_visits: usize,
    ) -> (Result<()>, TestRetainedHistoryCensus) {
        struct Restore(Option<VisitCounter>);
        impl Drop for Restore {
            fn drop(&mut self) {
                VALIDATION_VISITS.with(|state| {
                    state.replace(self.0.take());
                });
            }
        }
        let _restore = Restore(VALIDATION_VISITS.with(|state| {
            state.replace(Some(VisitCounter {
                maximum: maximum_visits,
                census: TestRetainedHistoryCensus::default(),
                histories: [0; MAX_RETAINED_BODIES],
            }))
        }));
        let result = self.require_current();
        let census =
            VALIDATION_VISITS.with(|state| state.take().expect("test observer retained").census);
        (result, census)
    }
}
