//! Thread-local allocation observer; every request is forwarded unchanged to System.
#![allow(unsafe_code)]
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct ObservedAllocator;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;
fn record() {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = COUNT.try_with(|count| count.set(count.get().saturating_add(1)));
        }
    });
}
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: forward the original allocator request unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: forward the original allocator request unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        // SAFETY: forward the caller's original allocation and new size unchanged.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forward the caller's matching pointer/layout unchanged.
        unsafe { System.dealloc(pointer, layout) }
    }
}
fn measured<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ENABLED.with(|enabled| enabled.set(false));
        }
    }
    ENABLED.with(|enabled| assert!(!enabled.get(), "nested measurement"));
    COUNT.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let reset = Reset;
    let value = operation();
    drop(reset);
    (value, COUNT.with(Cell::get))
}
#[test]
fn observer_counts_all_three_allocation_routes_and_resets_after_unwind() {
    let ((), count) = measured(|| {
        // SAFETY: every successful exact-layout allocation is deallocated with
        // its matching final layout; failed reallocation retains the old pointer.
        unsafe {
            let layout = Layout::from_size_align(16, 8).unwrap();
            let pointer = std::alloc::alloc(layout);
            assert!(!pointer.is_null());
            std::hint::black_box(pointer);
            let grown = std::alloc::realloc(pointer, layout, 32);
            if grown.is_null() {
                std::alloc::dealloc(pointer, layout);
                panic!("allocator refused observer self-test");
            }
            std::hint::black_box(grown);
            std::alloc::dealloc(grown, Layout::from_size_align(32, 8).unwrap());
            let zero = std::alloc::alloc_zeroed(layout);
            assert!(!zero.is_null());
            std::hint::black_box(zero);
            std::alloc::dealloc(zero, layout);
        }
    });
    assert_eq!(count, 3);
    let panic = std::panic::catch_unwind(|| measured(|| panic!("observer unwind control")));
    assert!(panic.is_err());
    assert!(!ENABLED.with(Cell::get));
    assert_eq!(measured(|| std::hint::black_box(7)).1, 0);
}

use iroha_data_model::fastpq::{FASTPQ_QUANTITY_UNIT_LIMBS, FastpqQuantityUnits};
use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
};

#[test]
fn fixed_limb_normalization_validation_and_arithmetic_allocate_nothing() {
    let mut maximum_bytes = [0xff; 64];
    maximum_bytes[63] = 0x7f;
    let maximum = Quantity::from_canonical_numeric(
        Numeric::try_new(BigInt::from_twos_bytes(&maximum_bytes).unwrap(), 0).unwrap(),
    )
    .unwrap();
    let fractional =
        Quantity::from_canonical_numeric(Numeric::try_new(BigInt::one(), 28).unwrap()).unwrap();
    let values = [
        Quantity::zero(),
        Quantity::one(),
        Quantity::from(u128::MAX),
        maximum,
        fractional,
    ];
    for value in &values {
        for scale in value.scale()..=28 {
            let (units, allocations) =
                measured(|| FastpqQuantityUnits::from_quantity(std::hint::black_box(value), scale));
            assert_eq!(allocations, 0);
            let units = units.unwrap();
            let ((copy, sum, difference, comparison), allocations) = measured(|| {
                (
                    FastpqQuantityUnits::from_limbs(*units.limbs(), scale),
                    units.checked_add(&units),
                    units.checked_sub(&units),
                    units.checked_cmp(&units),
                )
            });
            assert_eq!(allocations, 0);
            assert_eq!(copy, Some(units));
            assert_eq!(difference.unwrap().try_to_u64(), Some(0));
            assert_eq!(comparison, Some(core::cmp::Ordering::Equal));
            // The independent allocating ledger reference runs outside measurement.
            let expected = value.checked_add(value).ok();
            assert_eq!(sum.and_then(|result| result.to_quantity()), expected);
        }
    }
    let one = FastpqQuantityUnits::from_quantity(&values[1], 0).unwrap();
    let zero = FastpqQuantityUnits::from_quantity(&values[0], 0).unwrap();
    let other_scale = FastpqQuantityUnits::from_quantity(&values[1], 1).unwrap();
    for scale in [0, 1, 28, 29, u32::MAX] {
        let ((limbs, invalid_scale, underflow, mismatch), allocations) = measured(|| {
            (
                FastpqQuantityUnits::from_limbs([u32::MAX; FASTPQ_QUANTITY_UNIT_LIMBS], scale),
                FastpqQuantityUnits::from_quantity(&values[0], 29),
                zero.checked_sub(&one),
                one.checked_add(&other_scale),
            )
        });
        assert_eq!(allocations, 0);
        assert!(
            limbs.is_none() && invalid_scale.is_none() && underflow.is_none() && mismatch.is_none()
        );
    }
}
