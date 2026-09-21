//! Actual allocator controls for MV's prepaid fixed byte buffer.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Poll, Wake, Waker},
};

use mv::allocation::{
    AllocationBudget, AllocationRefusal, ChargedByteBuffer, ChargedByteBufferError,
};

struct ObservedAllocator;

thread_local! {
    static OBSERVE_ALLOCATIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

static SERIAL: Mutex<()> = Mutex::new(());
static NEXT_SIZE: AtomicUsize = AtomicUsize::new(usize::MAX);
static FAIL_NEXT: AtomicBool = AtomicBool::new(false);
static POINTER: AtomicUsize = AtomicUsize::new(0);
static REQUESTED_SIZE: AtomicUsize = AtomicUsize::new(0);
static REQUESTED_ALIGN: AtomicUsize = AtomicUsize::new(0);
static FREED_SIZE: AtomicUsize = AtomicUsize::new(0);
static FREED_ALIGN: AtomicUsize = AtomicUsize::new(0);
static FREED: AtomicBool = AtomicBool::new(false);

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let observed = OBSERVE_ALLOCATIONS
            .try_with(|enabled| enabled.get())
            .unwrap_or(false)
            && NEXT_SIZE
                .compare_exchange(layout.size(), usize::MAX, SeqCst, SeqCst)
                .is_ok();
        if observed {
            REQUESTED_SIZE.store(layout.size(), SeqCst);
            REQUESTED_ALIGN.store(layout.align(), SeqCst);
            if FAIL_NEXT.swap(false, SeqCst) {
                return std::ptr::null_mut();
            }
        }
        // SAFETY: forward the exact allocation request to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if observed {
            POINTER.store(pointer as usize, SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let observed = POINTER
            .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
            .is_ok();
        // SAFETY: the caller supplies the same pointer/layout as allocation.
        unsafe { System.dealloc(pointer, layout) };
        if observed {
            FREED_SIZE.store(layout.size(), SeqCst);
            FREED_ALIGN.store(layout.align(), SeqCst);
            FREED.store(true, SeqCst);
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn observe_next(size: usize, fail: bool) {
    OBSERVE_ALLOCATIONS.with(|enabled| enabled.set(true));
    POINTER.store(0, SeqCst);
    REQUESTED_SIZE.store(0, SeqCst);
    REQUESTED_ALIGN.store(0, SeqCst);
    FREED.store(false, SeqCst);
    FREED_SIZE.store(0, SeqCst);
    FREED_ALIGN.store(0, SeqCst);
    FAIL_NEXT.store(fail, SeqCst);
    NEXT_SIZE.store(size, SeqCst);
}

struct AfterFree {
    budget: AllocationBudget,
    expected: usize,
    wakes: AtomicUsize,
}

impl Wake for AfterFree {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        assert!(
            FREED.load(SeqCst),
            "refund woke before System.dealloc returned"
        );
        assert_eq!(FREED_SIZE.load(SeqCst), self.expected);
        assert_eq!(FREED_ALIGN.load(SeqCst), 1);
        assert_eq!(self.budget.reserved_bytes(), 0);
        self.wakes.fetch_add(1, SeqCst);
    }
}

#[test]
fn exact_backing_layout_and_charge_survive_fill_until_actual_deallocation() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(257);
    observe_next(257, false);
    let mut bytes = ChargedByteBuffer::new(257, &budget).unwrap();
    assert_eq!(REQUESTED_SIZE.load(SeqCst), 257);
    assert_eq!(REQUESTED_ALIGN.load(SeqCst), 1);
    let pointer = bytes.as_slice().as_ptr();
    assert_eq!(pointer as usize, POINTER.load(SeqCst));
    bytes.append(&[37; 200]).unwrap();
    bytes.append(&[91; 57]).unwrap();
    assert_eq!(bytes.as_slice(), &[&[37; 200][..], &[91; 57][..]].concat());
    assert_eq!(bytes.as_slice().as_ptr(), pointer);
    assert!(bytes.append(&[0]).is_err());
    assert_eq!(bytes.as_slice().len(), 257);
    assert_eq!(budget.reserved_bytes(), 257);
    let Err(AllocationRefusal::Capacity { release, .. }) = budget.try_reserve_bytes(1) else {
        panic!("the actual buffer must still own its charge");
    };
    let observed = Arc::new(AfterFree {
        budget: budget.clone(),
        expected: 257,
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&observed));
    let mut released = pin!(release.wait_for_release());
    assert!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(bytes);
    assert_eq!(observed.wakes.load(SeqCst), 1);
    assert!(
        released
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn capacity_refusal_precedes_allocation_and_original_wait_allows_retry() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(79);
    let occupied = budget.try_reserve_bytes(79).unwrap();
    observe_next(79, false);
    let Err(ChargedByteBufferError::Admission(AllocationRefusal::Capacity {
        requested_bytes,
        reserved_bytes,
        limit_bytes,
        release,
    })) = ChargedByteBuffer::new(79, &budget)
    else {
        panic!("capacity must return its original typed refusal");
    };
    assert_eq!((requested_bytes, reserved_bytes, limit_bytes), (79, 79, 79));
    assert_eq!(
        NEXT_SIZE.load(SeqCst),
        79,
        "allocator must not run on refusal"
    );
    let mut released = pin!(release.wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(released.as_mut().poll(&mut context), Poll::Pending);
    drop(occupied);
    assert_eq!(released.as_mut().poll(&mut context), Poll::Ready(()));
    let bytes = ChargedByteBuffer::new(79, &budget).unwrap();
    assert_eq!(REQUESTED_SIZE.load(SeqCst), 79);
    drop(bytes);
    assert!(FREED.load(SeqCst));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn allocator_failure_returns_unused_charge_without_constructing_a_vec() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(113);
    observe_next(113, true);
    let error = match ChargedByteBuffer::new(113, &budget) {
        Err(error) => error,
        Ok(_) => panic!("the observed allocator must refuse this allocation"),
    };
    assert!(matches!(
        error,
        ChargedByteBufferError::Allocator {
            requested_bytes: 113
        }
    ));
    assert_eq!(
        error.to_string(),
        "failed to allocate 113 admitted byte-buffer bytes"
    );
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(REQUESTED_SIZE.load(SeqCst), 113);
    assert_eq!(POINTER.load(SeqCst), 0);
    assert!(!FREED.load(SeqCst));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn zero_and_unrepresentable_layouts_never_request_backing_storage() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(0);
    observe_next(0, false);
    let mut bytes = ChargedByteBuffer::new(0, &budget).unwrap();
    bytes.append(&[]).unwrap();
    assert!(bytes.as_slice().is_empty());
    assert!(bytes.append(&[1]).is_err());
    drop(bytes);
    assert_eq!(NEXT_SIZE.load(SeqCst), 0);
    NEXT_SIZE.store(usize::MAX, SeqCst);
    let error = match ChargedByteBuffer::new(usize::MAX, &budget) {
        Err(error) => error,
        Ok(_) => panic!("the unrepresentable layout must be refused"),
    };
    assert!(matches!(
        error,
        ChargedByteBufferError::Admission(AllocationRefusal::DemandOverflow)
    ));
    assert_eq!(
        error.to_string(),
        AllocationRefusal::DemandOverflow.to_string()
    );
    assert!(
        std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<AllocationRefusal>()
            .is_some()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn unwind_frees_the_original_initialized_buffer_before_refund() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(191);
    observe_next(191, false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut bytes = ChargedByteBuffer::new(191, &budget).unwrap();
        bytes.append(&[9; 87]).unwrap();
        assert_eq!(budget.reserved_bytes(), 191);
        panic!("injected snapshot decode unwind");
    }));
    assert!(result.is_err());
    assert!(FREED.load(SeqCst));
    assert_eq!(
        (FREED_SIZE.load(SeqCst), FREED_ALIGN.load(SeqCst)),
        (191, 1)
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn simultaneous_buffers_share_capacity_until_their_actual_owners_drop() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(128);
    let mut first = ChargedByteBuffer::new(64, &budget).unwrap();
    first.append(&[13; 64]).unwrap();
    let second = ChargedByteBuffer::new(64, &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), 128);
    assert!(matches!(
        ChargedByteBuffer::new(1, &budget),
        Err(ChargedByteBufferError::Admission(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    std::thread::spawn(move || {
        assert_eq!(first.as_slice(), &[13; 64]);
        drop(first);
    })
    .join()
    .unwrap();
    assert_eq!(budget.reserved_bytes(), 64);
    let retry = ChargedByteBuffer::new(64, &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), 128);
    drop((retry, second));
    assert_eq!(budget.reserved_bytes(), 0);
}
