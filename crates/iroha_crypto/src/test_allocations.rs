//! Shared test-only allocator observation for cryptographic fixed owners.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct ObservedAllocator;

thread_local! {
    pub(crate) static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static REFUSE_SIZE: Cell<Option<usize>> = const { Cell::new(None) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATION_SIZE: Cell<Option<usize>> = const { Cell::new(None) };
    static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record_allocation() {
    if OBSERVE.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

// SAFETY: this observer forwards each request and original layout to System;
// it changes no pointer, allocation size, alignment, or deallocation behavior.
#[allow(unsafe_code)] // Required only to forward the allocator's original pointer/layout contract.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        if REFUSE_SIZE.try_with(Cell::get).unwrap_or(None) == Some(layout.size()) {
            return core::ptr::null_mut();
        }
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        if REFUSE_SIZE.try_with(Cell::get).unwrap_or(None) == Some(layout.size()) {
            return core::ptr::null_mut();
        }
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.dealloc(ptr, layout) }
        if DEALLOCATION_SIZE.try_with(Cell::get).unwrap_or(None) == Some(layout.size()) {
            let _ = DEALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

/// Return the actual allocation count for one operation on this test thread.
/// The observer is always retired during unwinding and observations cannot nest.
pub fn allocations_during<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct EndObservation;
    impl Drop for EndObservation {
        fn drop(&mut self) {
            OBSERVE.with(|active| active.set(false));
        }
    }
    assert!(
        !OBSERVE.with(Cell::get),
        "allocation observations cannot nest"
    );
    ALLOCATIONS.with(|count| count.set(0));
    OBSERVE.with(|active| active.set(true));
    let guard = EndObservation;
    let result = operation();
    drop(guard);
    (result, ALLOCATIONS.with(Cell::get))
}

pub fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    let (result, allocations) = allocations_during(operation);
    assert_eq!(allocations, 0, "uncached core allocated");
    result
}

#[test]
fn allocation_observer_detects_backing_and_retires_during_unwind() {
    let (backing, count) = allocations_during(|| std::hint::black_box(vec![0x5a_u8; 73]));
    assert_eq!(
        count, 1,
        "the shared counter observes a real original allocation"
    );
    drop(backing);
    let observed = std::panic::catch_unwind(|| {
        without_allocations(|| {
            std::hint::black_box(vec![0x5a_u8; 73]);
        });
    });
    assert!(observed.is_err(), "a real Vec allocation must be detected");
    assert!(!OBSERVE.with(Cell::get));
    let unwind = std::panic::catch_unwind(|| {
        without_allocations(|| panic!("unwind during the observed operation"));
    });
    assert!(unwind.is_err());
    assert!(!OBSERVE.with(Cell::get));
    without_allocations(|| ());
}

/// Observe completed deallocations of one exact layout size on this test thread.
pub fn with_deallocation_observation<T>(bytes: usize, body: impl FnOnce() -> T) -> (T, usize) {
    struct EndObservation;
    impl Drop for EndObservation {
        fn drop(&mut self) {
            DEALLOCATION_SIZE.with(|size| size.set(None));
        }
    }
    assert!(DEALLOCATION_SIZE.with(Cell::get).is_none());
    DEALLOCATIONS.with(|count| count.set(0));
    DEALLOCATION_SIZE.with(|size| size.set(Some(bytes)));
    let guard = EndObservation;
    let result = body();
    drop(guard);
    (result, observed_deallocations())
}

/// Completed matching deallocations, including while a custody token is dropping.
pub fn observed_deallocations() -> usize {
    DEALLOCATIONS.with(Cell::get)
}

#[test]
fn deallocation_observer_counts_completed_exact_size_release() {
    let ((), count) = with_deallocation_observation(97, || {
        let observed = vec![0x71_u8; 97];
        let other = vec![0x72_u8; 98];
        std::hint::black_box((&observed, &other));
        assert_eq!(observed_deallocations(), 0);
        drop(other);
        assert_eq!(observed_deallocations(), 0);
        drop(observed);
        assert_eq!(observed_deallocations(), 1);
    });
    assert_eq!(count, 1);
    let unwind = std::panic::catch_unwind(|| {
        with_deallocation_observation(97, || panic!("deallocation observation unwind"));
    });
    assert!(unwind.is_err());
    assert!(DEALLOCATION_SIZE.with(Cell::get).is_none());
}

/// Refuse exact-size physical requests on this thread without changing admission.
/// The caller must construct all unrelated test owners before entering this scope.
pub fn with_allocation_failure<T>(bytes: usize, body: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            REFUSE_SIZE.with(|size| size.set(None));
        }
    }
    assert!(REFUSE_SIZE.with(Cell::get).is_none());
    REFUSE_SIZE.with(|size| size.set(Some(bytes)));
    let guard = Restore;
    let value = body();
    drop(guard);
    value
}
