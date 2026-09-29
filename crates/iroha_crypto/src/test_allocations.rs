//! Shared test-only allocator observation for cryptographic fixed owners.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct ObservedAllocator;

thread_local! {
    pub(crate) static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
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
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
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
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

pub(crate) fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
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
    assert_eq!(ALLOCATIONS.with(Cell::get), 0, "uncached core allocated");
    result
}

#[test]
fn allocation_observer_detects_backing_and_retires_during_unwind() {
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
