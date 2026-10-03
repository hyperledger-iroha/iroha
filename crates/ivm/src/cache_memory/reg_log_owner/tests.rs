//! Exact original shell admission, retained borrowers and final reclamation.

use super::*;
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

fn empty(budget: &AllocationBudget, retention: &MemoryBudget) -> Result<SharedRegLog, VMError> {
    SharedRegLog::try_with_budgets(RegLog::new(Some(budget)), Some(budget), retention)
}

#[test]
fn original_admission_precedes_shell_allocation_and_retries_exactly() {
    let bytes = SharedRegLog::allocation_layout().size();
    let budget = AllocationBudget::new(0);
    let retention = MemoryBudget::new(bytes);
    super::super::with_refused_shared_allocation_for_test(|| {
        assert!(
            matches!(empty(&budget, &retention), Err(VMError::AllocationDeferred(
            AllocationRefusal::ExceedsLimit { requested_bytes, limit_bytes: 0 }
        )) if requested_bytes == bytes)
        );
        assert_eq!(budget.peak_reserved_bytes(), 0);
        assert_eq!(retention.stats().peak_reserved_bytes, 0);
        budget.set_limit_bytes(bytes);
        assert!(matches!(
            empty(&budget, &retention),
            Err(VMError::ExecutionDeferred(
                ExecutionDeferral::AllocationUnavailable
            ))
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(retention.stats().measured_resident_bytes(), 0);
    });
    let owner = empty(&budget, &retention).unwrap();
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(retention.stats().measured_resident_bytes(), bytes);
    assert!(owner.belongs_to(&budget.clone()));
    assert!(!owner.belongs_to(&AllocationBudget::new(bytes)));
    assert!(owner.lock().as_slice().is_empty());
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn final_borrower_retains_original_and_aggregate_credit_after_cache_eviction() {
    let bytes = SharedRegLog::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    let mut cached = empty(&budget, &retention).unwrap();
    assert!(cached.try_retain());
    assert_eq!(retention.stats().shared_reclaimable_bytes, bytes);
    let borrowed = cached.clone();
    assert!(SharedRegLog::ptr_eq(&cached, &borrowed));
    assert_eq!(retention.stats().shared_borrowed_bytes, bytes);
    budget.set_limit_bytes(0);
    retention.set_limit(0);
    drop(cached);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(retention.stats().shared_evicted_live_bytes, bytes);
    assert_eq!(retention.stats().measured_resident_bytes(), bytes);
    assert!(borrowed.lock().as_slice().is_empty());
    drop(borrowed);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn activation_and_zero_retention_never_refund_live_shells() {
    let bytes = SharedRegLog::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    let mut cached = empty(&budget, &retention).unwrap();
    assert!(cached.try_retain());
    let borrower = cached.clone();
    cached.activate();
    assert_eq!(retention.stats().retained_bytes, bytes);
    assert_eq!(retention.stats().shared_evicted_live_bytes, bytes);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(cached.try_retain());
    assert_eq!(retention.stats().shared_borrowed_bytes, bytes);
    drop(borrower);
    assert_eq!(retention.stats().shared_reclaimable_bytes, bytes);
    drop(cached);
    assert_eq!(budget.reserved_bytes(), 0);
    retention.set_limit(0);
    let mut active = empty(&budget, &retention).unwrap();
    assert!(!active.try_retain());
    assert_eq!(retention.stats().active_bytes, bytes);
    drop(active);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn trace_backing_cannot_be_retained_as_only_measured_shell_bytes() {
    let bytes = SharedRegLog::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    let mut owner = empty(&budget, &retention).unwrap();
    budget.set_limit_bytes(bytes + 4 * std::mem::size_of::<crate::zk::RegEvent>());
    owner.prepare_events(1).unwrap();
    assert!(owner.lock().capacity() > 0);
    assert!(!owner.try_retain());
    assert_eq!(retention.stats().retained_bytes, 0);
    assert_eq!(
        budget.reserved_bytes(),
        bytes + 4 * std::mem::size_of::<crate::zk::RegEvent>()
    );
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn concurrent_and_unwound_borrowers_refund_only_the_last_physical_owner() {
    let bytes = SharedRegLog::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(0);
    let owner = empty(&budget, &retention).unwrap();
    let original = owner.clone();
    let barrier = Barrier::new(9);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let borrower = owner.clone();
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                assert!(borrower.lock().as_slice().is_empty());
            });
        }
        drop(owner);
        barrier.wait();
    });
    assert_eq!(budget.reserved_bytes(), bytes);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _final_owner = original;
        panic!("test final owner unwind");
    }));
    assert!(panic.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn original_capacity_release_waits_for_last_logger_reference() {
    let bytes = SharedRegLog::allocation_layout().size();
    let registration_bytes = ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(bytes + registration_bytes);
    let retention = MemoryBudget::new(0);
    let mut registration = ReleaseRegistration::from_reservation(
        &mut budget
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap();
    let owner = empty(&budget, &retention).unwrap();
    let borrower = owner.clone();
    let Err(VMError::AllocationDeferred(AllocationRefusal::Capacity { release, .. })) =
        empty(&budget, &retention)
    else {
        panic!("original full pool must refuse another shell");
    };
    struct ObserveRelease {
        budget: AllocationBudget,
        retained: usize,
        count: Arc<AtomicUsize>,
    }
    impl Wake for ObserveRelease {
        fn wake(self: Arc<Self>) {
            assert_eq!(self.budget.reserved_bytes(), self.retained);
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(ObserveRelease {
        budget: budget.clone(),
        retained: registration_bytes,
        count: count.clone(),
    }));
    let mut context = Context::from_waker(&waker);
    let mut wait = release.wait_for_release(&mut registration);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    drop(owner);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    drop(borrower);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    drop(wait);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}
