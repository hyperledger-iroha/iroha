//! Observe initialized private backing at the real allocator boundary, before original credit returns.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
};

use super::super::{Memory, MemoryImage};
use crate::execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan};
use mv::allocation::AllocationBudget;

std::thread_local! {
    static CAPTURE_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

struct ObservedAllocator;
static SERIAL: Mutex<()> = Mutex::new(());
static POINTER: AtomicUsize = AtomicUsize::new(0);
static SPAN_OFFSET: AtomicUsize = AtomicUsize::new(0);
static SPAN_BYTES: AtomicUsize = AtomicUsize::new(0);
static SECOND_OFFSET: AtomicUsize = AtomicUsize::new(0);
static SECOND_BYTES: AtomicUsize = AtomicUsize::new(0);
static CAPTURE_SIZE: AtomicUsize = AtomicUsize::new(0);
static CAPTURE_ALIGN: AtomicUsize = AtomicUsize::new(0);
static CAPTURE_POINTER: AtomicUsize = AtomicUsize::new(0);
static EXPECTED_SIZE: AtomicUsize = AtomicUsize::new(0);
static EXPECTED_ALIGN: AtomicUsize = AtomicUsize::new(0);
static EXACT_LAYOUT: AtomicBool = AtomicBool::new(false);
static ZEROED: AtomicBool = AtomicBool::new(false);
static FREED: AtomicBool = AtomicBool::new(false);
static CREDIT_AT_FREE: AtomicUsize = AtomicUsize::new(0);
static RETENTION_AT_FREE: AtomicUsize = AtomicUsize::new(0);
static BUDGET: OnceLock<AllocationBudget> = OnceLock::new();

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the allocator contract unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if CAPTURE_ACTIVE
            .try_with(std::cell::Cell::get)
            .unwrap_or(false)
            && layout.size() == CAPTURE_SIZE.load(SeqCst)
            && layout.align() == CAPTURE_ALIGN.load(SeqCst)
        {
            CAPTURE_POINTER.store(pointer as usize, SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let observed = POINTER
            .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
            .is_ok();
        if observed {
            let offset = SPAN_OFFSET.load(SeqCst);
            let bytes = SPAN_BYTES.load(SeqCst);
            let exact = layout.size() == EXPECTED_SIZE.load(SeqCst)
                && layout.align() == EXPECTED_ALIGN.load(SeqCst)
                && offset
                    .checked_add(bytes)
                    .is_some_and(|end| end <= layout.size());
            EXACT_LAYOUT.store(exact, SeqCst);
            if exact {
                // SAFETY: watch records the original live allocation and only
                // its fully initialized byte image or inline register array;
                // no padding, uninitialized capacity or freed memory is read.
                let values = unsafe { std::slice::from_raw_parts(pointer.add(offset), bytes) };
                let mut zeroed = values.iter().all(|value| *value == 0);
                let second_offset = SECOND_OFFSET.load(SeqCst);
                let second_bytes = SECOND_BYTES.load(SeqCst);
                if second_bytes != 0 {
                    if second_offset
                        .checked_add(second_bytes)
                        .is_some_and(|end| end <= layout.size())
                    {
                        // SAFETY: the second watched span is another initialized
                        // field in this same still-live allocation, never padding.
                        let second = unsafe {
                            std::slice::from_raw_parts(pointer.add(second_offset), second_bytes)
                        };
                        zeroed &= second.iter().all(|value| *value == 0);
                    } else {
                        EXACT_LAYOUT.store(false, SeqCst);
                        zeroed = false;
                    }
                }
                ZEROED.store(zeroed, SeqCst);
            }
        }
        // SAFETY: the caller supplies the original pointer/layout, forwarded once.
        unsafe { System.dealloc(pointer, layout) };
        if observed {
            CREDIT_AT_FREE.store(
                BUDGET.get().map_or(0, AllocationBudget::reserved_bytes),
                SeqCst,
            );
            RETENTION_AT_FREE.store(
                crate::cache_memory::memory_stats().measured_resident_bytes(),
                SeqCst,
            );
            FREED.store(true, SeqCst);
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

pub(crate) fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn budget() -> &'static AllocationBudget {
    BUDGET.get_or_init(|| AllocationBudget::new(128 * 1024 * 1024))
}

pub(crate) struct Observation;
impl Drop for Observation {
    fn drop(&mut self) {
        POINTER.store(0, SeqCst);
        CAPTURE_SIZE.store(0, SeqCst);
    }
}

/// Observe one owned allocation and an exact, initialized subspan at its final free.
pub(crate) fn watch(
    pointer: *const u8,
    layout: Layout,
    offset: usize,
    bytes: usize,
) -> Observation {
    assert_eq!(POINTER.load(SeqCst), 0);
    assert!(
        offset
            .checked_add(bytes)
            .is_some_and(|end| end <= layout.size())
    );
    SECOND_OFFSET.store(0, SeqCst);
    SECOND_BYTES.store(0, SeqCst);
    SPAN_OFFSET.store(offset, SeqCst);
    SPAN_BYTES.store(bytes, SeqCst);
    EXPECTED_SIZE.store(layout.size(), SeqCst);
    EXPECTED_ALIGN.store(layout.align(), SeqCst);
    EXACT_LAYOUT.store(false, SeqCst);
    ZEROED.store(false, SeqCst);
    FREED.store(false, SeqCst);
    CREDIT_AT_FREE.store(0, SeqCst);
    RETENTION_AT_FREE.store(0, SeqCst);
    POINTER.store(pointer as usize, SeqCst);
    Observation
}

pub(crate) fn freed() -> bool {
    FREED.load(SeqCst)
}

pub(crate) fn assert_erased_and_freed() {
    assert!(FREED.load(SeqCst), "actual backing must be deallocated");
    assert!(
        EXACT_LAYOUT.load(SeqCst),
        "original size/alignment must be preserved"
    );
    assert!(
        ZEROED.load(SeqCst),
        "all initialized observed bytes must be zero before deallocation"
    );
}

#[test]
fn local_image_drop_zeroizes_the_full_exact_box_before_retention_refund() {
    let _serial = serial();
    let mut memory = Memory::new_with_stack_limit(Memory::MIN_STACK_SIZE).unwrap();
    memory.data.fill(0xA5);
    let bytes = memory.data.len();
    assert!(matches!(&memory.data, MemoryImage::Local(_)));
    let _watch = watch(
        memory.data.as_ptr(),
        Layout::array::<u8>(bytes).unwrap(),
        0,
        bytes,
    );
    drop(memory);
    assert_erased_and_freed();
    assert!(
        RETENTION_AT_FREE.load(SeqCst) >= bytes,
        "image retention must outlive physical backing"
    );
}

#[test]
fn funded_image_drop_zeroizes_full_initialized_capacity_before_original_refund() {
    let _serial = serial();
    let budget = budget();
    assert_eq!(budget.reserved_bytes(), 0);
    let mut memory = Memory::new_with_stack_limit_funded(Memory::MIN_STACK_SIZE, budget).unwrap();
    let MemoryImage::Funded(image) = &memory.data else {
        panic!("funded image")
    };
    assert_eq!(image.as_slice().len(), image.capacity());
    memory.data.fill(0xC7);
    let bytes = memory.data.len();
    let _watch = watch(
        memory.data.as_ptr(),
        Layout::array::<u8>(bytes).unwrap(),
        0,
        bytes,
    );
    drop(memory);
    assert_erased_and_freed();
    assert_eq!(
        CREDIT_AT_FREE.load(SeqCst),
        bytes,
        "image credit stays live until System.dealloc returns"
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn failed_memory_construction_erases_its_already_initialized_funded_image() {
    let _serial = serial();
    let budget = budget();
    assert_eq!(budget.reserved_bytes(), 0);
    let bytes = Memory::image_bytes_for_stack_limit(Memory::MIN_STACK_SIZE).unwrap();
    let mut lease =
        ExecutionMemoryLease::reserve(budget, ExecutionMemoryPlan::array::<u8>(bytes).unwrap())
            .unwrap();
    let mut backing = ExecutionBuffer::zeroed(bytes, &mut lease).unwrap();
    backing.as_mut_slice().fill(0xD8);
    let image = MemoryImage::Funded(backing);
    let _watch = watch(
        image.as_ptr(),
        Layout::array::<u8>(bytes).unwrap(),
        0,
        bytes,
    );
    // All parent credit was split to the image. Merkle construction refuses
    // before publishing a Memory, and must destroy this moved image safely.
    assert!(
        Memory::new_with_image(
            Memory::MIN_STACK_SIZE,
            bytes,
            image,
            Some(&mut lease),
            Some(budget)
        )
        .is_err()
    );
    assert_erased_and_freed();
    assert_eq!(CREDIT_AT_FREE.load(SeqCst), bytes);
    drop(lease);
    assert_eq!(budget.reserved_bytes(), 0);
}

/// Observe the actual allocator request rather than assuming Arc header offsets.
pub(crate) struct CaptureScope;

impl Drop for CaptureScope {
    fn drop(&mut self) {
        CAPTURE_ACTIVE.set(false);
    }
}

pub(crate) fn capture_layout(layout: Layout) -> CaptureScope {
    CAPTURE_POINTER.store(0, SeqCst);
    CAPTURE_ALIGN.store(layout.align(), SeqCst);
    CAPTURE_SIZE.store(layout.size(), SeqCst);
    CAPTURE_ACTIVE.set(true);
    CaptureScope
}

pub(crate) fn captured_allocation() -> *const u8 {
    CAPTURE_ACTIVE.set(false);
    CAPTURE_SIZE.store(0, SeqCst);
    let pointer = CAPTURE_POINTER.load(SeqCst);
    assert_ne!(
        pointer, 0,
        "exact requested Arc layout must reach allocator"
    );
    pointer as *const u8
}

pub(crate) fn watch_second_span(offset: usize, bytes: usize) {
    assert!(
        offset
            .checked_add(bytes)
            .is_some_and(|end| end <= EXPECTED_SIZE.load(SeqCst))
    );
    SECOND_OFFSET.store(offset, SeqCst);
    SECOND_BYTES.store(bytes, SeqCst);
}

pub(crate) fn original_credit_at_free() -> usize {
    CREDIT_AT_FREE.load(SeqCst)
}
