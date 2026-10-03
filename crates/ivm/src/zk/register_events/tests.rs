//! Original row admission, exact overlapping growth and real scrub-before-refund.

use super::*;
use iroha_crypto::{Hash, HashOf};
use std::{alloc::Layout, mem::size_of};

fn event(value: u64, written: bool) -> RegEvent {
    let root = HashOf::from_untyped_unchecked(Hash::new(value.to_le_bytes()));
    let path = [[0xa5; 32]; crate::REGISTER_MERKLE_PATH_DEPTH];
    if written {
        RegEvent::Write {
            index: 7,
            value,
            tag: true,
            path,
            root,
        }
    } else {
        RegEvent::Read {
            index: 7,
            value,
            tag: true,
            path,
            root,
        }
    }
}
fn prepare(log: &mut RegLog, count: usize, original: &AllocationBudget) -> Result<(), VMError> {
    original.with_deferred_refund_notifications(|scope| log.prepare_events(count, Some(scope)))
}
fn append(log: &mut RegLog, value: u64, original: &AllocationBudget) {
    prepare(log, 1, original).unwrap();
    log.record_reserved(event(value, value % 2 == 1));
}

#[test]
fn exact_original_growth_keeps_both_backings_and_preserves_refused_rows() {
    let size = size_of::<RegEvent>();
    let original = AllocationBudget::new(0);
    let mut log = RegLog::new(Some(&original));
    assert!(
        matches!(prepare(&mut log, 1, &original), Err(VMError::AllocationDeferred(
        AllocationRefusal::ExceedsLimit { requested_bytes, limit_bytes: 0 }
    )) if requested_bytes == 4 * size)
    );
    assert_eq!(original.peak_reserved_bytes(), 0);
    original.set_limit_bytes(4 * size);
    for value in 0..4 {
        append(&mut log, value, &original);
    }
    let pointer = log.as_slice().as_ptr();
    original.set_limit_bytes(12 * size - 1);
    let Err(VMError::AllocationDeferred(AllocationRefusal::Capacity {
        requested_bytes,
        release,
        ..
    })) = prepare(&mut log, 1, &original)
    else {
        panic!("original overlap must refuse before allocation");
    };
    assert_eq!(requested_bytes, 8 * size);
    let Err(AllocationRefusal::Capacity {
        release: original_release,
        ..
    }) = original.try_reserve(Layout::array::<RegEvent>(8).unwrap())
    else {
        panic!("the same original pool must retain the refused demand");
    };
    assert_eq!(release, original_release);
    assert_eq!(log.as_slice().as_ptr(), pointer);
    assert_eq!(
        log.as_slice(),
        &[
            event(0, false),
            event(1, true),
            event(2, false),
            event(3, true)
        ]
    );
    original.set_limit_bytes(12 * size);
    append(&mut log, 4, &original);
    assert_eq!(original.peak_reserved_bytes(), 12 * size);
    assert_eq!(original.reserved_bytes(), 8 * size);
    assert_ne!(log.as_slice().as_ptr(), pointer);
    original.set_limit_bytes(0);
    prepare(&mut log, 3, &original).unwrap();
    for value in 5..8 {
        log.record_reserved(event(value, true));
    }
    assert_eq!(original.reserved_bytes(), 8 * size);
    assert_eq!(log.allocated_bytes().unwrap(), 8 * size);
    drop(log);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn missing_or_foreign_scope_cannot_grow_reset_or_copy_even_with_equal_limits() {
    let original = AllocationBudget::new(32 * size_of::<RegEvent>());
    let foreign = AllocationBudget::new(32 * size_of::<RegEvent>());
    let mut log = RegLog::new(Some(&original));
    append(&mut log, 17, &original);
    let pointer = log.as_slice().as_ptr();
    assert!(matches!(
        log.prepare_events(0, None),
        Err(VMError::HostUnavailable)
    ));
    foreign.with_deferred_refund_notifications(|scope| {
        assert!(matches!(
            log.prepare_events(8, Some(scope)),
            Err(VMError::HostUnavailable)
        ));
        assert!(matches!(
            log.reset(Some(scope)),
            Err(VMError::HostUnavailable)
        ));
        assert!(matches!(
            log.try_clone_allocation(Some(scope)),
            Err(VMError::HostUnavailable)
        ));
    });
    assert_eq!(log.as_slice(), &[event(17, true)]);
    assert_eq!(log.as_slice().as_ptr(), pointer);
    assert_eq!(foreign.reserved_bytes(), 0);
}

#[test]
fn allocator_refusal_overflow_and_copy_unwind_preserve_original_owner() {
    let size = size_of::<RegEvent>();
    let original = AllocationBudget::new(32 * size);
    let mut log = RegLog::new(Some(&original));
    append(&mut log, 29, &original);
    let pointer = log.as_slice().as_ptr();
    REFUSE_NEXT_ALLOCATION.set(true);
    assert!(matches!(
        prepare(&mut log, 7, &original),
        Err(VMError::ExecutionDeferred(
            ExecutionDeferral::AllocationUnavailable
        ))
    ));
    assert!(matches!(
        prepare(&mut log, usize::MAX, &original),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::DemandOverflow
        ))
    ));
    PANIC_AFTER_COPY.set(true);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepare(&mut log, 7, &original).unwrap();
    }));
    assert!(panic.is_err());
    assert_eq!(log.as_slice().as_ptr(), pointer);
    assert_eq!(log.as_slice(), &[event(29, true)]);
    assert_eq!(original.reserved_bytes(), 4 * size);
    prepare(&mut log, 7, &original).unwrap();
    assert_eq!(original.reserved_bytes(), 8 * size);
}

