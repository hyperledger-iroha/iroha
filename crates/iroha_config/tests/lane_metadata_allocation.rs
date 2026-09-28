//! Actual allocator observation for borrowed lane catalog validation.

use iroha_config::parameters::actual::LaneConfig;
use iroha_data_model::nexus::{LaneCatalog, LaneConfig as Metadata};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    num::NonZeroU32,
};

thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}
struct Allocator;
fn record_allocation() {
    let _ = COUNT.try_with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: unchanged allocation request is forwarded to the system.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: unchanged live pointer and layout are returned to the system.
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: unchanged live allocation and requested size are forwarded.
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

struct Observation;
impl Drop for Observation {
    fn drop(&mut self) {
        COUNT.with(|count| count.set(None));
    }
}

#[test]
fn canonical_and_rejected_metadata_checks_allocate_nothing() {
    let metadata = Metadata {
        alias: "__Long Mixed 東京 Alias__".into(),
        ..Metadata::default()
    };
    let catalog = LaneCatalog::new(NonZeroU32::MIN, vec![metadata.clone()]).unwrap();
    let config = LaneConfig::from_catalog(&catalog);
    let entry = config.primary();
    let mut drifted = entry.clone();
    drifted.merge_segment.push('x');
    // Initialize thread-local observation before counting. Assertions that can
    // allocate diagnostics run only after observation has stopped.
    COUNT.with(|count| count.set(Some(0)));
    let scope = Observation;
    let mut valid = true;
    for _ in 0..128 {
        valid &= std::hint::black_box(entry).matches_metadata(std::hint::black_box(&metadata));
        valid &= !std::hint::black_box(&drifted).matches_metadata(std::hint::black_box(&metadata));
    }
    let allocations = COUNT.with(|count| count.get().unwrap());
    drop(scope);
    assert!(valid);
    assert_eq!(
        allocations, 0,
        "catalog validation may not clone or render heap data"
    );
}
