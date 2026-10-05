//! Fixed thread-local census of every allocation request in the measured parameter scope.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct Observation {
    active: bool,
    pub(super) requests: [usize; 3],
    pub(super) bytes: usize,
}
impl Observation {
    const fn new() -> Self {
        Self {
            active: false,
            requests: [0; 3],
            bytes: 0,
        }
    }
}
thread_local! { static OBSERVED: Cell<Observation> = const { Cell::new(Observation::new()) }; }
fn request(kind: usize, bytes: usize) {
    let _ = OBSERVED.try_with(|cell| {
        let mut observed = cell.get();
        if observed.active {
            observed.requests[kind] = observed.requests[kind].saturating_add(1);
            observed.bytes = observed.bytes.saturating_add(bytes);
            cell.set(observed);
        }
    });
}
struct Allocator;
#[allow(unsafe_code)]
// SAFETY: every original request is delegated unchanged to the System allocator.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        request(0, layout.size());
        // SAFETY: unchanged original request.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        request(1, layout.size());
        // SAFETY: unchanged original request.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        request(2, size);
        // SAFETY: original live allocation and requested size are forwarded.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: original allocation is returned to its allocator unchanged.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        let _ = OBSERVED.try_with(|cell| {
            let mut value = cell.get();
            value.active = false;
            cell.set(value);
        });
    }
}
pub(super) fn measured<T>(operation: impl FnOnce() -> T) -> (T, Observation) {
    OBSERVED.with(|cell| {
        assert!(!cell.get().active);
        cell.set(Observation {
            active: true,
            ..Observation::new()
        });
    });
    let stop = Stop;
    let result = operation();
    let observed = OBSERVED.with(Cell::get);
    drop(stop);
    (result, observed)
}