#[test]
fn clear_reuses_original_credit_and_reset_never_rebinds_the_pool() {
    let size = size_of::<RegEvent>();
    let original = AllocationBudget::new(4 * size);
    let mut log = RegLog::new(Some(&original));
    append(&mut log, 11, &original);
    let pointer = log.as_slice().as_ptr();
    original.set_limit_bytes(0);
    log.scrub();
    assert_eq!(original.reserved_bytes(), 4 * size);
    append(&mut log, 23, &original);
    assert_eq!(log.as_slice().as_ptr(), pointer);
    assert_eq!(log.as_slice(), &[event(23, true)]);
    original
        .with_deferred_refund_notifications(|scope| log.reset(Some(scope)))
        .unwrap();
    assert_eq!(original.reserved_bytes(), 0);
    assert!(matches!(
        prepare(&mut log, 1, &original),
        Err(VMError::AllocationDeferred(_))
    ));
}

#[test]
fn independent_copy_and_final_borrower_each_keep_their_original_rows_alive() {
    use std::sync::Arc;
    let size = size_of::<RegEvent>();
    let original = AllocationBudget::new(4 * size);
    let mut log = RegLog::new(Some(&original));
    append(&mut log, 13, &original);
    assert!(matches!(
        original.with_deferred_refund_notifications(|scope| log.try_clone_allocation(Some(scope))),
        Err(VMError::AllocationDeferred(_))
    ));
    original.set_limit_bytes(8 * size);
    let copy = original
        .with_deferred_refund_notifications(|scope| log.try_clone_allocation(Some(scope)))
        .unwrap();
    assert_ne!(log.as_slice().as_ptr(), copy.as_slice().as_ptr());
    assert_eq!(log.as_slice(), copy.as_slice());
    drop(log);
    assert_eq!(original.reserved_bytes(), 4 * size);
    let owner = Arc::new(copy);
    let borrower = owner.clone();
    drop(owner);
    assert_eq!(original.reserved_bytes(), 4 * size);
    assert_eq!(borrower.as_slice(), &[event(13, true)]);
    drop(borrower);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn standalone_growth_is_fallible_without_creating_a_state_pool() {
    let mut log = RegLog::new(None);
    crate::cache_memory::refuse_next_owned_vec_growth_for_test();
    assert!(matches!(
        log.prepare_events(1, None),
        Err(VMError::ExecutionDeferred(
            ExecutionDeferral::AllocationUnavailable
        ))
    ));
    for value in 0..12 {
        log.prepare_events(1, None).unwrap();
        log.record_reserved(event(value, true));
    }
    let copy = log.try_clone_allocation(None).unwrap();
    assert_eq!(log.as_slice(), copy.as_slice());
    assert!(copy.original.is_none());
    assert_eq!(copy.as_slice().len(), 12);
    log.reset(None).unwrap();
    assert_eq!(log.allocated_bytes().unwrap(), 0);
}

#[test]
fn each_initialized_field_is_erased_at_actual_deallocation_before_credit_refund() {
    use crate::memory::private_disposal::tests as observer;
    let _serial = observer::serial();
    let original = observer::budget();
    for written in [false, true] {
        for field in 0..5 {
            assert_eq!(original.reserved_bytes(), 0);
            let mut log = RegLog::new(Some(original));
            prepare(&mut log, 1, original).unwrap();
            log.record_reserved(event(0xabcdef0123456789, written));
            let base = log.as_slice().as_ptr().cast::<u8>();
            let (index, value, tag, path, root) = match &log.as_slice()[0] {
                RegEvent::Read {
                    index,
                    value,
                    tag,
                    path,
                    root,
                }
                | RegEvent::Write {
                    index,
                    value,
                    tag,
                    path,
                    root,
                } => (index, value, tag, path, root),
            };
            let spans = [
                (std::ptr::from_ref(index).cast::<u8>(), size_of::<usize>()),
                (std::ptr::from_ref(value).cast::<u8>(), size_of::<u64>()),
                (std::ptr::from_ref(tag).cast::<u8>(), size_of::<bool>()),
                (
                    path.as_ptr().cast::<u8>(),
                    size_of::<[[u8; 32]; crate::REGISTER_MERKLE_PATH_DEPTH]>(),
                ),
                (root.as_ref().as_ptr(), 32),
            ];
            let offset = spans[field].0 as usize - base as usize;
            let credit = original.reserved_bytes();
            let _watch = observer::watch(
                base,
                Layout::array::<RegEvent>(log.capacity()).unwrap(),
                offset,
                spans[field].1,
            );
            // The actual Rows destructor, not a duplicated manual erase loop.
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _owner = log;
                panic!("final register row owner unwind");
            }));
            assert!(panic.is_err());
            observer::assert_erased_and_freed();
            assert_eq!(observer::original_credit_at_free(), credit);
            assert_eq!(original.reserved_bytes(), 0);
        }
    }
}

