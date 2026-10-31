//! Bounded, allocation-free instrumentation of live test-thread heap owners.
//!
//! The pointer table is allocated before observation. Only allocations born
//! during the closure are counted; pre-existing allocations and other threads
//! are excluded. These are allocator-visible requested sizes, not RSS or the
//! allocator's internal bookkeeping. Reallocation's possible transient overlap
//! has a separate conservative bound because `GlobalAlloc` cannot observe it.

use std::{cell::Cell, ptr};

const EMPTY: usize = 0;
const DELETED: usize = usize::MAX;

thread_local! {
    static ACTIVE: Cell<*mut Census> = const { Cell::new(ptr::null_mut()) };
}

/// Actual allocation lifetimes observed on the selected test thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveAllocationStats {
    /// Number of successful observed allocation/reallocation events.
    pub events: usize,
    /// Sum of requested sizes over events, explicitly cumulative rather than live.
    pub cumulative_bytes: usize,
    /// Allocations still owned when observation ends.
    pub retained_allocations: usize,
    /// Requested bytes still owned when observation ends.
    pub retained_bytes: usize,
    /// Maximum simultaneous allocator-visible requested bytes.
    pub peak_live_bytes: usize,
    /// Conservative maximum allowing old plus new storage during reallocation.
    pub peak_with_realloc_overlap_bytes: usize,
    /// True if the fixed table or arithmetic could not represent the observation.
    pub incomplete: bool,
}

#[derive(Clone, Copy, Default)]
struct Entry {
    address: usize,
    bytes: usize,
}

struct Census {
    entries: Box<[Entry]>,
    stats: LiveAllocationStats,
}

impl Census {
    fn index(&self, address: usize) -> usize {
        (address >> 3).wrapping_mul(0x9e37_79b9) & (self.entries.len() - 1)
    }

    fn take(&mut self, address: usize) -> Option<usize> {
        let initial = self.index(address);
        for offset in 0..self.entries.len() {
            let index = (initial + offset) & (self.entries.len() - 1);
            let entry = &mut self.entries[index];
            if entry.address == EMPTY {
                return None;
            }
            if entry.address == address {
                let bytes = entry.bytes;
                entry.address = DELETED;
                self.stats.retained_allocations -= 1;
                self.stats.retained_bytes -= bytes;
                return Some(bytes);
            }
        }
        None
    }

    fn add(&mut self, address: usize, bytes: usize) {
        if address == EMPTY || address == DELETED {
            self.stats.incomplete = true;
            return;
        }
        let initial = self.index(address);
        let mut vacant = None;
        for offset in 0..self.entries.len() {
            let index = (initial + offset) & (self.entries.len() - 1);
            let entry = self.entries[index];
            if entry.address == address {
                self.stats.incomplete = true;
                return;
            }
            if entry.address == DELETED && vacant.is_none() {
                vacant = Some(index);
            }
            if entry.address == EMPTY {
                vacant = vacant.or(Some(index));
                break;
            }
        }
        let Some(index) = vacant else {
            self.stats.incomplete = true;
            return;
        };
        let Some(retained) = self.stats.retained_bytes.checked_add(bytes) else {
            self.stats.incomplete = true;
            return;
        };
        self.entries[index] = Entry { address, bytes };
        self.stats.retained_allocations += 1;
        self.stats.retained_bytes = retained;
        self.stats.peak_live_bytes = self.stats.peak_live_bytes.max(retained);
        self.stats.peak_with_realloc_overlap_bytes =
            self.stats.peak_with_realloc_overlap_bytes.max(retained);
        self.stats.events = self.stats.events.saturating_add(1);
        match self.stats.cumulative_bytes.checked_add(bytes) {
            Some(total) => self.stats.cumulative_bytes = total,
            None => self.stats.incomplete = true,
        }
    }
}

fn with_active(work: impl FnOnce(&mut Census)) {
    let _ = ACTIVE.try_with(|active| {
        let pointer = active.get();
        if !pointer.is_null() {
            // SAFETY: the synchronous observation owns this stack value until
            // its reset guard clears ACTIVE. Hooks allocate nothing and cannot
            // re-enter. TLS excludes every other thread.
            work(unsafe { &mut *pointer });
        }
    });
}

pub(super) fn allocated(pointer: *mut u8, bytes: usize) {
    with_active(|census| census.add(pointer as usize, bytes));
}

pub(super) fn deallocated(pointer: *mut u8) {
    with_active(|census| {
        census.take(pointer as usize);
    });
}

pub(super) fn reallocated(original: *mut u8, replacement: *mut u8, bytes: usize) {
    with_active(|census| {
        let before = census.stats.retained_bytes;
        // A realloc of pre-existing storage stays outside this observation.
        if census.take(original as usize).is_some() {
            match before.checked_add(bytes) {
                Some(overlap) => {
                    census.stats.peak_with_realloc_overlap_bytes =
                        census.stats.peak_with_realloc_overlap_bytes.max(overlap);
                }
                None => census.stats.incomplete = true,
            }
            census.add(replacement as usize, bytes);
        }
    });
}

