//! Finite capacity, prepaid splitting and scoped reclamation notifications.

use iroha_allocation::AllocationRefusal;
use std::{
    alloc::Layout,
    cell::Cell,
    sync::{Arc, atomic::AtomicUsize},
};
use std::{
    alloc::{GlobalAlloc, System},
    future::Future,
    pin::Pin,
    sync::atomic::Ordering::SeqCst,
    task::{Context, Poll, Wake, Waker},
};

thread_local! {
    static ALLOCATION_COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

struct ObservedAllocator;

fn record_allocation() {
    let _ = ALLOCATION_COUNT.try_with(|count| {
        if let Some(previous) = count.get() {
            count.set(Some(previous.saturating_add(1)));
        }
    });
}

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: forward the unchanged request to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: preserve the original zeroing and layout contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: preserve the live allocation, old layout and requested size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forward the original allocation and its exact layout.
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

pub(crate) fn without_allocations<R>(operation: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_COUNT.with(|count| count.set(None));
        }
    }
    assert!(
        ALLOCATION_COUNT
            .with(|count| count.replace(Some(0)))
            .is_none()
    );
    let reset = Reset;
    let output = operation();
    let allocations = ALLOCATION_COUNT.with(|count| count.replace(None)).unwrap();
    drop(reset);
    assert_eq!(allocations, 0, "lexical refund handling allocated storage");
    output
}

#[derive(Default)]
pub(crate) struct WakeCount(pub(crate) AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, SeqCst);
    }
}

pub(crate) fn poll(
    wait: &mut iroha_allocation::release::ReleaseFuture<'_>,
    wakes: &Arc<WakeCount>,
) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::clone(wakes))))
}

pub(crate) fn capacity_wait(
    error: AllocationRefusal,
    registration: &mut iroha_allocation::release::ReleaseRegistration,
) -> iroha_allocation::release::ReleaseFuture<'_> {
    let AllocationRefusal::Capacity { release, .. } = error else {
        panic!("expected temporary capacity refusal: {error}");
    };
    release.wait_for_release(registration)
}

pub(crate) fn layout(size: usize) -> Layout {
    Layout::from_size_align(size, 1).unwrap()
}
