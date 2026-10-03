//! Allocation-free census of simultaneously live ICU and stable-sort buffers.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

#[derive(Clone, Copy, Default)]
struct Live {
    pointer: usize,
    bytes: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Observation {
    active: bool,
    pub(super) requests: [usize; 64],
    pub(super) count: usize,
    pub(super) bytes: usize,
    live: [Live; 4],
    pub(super) live_bytes: usize,
    pub(super) peak: usize,
    pub(super) max_live: usize,
    pub(super) invalid: bool,
}
impl Observation {
    const fn new() -> Self {
        Self {
            active: false,
            requests: [0; 64],
            count: 0,
            bytes: 0,
            live: [Live {
                pointer: 0,
                bytes: 0,
            }; 4],
            live_bytes: 0,
            peak: 0,
            max_live: 0,
            invalid: false,
        }
    }
    pub(super) fn live_allocations(&self) -> usize {
        self.live.iter().filter(|entry| entry.pointer != 0).count()
    }
    fn request(&mut self, pointer: *mut u8, bytes: usize, previous: Option<(*mut u8, usize)>) {
        if !self.active {
            return;
        }
        if let Some(slot) = self.requests.get_mut(self.count) {
            *slot = bytes;
        } else {
            self.invalid = true;
        }
        self.count += 1;
        self.bytes += bytes;
        // Even an in-place realloc must be covered as if old/new storage overlap.
        self.peak = self.peak.max(self.live_bytes + bytes);
        if let Some((previous, old_size)) = previous {
            self.release(previous, old_size);
        }
        if let Some(slot) = self.live.iter_mut().find(|entry| entry.pointer == 0) {
            *slot = Live {
                pointer: pointer as usize,
                bytes,
            };
            self.live_bytes += bytes;
        } else {
            self.invalid = true;
        }
        self.max_live = self.max_live.max(self.live_allocations());
    }
    fn release(&mut self, pointer: *mut u8, bytes: usize) {
        if !self.active {
            return;
        }
        if let Some(slot) = self
            .live
            .iter_mut()
            .find(|entry| entry.pointer == pointer as usize)
        {
            self.invalid |= slot.bytes != bytes;
            self.live_bytes -= slot.bytes;
            *slot = Live::default();
        } else {
            self.invalid = true;
        }
    }
}

thread_local! {
    static OBSERVED: Cell<Observation> = const { Cell::new(Observation::new()) };
}
fn observe(operation: impl FnOnce(&mut Observation)) {
    let _ = OBSERVED.try_with(|cell| {
        let mut value = cell.get();
        operation(&mut value);
        cell.set(value);
    });
}
struct Allocator;
#[allow(unsafe_code)]
// SAFETY: each original allocation request is delegated unchanged to System.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: unchanged system allocation request.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            observe(|state| state.request(pointer, layout.size(), None));
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: unchanged system allocation request.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            observe(|state| state.request(pointer, layout.size(), None));
        }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
        // SAFETY: the original live allocation and requested size are forwarded.
        let result = unsafe { System.realloc(pointer, layout, bytes) };
        if !result.is_null() {
            observe(|state| state.request(result, bytes, Some((pointer, layout.size()))));
        }
        result
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        observe(|state| state.release(pointer, layout.size()));
        // SAFETY: the original allocation is returned to its allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        observe(|state| state.active = false);
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
