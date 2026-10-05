//! Physical allocation/refund observations for original metadata backing owners.

// Only the fixture registers these pending production primitives. The complete
// decoder return/child-control transfer has not yet been registered.
#[path = "native_allocation.rs"]
mod owner;

use iroha_allocation::AllocationBudget;
use owner::{NativeDecodeBox, NativeDecodeVec};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Allocated,
    Refused,
    BeforeFree,
    AfterFree,
}
#[derive(Clone, Copy, Debug)]
struct Observation {
    phase: Phase,
    pointer: usize,
    bytes: usize,
    reserved: usize,
}
const EMPTY_OBSERVATION: Observation = Observation {
    phase: Phase::Allocated,
    pointer: 0,
    bytes: 0,
    reserved: 0,
};
#[derive(Clone, Copy)]
struct Trace {
    pool: *const AllocationBudget,
    layouts: [Layout; 2],
    fail_next: bool,
    observations: [Observation; 16],
    len: usize,
    overflow: bool,
}
thread_local! {
    static TRACE: Cell<Option<Trace>> = const { Cell::new(None) };
}
struct NativeAllocator;
#[global_allocator]
static ALLOCATOR: NativeAllocator = NativeAllocator;

fn matches(trace: &Trace, layout: Layout) -> bool {
    trace.layouts.contains(&layout)
}
#[allow(unsafe_code)]
fn observe(phase: Phase, pointer: *mut u8, layout: Layout) {
    let _ = TRACE.try_with(|slot| {
        let Some(mut trace) = slot.get() else { return };
        if !matches(&trace, layout) {
            return;
        }
        // SAFETY: TraceGuard borrows the exact pool until it clears this TLS
        // pointer. Observation performs only an atomic read and allocates nothing.
        let reserved = unsafe { &*trace.pool }.reserved_bytes();
        if trace.len == trace.observations.len() {
            trace.overflow = true;
        } else {
            trace.observations[trace.len] = Observation {
                phase,
                pointer: pointer as usize,
                bytes: layout.size(),
                reserved,
            };
            trace.len += 1;
        }
        slot.set(Some(trace));
    });
}
fn refuse(layout: Layout) -> bool {
    TRACE
        .try_with(|slot| {
            let Some(mut trace) = slot.get() else {
                return false;
            };
            if trace.fail_next && matches(&trace, layout) {
                trace.fail_next = false;
                slot.set(Some(trace));
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for NativeAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if refuse(layout) {
            observe(Phase::Refused, std::ptr::null_mut(), layout);
            return std::ptr::null_mut();
        }
        // SAFETY: forward the allocator's exact caller-supplied layout.
        let pointer = unsafe { System.alloc(layout) };
        observe(Phase::Allocated, pointer, layout);
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        observe(Phase::BeforeFree, pointer, layout);
        // SAFETY: forward the matching allocation pointer and layout unchanged.
        unsafe { System.dealloc(pointer, layout) };
        observe(Phase::AfterFree, pointer, layout);
    }
}
struct TraceGuard<'a> {
    _pool: &'a AllocationBudget,
}
impl<'a> TraceGuard<'a> {
    fn begin(pool: &'a AllocationBudget, first: Layout, second: Layout, fail: bool) -> Self {
        TRACE.with(|slot| {
            assert!(slot.get().is_none(), "no nested allocator fixture");
            slot.set(Some(Trace {
                pool,
                layouts: [first, second],
                fail_next: fail,
                observations: [EMPTY_OBSERVATION; 16],
                len: 0,
                overflow: false,
            }));
        });
        Self { _pool: pool }
    }
    fn snapshot(&self) -> Trace {
        TRACE.with(|slot| slot.get().expect("fixture armed"))
    }
}
impl Drop for TraceGuard<'_> {
    fn drop(&mut self) {
        TRACE.with(|slot| slot.set(None));
    }
}

fn assert_freed_before_refund(trace: Trace, pointer: usize, bytes: usize) {
    assert!(!trace.overflow, "fixed allocator observation capacity");
    let records = &trace.observations[..trace.len];
    let before = records
        .iter()
        .find(|record| {
            record.phase == Phase::BeforeFree && record.pointer == pointer && record.bytes == bytes
        })
        .expect("native pre-free observation");
    let after = records
        .iter()
        .find(|record| {
            record.phase == Phase::AfterFree && record.pointer == pointer && record.bytes == bytes
        })
        .expect("native post-free observation");
    assert!(before.reserved >= bytes);
    assert_eq!(
        after.reserved, before.reserved,
        "original credit remains unavailable after physical free, until its guard drops"
    );
}

