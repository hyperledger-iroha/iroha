//! Shared thread-local allocation census for exact Core ownership controls.
//!
//! Production builds use no instrumentation. Each observation disables itself
//! during unwinding; other test threads remain outside its counter.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn record_allocation() {
    let _ = TRACK_ALLOCATIONS.try_with(|tracking| {
        if tracking.get() {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
    });
}

// SAFETY: all operations delegate to System with the original allocation layout.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation();
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation();
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            record_allocation();
        }
        result
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

/// Count successful heap allocations on this thread only, including reallocations.
///
/// # Panics
/// Rejects nested observation before changing the active counter or tracking guard.
pub(crate) fn allocations_during(f: impl FnOnce()) -> usize {
    assert!(
        !TRACK_ALLOCATIONS.with(Cell::get),
        "nested allocation census would invalidate the outer observation"
    );
    ALLOCATION_COUNT.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    struct StopTracking;
    impl Drop for StopTracking {
        fn drop(&mut self) {
            TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        }
    }
    let stop = StopTracking;
    f();
    drop(stop);
    ALLOCATION_COUNT.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        hint::black_box,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn allocation_census_has_positive_and_zero_controls() {
        assert_eq!(allocations_during(|| {}), 0);
        assert_eq!(
            allocations_during(|| drop(black_box(Box::new([42_u8; 64])))),
            1
        );
        assert!(!TRACK_ALLOCATIONS.with(Cell::get));
    }

    #[test]
    fn allocation_census_clears_tracking_on_unwind() {
        let result = std::panic::catch_unwind(|| allocations_during(|| panic!("census unwind")));
        assert!(result.is_err());
        assert!(!TRACK_ALLOCATIONS.with(Cell::get));
        assert_eq!(allocations_during(|| {}), 0);
    }

    #[test]
    fn nested_allocation_census_cannot_reset_or_disable_outer_observation() {
        let nested_ran = Cell::new(false);
        let mut nested_result = None;
        let mut before = 0;
        let mut after = 0;
        let mut still_tracking = false;
        let count = allocations_during(|| {
            drop(black_box(Box::new([3_u8; 64])));
            before = ALLOCATION_COUNT.with(Cell::get);
            nested_result = Some(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || allocations_during(|| nested_ran.set(true)),
            )));
            after = ALLOCATION_COUNT.with(Cell::get);
            still_tracking = TRACK_ALLOCATIONS.with(Cell::get);
            drop(black_box(Box::new([4_u8; 64])));
        });
        assert!(nested_result.unwrap().is_err());
        assert!(!nested_ran.get());
        assert_eq!(before, 1);
        assert!(
            after >= before,
            "panic custody may allocate; the outer count cannot decrease"
        );
        assert!(still_tracking);
        assert_eq!(count, after + 1);
        assert!(!TRACK_ALLOCATIONS.with(Cell::get));
    }

    #[test]
    fn allocation_census_excludes_other_threads() {
        let ready = AtomicBool::new(false);
        let start = AtomicBool::new(false);
        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                ready.store(true, Ordering::Release);
                while !start.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                for _ in 0..16 {
                    drop(black_box(Box::new([1_u8; 128])));
                }
                done.store(true, Ordering::Release);
            });
            while !ready.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            let allocations = allocations_during(|| {
                start.store(true, Ordering::Release);
                while !done.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            });
            worker.join().unwrap();
            assert_eq!(allocations, 0);
        });
    }
}
