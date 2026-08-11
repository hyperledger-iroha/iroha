//! Allocation regression tests for public-key Norito sizing.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
};

use iroha_crypto::{Algorithm, KeyPair};
use norito::core::NoritoSerialize;

struct CountingAllocator;

thread_local! {
    static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
}
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the caller supplies the `GlobalAlloc` layout contract unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout are forwarded to the allocator that created them.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn mldsa_public_key_sizing_is_allocation_free() {
    let keypair =
        KeyPair::try_from_seed(b"iroha:public-key-sizing:ml-dsa".to_vec(), Algorithm::MlDsa)
            .expect("construct ML-DSA key");
    let public_key = keypair.public_key();

    let expected_exact = public_key
        .encoded_len_exact()
        .expect("ML-DSA public key has an exact wire length");
    let expected_hint = public_key
        .encoded_len_hint()
        .expect("ML-DSA public key has a wire-length hint");
    ALLOCATIONS.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
    let exact = black_box(public_key.encoded_len_exact());
    let hint = black_box(public_key.encoded_len_hint());
    TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));

    assert_eq!(exact, Some(expected_exact));
    assert_eq!(hint, Some(expected_hint));
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
}
