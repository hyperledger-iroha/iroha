use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

use iroha_schema::IntoSchema;
use norito::core::*;

struct CountingAlloc;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

#[derive(IntoSchema, NoritoSerialize)]
#[repr(C)]
struct Named {
    x: u32,
    y: u64,
}

#[derive(IntoSchema, NoritoSerialize)]
#[repr(C)]
struct Tuple(u32, u64);

#[test]
fn named_struct_no_extra_allocations() {
    // Warm up any one-time allocations (e.g., tables) outside measurement
    let _ = to_bytes(&0u8).unwrap();
    // Warm serialization path to initialize any one-time internals
    let _ = {
        let mut tmp = Vec::with_capacity(64);
        let warm = Named { x: 0, y: 0 };
        norito::core::NoritoSerialize::serialize(&warm, &mut tmp).unwrap();
        tmp
    };
    ALLOCATIONS.store(0, Ordering::SeqCst);
    let value = Named { x: 1, y: 2 };
    let mut buf = Vec::with_capacity(128);
    norito::core::NoritoSerialize::serialize(&value, &mut buf).unwrap();
    assert!(
        ALLOCATIONS.load(Ordering::SeqCst) <= 4,
        "unexpected allocations: {}",
        ALLOCATIONS.load(Ordering::SeqCst)
    );
}

#[test]
fn tuple_struct_no_extra_allocations() {
    let _ = to_bytes(&0u8).unwrap();
    let _ = {
        let mut tmp = Vec::with_capacity(64);
        let warm = Tuple(0, 0);
        norito::core::NoritoSerialize::serialize(&warm, &mut tmp).unwrap();
        tmp
    };
    ALLOCATIONS.store(0, Ordering::SeqCst);
    let value = Tuple(1, 2);
    let mut buf = Vec::with_capacity(128);
    norito::core::NoritoSerialize::serialize(&value, &mut buf).unwrap();
    assert!(
        ALLOCATIONS.load(Ordering::SeqCst) <= 4,
        "unexpected allocations: {}",
        ALLOCATIONS.load(Ordering::SeqCst)
    );
}