#[test]
fn native_metadata_vector_exact_pointer_grows_under_both_original_charges() {
    let first = Layout::array::<u64>(2).unwrap();
    let second = Layout::array::<u64>(4).unwrap();
    let pool = AllocationBudget::new(first.size() + second.size());
    let mut reservation = pool
        .try_reserve_bytes(first.size() + second.size())
        .unwrap();
    let trace = TraceGuard::begin(&pool, first, second, false);
    let mut values = NativeDecodeVec::new(2, &pool, &mut reservation).unwrap();
    values.push(11_u64).unwrap();
    values.push(22_u64).unwrap();
    let old_pointer = values.as_slice().as_ptr() as usize;
    assert_eq!(values.capacity(), 2);
    values
        .reserve_additional(1, &pool, &mut reservation)
        .unwrap();
    assert_eq!(values.as_slice(), &[11, 22]);
    assert_eq!(values.capacity(), 4);
    let pointer = values.as_slice().as_ptr() as usize;
    assert_ne!(
        pointer, old_pointer,
        "replacement allocates while original is still live"
    );
    assert_eq!(pool.reserved_bytes(), second.size());
    let growth = trace.snapshot();
    assert!(
        growth.observations[..growth.len]
            .iter()
            .any(|record| record.phase == Phase::Allocated
                && record.bytes == second.size()
                && record.reserved == first.size() + second.size())
    );
    assert_freed_before_refund(growth, old_pointer, first.size());
    drop(values);
    assert_freed_before_refund(trace.snapshot(), pointer, second.size());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_metadata_vector_allocator_refusal_preserves_original_elements_pointer_and_owner() {
    let first = Layout::array::<u64>(2).unwrap();
    let second = Layout::array::<u64>(4).unwrap();
    let pool = AllocationBudget::new(first.size() + second.size());
    let mut reservation = pool
        .try_reserve_bytes(first.size() + second.size())
        .unwrap();
    let mut values = NativeDecodeVec::new(2, &pool, &mut reservation).unwrap();
    values.push(11_u64).unwrap();
    values.push(22_u64).unwrap();
    let pointer = values.as_slice().as_ptr();
    let trace = TraceGuard::begin(&pool, second, second, true);
    assert!(
        values
            .reserve_additional(1, &pool, &mut reservation)
            .is_err()
    );
    assert_eq!(values.as_slice().as_ptr(), pointer);
    assert_eq!(values.as_slice(), &[11, 22]);
    assert_eq!(values.capacity(), 2);
    assert_eq!(pool.reserved_bytes(), first.size());
    let refused = trace.snapshot();
    assert_eq!(refused.len, 1);
    assert_eq!(refused.observations[0].phase, Phase::Refused);
    assert_eq!(
        refused.observations[0].reserved,
        first.size() + second.size()
    );
    assert_eq!(refused.observations[0].pointer, 0);
    drop(values);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_metadata_vector_foreign_pool_and_overflow_refuse_before_allocator() {
    let layout = Layout::array::<u64>(2).unwrap();
    let pool = AllocationBudget::new(layout.size());
    let foreign = AllocationBudget::new(layout.size());
    let mut reservation = pool.try_reserve(layout).unwrap();
    let trace = TraceGuard::begin(&pool, layout, layout, false);
    assert!(NativeDecodeVec::<u64>::new(2, &foreign, &mut reservation).is_err());
    assert!(NativeDecodeVec::<u64>::new(usize::MAX, &pool, &mut reservation).is_err());
    assert_eq!(trace.snapshot().len, 0);
    assert_eq!(reservation.remaining_bytes(), layout.size());
    assert_eq!(pool.reserved_bytes(), layout.size());
    drop(reservation);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_metadata_vector_short_original_remainder_refuses_before_growth() {
    let first = Layout::array::<u64>(2).unwrap();
    let second = Layout::array::<u64>(4).unwrap();
    let pool = AllocationBudget::new(first.size() + second.size());
    let mut initial = pool.try_reserve(first).unwrap();
    let mut values = NativeDecodeVec::new(2, &pool, &mut initial).unwrap();
    values.push(11_u64).unwrap();
    values.push(22_u64).unwrap();
    let pointer = values.as_slice().as_ptr();
    let mut short = pool.try_reserve_bytes(second.size() - 1).unwrap();
    let trace = TraceGuard::begin(&pool, second, second, false);
    assert!(values.reserve_additional(1, &pool, &mut short).is_err());
    assert_eq!(values.as_slice().as_ptr(), pointer);
    assert_eq!(values.as_slice(), &[11, 22]);
    assert_eq!(short.remaining_bytes(), second.size() - 1);
    assert_eq!(trace.snapshot().len, 0);
    drop(short);
    assert_eq!(pool.reserved_bytes(), first.size());
    drop(values);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(Debug)]
#[repr(C, align(64))]
struct BoxValue([u8; 64]);
#[test]
fn native_metadata_box_exact_pointer_frees_before_original_credit() {
    let layout = Layout::new::<BoxValue>();
    let pool = AllocationBudget::new(layout.size());
    let mut reservation = pool.try_reserve(layout).unwrap();
    let trace = TraceGuard::begin(&pool, layout, layout, false);
    let value = NativeDecodeBox::new(BoxValue([7; 64]), &pool, &mut reservation).unwrap();
    let pointer = std::ptr::from_ref(value.as_ref()) as usize;
    assert_eq!(value.as_ref().0, [7; 64]);
    let allocated = trace.snapshot();
    assert_eq!(allocated.len, 1);
    assert_eq!(allocated.observations[0].pointer, pointer);
    assert_eq!(allocated.observations[0].bytes, layout.size());
    assert_eq!(allocated.observations[0].reserved, layout.size());
    drop(value);
    assert_freed_before_refund(trace.snapshot(), pointer, layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_metadata_box_allocator_refusal_returns_original_value_and_refunds_no_live_backing() {
    let layout = Layout::new::<BoxValue>();
    let pool = AllocationBudget::new(layout.size());
    let mut reservation = pool.try_reserve(layout).unwrap();
    let trace = TraceGuard::begin(&pool, layout, layout, true);
    let Err((value, _error)) = NativeDecodeBox::new(BoxValue([9; 64]), &pool, &mut reservation)
    else {
        panic!("physical allocator must refuse the original prepaid Box")
    };
    assert_eq!(value.0, [9; 64]);
    let refused = trace.snapshot();
    assert_eq!(refused.len, 1);
    assert_eq!(refused.observations[0].phase, Phase::Refused);
    assert_eq!(refused.observations[0].reserved, layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(Debug)]
#[repr(C, align(64))]
struct PanicValue<'a> {
    _bytes: [u8; 64],
    observed: &'a Cell<bool>,
}
impl Drop for PanicValue<'_> {
    fn drop(&mut self) {
        self.observed.set(true);
        panic!("native metadata payload drop fault");
    }
}
#[test]
fn native_metadata_box_unwind_preserves_credit_until_actual_box_deallocation() {
    let observed = Cell::new(false);
    let layout = Layout::new::<PanicValue<'_>>();
    let pool = AllocationBudget::new(layout.size());
    let mut reservation = pool.try_reserve(layout).unwrap();
    let trace = TraceGuard::begin(&pool, layout, layout, false);
    let value = NativeDecodeBox::new(
        PanicValue {
            _bytes: [0; 64],
            observed: &observed,
        },
        &pool,
        &mut reservation,
    )
    .unwrap();
    let pointer = std::ptr::from_ref(value.as_ref()) as usize;
    assert!(catch_unwind(AssertUnwindSafe(|| drop(value))).is_err());
    assert!(observed.get());
    assert_freed_before_refund(trace.snapshot(), pointer, layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_metadata_vector_unwind_preserves_credit_until_actual_vec_deallocation() {
    let observed = Cell::new(false);
    let layout = Layout::array::<PanicValue<'_>>(2).unwrap();
    let pool = AllocationBudget::new(layout.size());
    let mut reservation = pool.try_reserve(layout).unwrap();
    let trace = TraceGuard::begin(&pool, layout, layout, false);
    let mut values = NativeDecodeVec::new(2, &pool, &mut reservation).unwrap();
    values
        .push(PanicValue {
            _bytes: [0; 64],
            observed: &observed,
        })
        .unwrap();
    let pointer = values.as_slice().as_ptr() as usize;
    assert!(catch_unwind(AssertUnwindSafe(|| drop(values))).is_err());
    assert!(observed.get());
    assert_freed_before_refund(trace.snapshot(), pointer, layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(Debug)]
struct Zero;
#[test]
fn native_metadata_zero_size_owners_keep_logical_bounds_without_allocating() {
    let pool = AllocationBudget::new(0);
    let mut reservation = pool.try_reserve_bytes(0).unwrap();
    let layout = Layout::new::<Zero>();
    let trace = TraceGuard::begin(&pool, layout, layout, false);
    let mut values = NativeDecodeVec::new(2, &pool, &mut reservation).unwrap();
    values.push(Zero).unwrap();
    values.push(Zero).unwrap();
    assert!(values.push(Zero).is_err());
    values
        .reserve_additional(1, &pool, &mut reservation)
        .unwrap();
    values.push(Zero).unwrap();
    assert_eq!(values.capacity(), 4);
    assert_eq!(values.as_slice().len(), 3);
    let value = NativeDecodeBox::new(Zero, &pool, &mut reservation).unwrap();
    drop(value);
    drop(values);
    assert_eq!(trace.snapshot().len, 0);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_metadata_zero_size_box_still_requires_exact_original_pool() {
    let pool = AllocationBudget::new(0);
    let foreign = AllocationBudget::new(0);
    let mut reservation = pool.try_reserve_bytes(0).unwrap();
    let layout = Layout::new::<Zero>();
    let trace = TraceGuard::begin(&pool, layout, layout, false);
    let Err((value, _error)) = NativeDecodeBox::new(Zero, &foreign, &mut reservation) else {
        panic!("equal limits cannot replace original pool identity")
    };
    assert!(matches!(value, Zero));
    assert_eq!(trace.snapshot().len, 0);
    assert_eq!(reservation.remaining_bytes(), 0);
}