#[test]
fn growth_refund_callbacks_run_after_the_original_logger_mutex_is_released() {
    use iroha_allocation::release::ReleaseRegistration;
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };
    let size = size_of::<RegEvent>();
    let registration_bytes = ReleaseRegistration::allocation_layout().size();
    let original = AllocationBudget::new(12 * size + registration_bytes);
    let mut registration = ReleaseRegistration::from_reservation(
        &mut original
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap();
    let mut rows = RegLog::new(Some(&original));
    for value in 0..4 {
        append(&mut rows, value, &original);
    }
    let owner = Arc::new(parking_lot::Mutex::new(rows));
    let Err(AllocationRefusal::Capacity { release, .. }) =
        original.try_reserve(Layout::array::<RegEvent>(9).unwrap())
    else {
        panic!("original capacity pressure")
    };
    struct Reenter {
        log: Arc<parking_lot::Mutex<RegLog>>,
        count: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            let log = self
                .log
                .try_lock()
                .expect("refund callback must run outside the original mutex");
            assert_eq!(log.capacity(), 8);
            assert_eq!(log.as_slice().len(), 4);
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }
    let reenter = Arc::new(Reenter {
        log: owner.clone(),
        count: AtomicUsize::new(0),
    });
    let waker = Waker::from(reenter.clone());
    let mut cx = Context::from_waker(&waker);
    let mut wait = release.wait_for_release(&mut registration);
    assert_eq!(Pin::new(&mut wait).poll(&mut cx), Poll::Pending);
    original.with_deferred_refund_notifications(|outer| {
        original.with_deferred_refund_notifications(|scope| {
            let mut locked = owner.lock();
            locked.prepare_events(1, Some(scope)).unwrap();
            assert_eq!(reenter.count.load(Ordering::SeqCst), 0);
        });
        assert!(outer.belongs_to(&original));
        assert_eq!(reenter.count.load(Ordering::SeqCst), 0);
    });
    assert_eq!(reenter.count.load(Ordering::SeqCst), 1);
    assert_eq!(Pin::new(&mut wait).poll(&mut cx), Poll::Ready(()));
    drop(wait);
    drop(waker);
    drop(reenter);
    drop(owner);
    drop(registration);
    assert_eq!(original.reserved_bytes(), 0);
}
