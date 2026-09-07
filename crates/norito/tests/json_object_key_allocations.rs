//! Allocation limits for streamed JSON object-key text.

use norito::json::{self, BoundedJsonError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::BTreeMap,
};

struct TrackingAllocator;

thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static ALLOCATED_BYTES: Cell<usize> = const { Cell::new(0) };
}

fn record_allocation(bytes: usize) {
    if TRACKING.with(Cell::get) {
        ALLOCATIONS.with(|count| count.set(count.get().saturating_add(1)));
        ALLOCATED_BYTES.with(|count| count.set(count.get().saturating_add(bytes)));
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: forward the allocation request unchanged to System.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size());
        // SAFETY: forward the zeroed allocation request unchanged to System.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: pointer and layout belong to the matching System allocation.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation(size);
        // SAFETY: forward the existing allocation and new size unchanged to System.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

fn measured<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    struct StopTracking;
    impl Drop for StopTracking {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }
    ALLOCATIONS.with(|count| count.set(0));
    ALLOCATED_BYTES.with(|count| count.set(0));
    TRACKING.with(|tracking| tracking.set(true));
    let tracking = StopTracking;
    let result = operation();
    drop(tracking);
    (
        result,
        ALLOCATIONS.with(Cell::get),
        ALLOCATED_BYTES.with(Cell::get),
    )
}

#[test]
fn oversized_string_and_hex_keys_fail_before_allocating_output_or_scratch() {
    let text = "x".repeat(32 * 1024);
    let bytes = [0xab_u8; 32 * 1024];
    let text_map = BTreeMap::from([(text.as_str(), 1_u8)]);
    let byte_map = BTreeMap::from([(&bytes, 1_u8)]);
    // Warm the writer and thread-local state outside allocation accounting.
    assert_eq!(
        json::to_json_bounded(&text_map, 8),
        Err(BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(
        json::to_json_bounded(&byte_map, 8),
        Err(BoundedJsonError::BodyTooLarge)
    );

    for (result, count, allocated) in [
        measured(|| json::to_json_bounded(&text_map, 8)),
        measured(|| json::to_json_bounded(&byte_map, 8)),
    ] {
        assert_eq!(result, Err(BoundedJsonError::BodyTooLarge));
        assert_eq!((count, allocated), (0, 0));
    }
}

#[test]
fn successful_hex_key_serialization_allocates_only_the_exact_output() {
    let bytes = [0xab_u8; 4096];
    let map = BTreeMap::from([(&bytes, 1_u8)]);
    let exact = bytes.len() * 2 + "{\"\":1}".len();
    let warm = json::to_json_bounded(&map, exact).unwrap();
    assert_eq!(warm.len(), exact);
    let (result, count, allocated) = measured(|| json::to_json_bounded(&map, exact));
    assert_eq!(result.unwrap(), warm);
    assert_eq!((count, allocated), (1, exact));
}