/// Measure live requested heap storage created inside a synchronous closure.
///
/// The fixed pointer table is excluded from the measurement. An exhausted table
/// explicitly marks the result incomplete; no growth occurs inside allocator
/// hooks. The result may itself retain measured allocations.
///
/// # Panics
/// Rejects nested observation and capacities that are not nonzero powers of two.
pub fn live_allocations_during<T>(
    capacity: usize,
    work: impl FnOnce() -> T,
) -> (T, LiveAllocationStats) {
    assert!(
        capacity.is_power_of_two(),
        "live census capacity must be a power of two"
    );
    assert!(
        ACTIVE.with(Cell::get).is_null(),
        "live allocation observations cannot nest"
    );
    let mut census = Census {
        entries: vec![Entry::default(); capacity].into_boxed_slice(),
        stats: LiveAllocationStats::default(),
    };
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(ptr::null_mut()));
        }
    }
    ACTIVE.with(|active| active.set(&raw mut census));
    let reset = Reset;
    let result = work();
    drop(reset);
    (result, census.stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{Layout, alloc, alloc_zeroed, dealloc, realloc};

    #[test]
    fn live_census_distinguishes_peak_retained_and_cumulative_bytes() {
        let layout = Layout::from_size_align(37, 1).unwrap();
        let (retained, stats) = live_allocations_during(16, || unsafe {
            let first = alloc(layout);
            let second = alloc_zeroed(layout);
            assert!(!first.is_null() && !second.is_null());
            dealloc(first, layout);
            let third = alloc(layout);
            assert!(!third.is_null());
            dealloc(third, layout);
            second
        });
        assert_eq!(stats.events, 3);
        assert_eq!(stats.cumulative_bytes, 111);
        assert_eq!(stats.retained_allocations, 1);
        assert_eq!(stats.retained_bytes, 37);
        assert_eq!(stats.peak_live_bytes, 74);
        assert_eq!(stats.peak_with_realloc_overlap_bytes, 74);
        assert!(!stats.incomplete);
        unsafe { dealloc(retained, layout) };
    }

    #[test]
    fn live_census_reallocation_tracks_original_lifetime_and_possible_overlap() {
        let original = Layout::from_size_align(19, 1).unwrap();
        let grown = Layout::from_size_align(37, 1).unwrap();
        let ((), stats) = live_allocations_during(16, || unsafe {
            let pointer = alloc(original);
            assert!(!pointer.is_null());
            let replacement = realloc(pointer, original, grown.size());
            assert!(!replacement.is_null());
            dealloc(replacement, grown);
        });
        assert_eq!(stats.events, 2);
        assert_eq!(stats.cumulative_bytes, 56);
        assert_eq!(stats.retained_bytes, 0);
        assert_eq!(stats.peak_live_bytes, 37);
        assert_eq!(stats.peak_with_realloc_overlap_bytes, 56);
        assert!(!stats.incomplete);
    }

    #[test]
    fn live_census_failed_reallocation_preserves_original_and_excludes_preexisting() {
        let original = Layout::from_size_align(19, 1).unwrap();
        let grown = Layout::from_size_align(37, 1).unwrap();
        let preexisting = unsafe { alloc(original) };
        assert!(!preexisting.is_null());
        let ((), stats) = live_allocations_during(16, || unsafe {
            let preexisting = realloc(preexisting, original, grown.size());
            assert!(!preexisting.is_null());
            dealloc(preexisting, grown);
            let pointer = alloc(original);
            assert!(!pointer.is_null());
            let (replacement, refused) = super::super::refuse_one_layout_during(grown, || {
                realloc(pointer, original, grown.size())
            });
            assert!(refused && replacement.is_null());
            dealloc(pointer, original);
        });
        assert_eq!(stats.events, 1);
        assert_eq!(stats.cumulative_bytes, 19);
        assert_eq!(stats.retained_bytes, 0);
        assert_eq!(stats.peak_live_bytes, 19);
        assert!(!stats.incomplete);
    }

    #[test]
    fn live_census_capacity_exhaustion_is_explicit_and_does_not_grow() {
        let layout = Layout::from_size_align(19, 1).unwrap();
        let ((), stats) = live_allocations_during(1, || unsafe {
            let first = alloc(layout);
            let second = alloc(layout);
            assert!(!first.is_null() && !second.is_null());
            dealloc(first, layout);
            dealloc(second, layout);
        });
        assert!(stats.incomplete);
        assert_eq!(stats.retained_bytes, 0);
    }

    #[test]
    fn live_census_resets_after_unwind_and_refuses_nested_observation() {
        let failed = std::panic::catch_unwind(|| {
            live_allocations_during(16, || live_allocations_during(16, || ()))
        });
        assert!(failed.is_err());
        assert!(ACTIVE.with(Cell::get).is_null());
        let ((), stats) = live_allocations_during(16, || ());
        assert_eq!(stats, LiveAllocationStats::default());
    }

    #[test]
    fn live_census_excludes_other_threads() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let ready = AtomicBool::new(false);
        let start = AtomicBool::new(false);
        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                ready.store(true, Ordering::Release);
                while !start.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                let layout = Layout::from_size_align(37, 1).unwrap();
                unsafe {
                    let pointer = alloc(layout);
                    assert!(!pointer.is_null());
                    dealloc(pointer, layout);
                }
                done.store(true, Ordering::Release);
            });
            while !ready.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            let ((), stats) = live_allocations_during(16, || {
                start.store(true, Ordering::Release);
                while !done.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            });
            worker.join().unwrap();
            assert_eq!(stats, LiveAllocationStats::default());
        });
    }
}
