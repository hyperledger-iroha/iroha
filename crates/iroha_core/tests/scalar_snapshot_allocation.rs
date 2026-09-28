//! Physical allocation observation for the production scalar snapshot decoder.

#[path = "../src/state/scalar_cell_custody_snapshot.rs"]
mod snapshot;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
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
        // SAFETY: unchanged allocation request goes to the system allocator.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the original pointer and layout return to the same allocator.
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
fn exact_both_cut_decode_and_canonical_refusals_allocate_nothing() {
    let cases = [
        (r#"{"revert":null,"blocks":0}"#, Some((0, None))),
        (r#"{"revert":6,"blocks":9}"#, Some((9, Some(6)))),
        (
            r#"{"revert":18446744073709551615,"blocks":18446744073709551615}"#,
            Some((u64::MAX, Some(u64::MAX))),
        ),
        (r#"{"revert":6,"blocks":9,"blocks":9}"#, None),
        (r#"{"revert":6,"blocks":18446744073709551616}"#, None),
        (r#"{"revert":6,"blocks":09}"#, None),
        (r#"{"revert":6,"blocks": 9}"#, None),
        (r#"{"revert":6,"blocks":9} "#, None),
    ];
    COUNT.with(|count| count.set(Some(0)));
    let observation = Observation;
    let mut exact = true;
    for _ in 0..128 {
        for (source, expected) in cases {
            exact &= snapshot::decode_snapshot(std::hint::black_box(source)).ok() == expected;
        }
    }
    let allocations = COUNT.with(|count| count.get().unwrap());
    drop(observation);
    assert!(exact);
    assert_eq!(
        allocations, 0,
        "scalar parsing cannot allocate a comparison buffer"
    );
}
