//! Retained original-pool notifications survive scopes and physical owner transfer.

use super::*;

#[test]
fn retained_refunds_reuse_credits_without_allocating_or_waking_before_batch_drop() {
    let budget = AllocationBudget::new(8 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let clone = budget.clone();
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve(layout(1)).unwrap_err(),
        &mut budget_registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    let mut batch = without_allocations(|| budget.deferred_refund_batch());
    without_allocations(|| {
        batch.with_scope(|_| {
            clone.with_deferred_refund_notifications(|_| drop(owner));
            assert_eq!(
                budget.reserved_bytes(),
                ReleaseRegistration::allocation_layout().size()
            );
            drop(clone.try_reserve(layout(8)).unwrap());
        });
    });
    assert_eq!(wakes.0.load(SeqCst), 0);
    assert!(poll(&mut wait, &wakes).is_pending());
    without_allocations(|| drop(batch));
    assert_eq!(wakes.0.load(SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
}

#[test]
fn an_empty_batch_never_announces_capacity_or_observes_other_scopes() {
    let budget = AllocationBudget::new(1 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let owner = budget.try_reserve(layout(1)).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve(layout(1)).unwrap_err(),
        &mut budget_registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    without_allocations(|| {
        let mut batch = budget.deferred_refund_batch();
        batch.with_scope(|_| {});
        drop(batch);
    });
    assert_eq!(wakes.0.load(SeqCst), 0);
    drop(owner);
    assert_eq!(wakes.0.load(SeqCst), 1);
}

#[test]
fn retained_same_pool_batches_remain_independent_and_drain_into_enclosing_scope() {
    let budget = AllocationBudget::new(2 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let first_owner = budget.try_reserve(layout(1)).unwrap();
    let second_owner = budget.try_reserve(layout(1)).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve(layout(1)).unwrap_err(),
        &mut budget_registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    let mut outer = budget.deferred_refund_batch();
    let mut inner = budget.deferred_refund_batch();
    outer.with_scope(|_| {
        drop(first_owner);
        inner.with_scope(|_| drop(second_owner));
    });
    assert_eq!(wakes.0.load(SeqCst), 0);
    budget.with_deferred_refund_notifications(|_| {
        drop(inner);
        assert_eq!(wakes.0.load(SeqCst), 0);
        drop(outer);
        assert_eq!(wakes.0.load(SeqCst), 0);
    });
    assert_eq!(wakes.0.load(SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
}

#[test]
fn retained_batch_captures_only_its_original_pool() {
    let first = AllocationBudget::new(1 + ReleaseRegistration::allocation_layout().size());
    let mut first_registration = registration(&first);
    let second = AllocationBudget::new(1 + ReleaseRegistration::allocation_layout().size());
    let mut second_registration = registration(&second);
    let a = first.try_reserve(layout(1)).unwrap();
    let b = second.try_reserve(layout(1)).unwrap();
    let mut first_wait = capacity_wait(
        first.try_reserve(layout(1)).unwrap_err(),
        &mut first_registration,
    );
    let mut second_wait = capacity_wait(
        second.try_reserve(layout(1)).unwrap_err(),
        &mut second_registration,
    );
    let first_wakes = Arc::new(WakeCount::default());
    let second_wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut first_wait, &first_wakes).is_pending());
    assert!(poll(&mut second_wait, &second_wakes).is_pending());
    let mut batch = first.deferred_refund_batch();
    batch.with_scope(|_| {
        drop(a);
        drop(b);
    });
    assert_eq!(first_wakes.0.load(SeqCst), 0);
    assert_eq!(second_wakes.0.load(SeqCst), 1);
    drop(batch);
    assert_eq!(first_wakes.0.load(SeqCst), 1);
}

#[test]
fn caught_unwind_retains_wake_until_the_actual_outer_writer_is_released() {
    struct Probe {
        physical: Arc<Mutex<()>>,
        free: AtomicUsize,
        busy: AtomicUsize,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            if self.physical.try_lock().is_ok() {
                self.free.fetch_add(1, SeqCst);
            } else {
                self.busy.fetch_add(1, SeqCst);
            }
        }
    }
    let budget = AllocationBudget::new(1 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let owner = budget.try_reserve(layout(1)).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve(layout(1)).unwrap_err(),
        &mut budget_registration,
    );
    let physical = Arc::new(Mutex::new(()));
    let probe = Arc::new(Probe {
        physical: Arc::clone(&physical),
        free: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&probe));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let mut batch = budget.deferred_refund_batch();
    let guard = physical.lock().unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            batch.with_scope(|_| {
                drop(owner);
                panic!("local scratch unwinds while retained caller still holds its writer");
            });
        }))
        .is_err()
    );
    assert_eq!(
        budget.reserved_bytes(),
        ReleaseRegistration::allocation_layout().size()
    );
    assert_eq!(probe.free.load(SeqCst), 0);
    assert_eq!(probe.busy.load(SeqCst), 0);
    drop(guard);
    drop(batch);
    assert_eq!(probe.free.load(SeqCst), 1);
    assert_eq!(probe.busy.load(SeqCst), 0);
}

#[test]
fn batch_can_move_to_retirement_thread_after_scope_ends() {
    let budget = AllocationBudget::new(1 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let owner = budget.try_reserve(layout(1)).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve(layout(1)).unwrap_err(),
        &mut budget_registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    let mut batch = budget.deferred_refund_batch();
    batch.with_scope(|_| drop(owner));
    drop(budget);
    assert_eq!(wakes.0.load(SeqCst), 0);
    std::thread::spawn(move || drop(batch)).join().unwrap();
    assert_eq!(wakes.0.load(SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
}

#[test]
fn retained_batch_refunds_join_the_current_owned_scope_after_limit_shrink() {
    let bytes = OwnedAllocationScope::allocation_layout().size();
    let budget = AllocationBudget::new(bytes + 1 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let scope = budget.try_owned_refund_scope().unwrap();
    let held = budget.try_reserve_bytes(1).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve_bytes(1).unwrap_err(),
        &mut budget_registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    let mut batch = budget.deferred_refund_batch();
    without_allocations(|| batch.with_scope(|_| drop(held)));
    assert_eq!(
        budget.reserved_bytes(),
        bytes + ReleaseRegistration::allocation_layout().size()
    );
    assert_eq!(wakes.0.load(SeqCst), 0);
    budget.set_limit_bytes(0);
    assert_eq!(
        budget.peak_reserved_bytes(),
        bytes + 1 + ReleaseRegistration::allocation_layout().size()
    );
    without_allocations(|| drop(batch));
    assert_eq!(wakes.0.load(SeqCst), 0);
    assert!(poll(&mut wait, &wakes).is_pending());
    without_allocations(|| drop(scope));
    assert_eq!(
        budget.reserved_bytes(),
        ReleaseRegistration::allocation_layout().size()
    );
    assert!(wakes.0.load(SeqCst) > 0);
    assert!(poll(&mut wait, &wakes).is_ready());
    assert_eq!(
        budget.peak_reserved_bytes(),
        bytes + 1 + ReleaseRegistration::allocation_layout().size()
    );
}

#[test]
fn owned_scope_outlives_its_batch_scope_without_a_stale_tls_link() {
    let bytes = OwnedAllocationScope::allocation_layout().size();
    let budget = AllocationBudget::new(bytes + 1 + ReleaseRegistration::allocation_layout().size());
    let mut budget_registration = registration(&budget);
    let mut batch = budget.deferred_refund_batch();
    let scope = batch.with_scope(|_| budget.try_owned_refund_scope().unwrap());
    // Removing the borrowed scope must relink this admitted owned record.
    let held = budget.try_reserve_bytes(1).unwrap();
    let mut wait = capacity_wait(
        budget.try_reserve_bytes(1).unwrap_err(),
        &mut budget_registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    without_allocations(|| drop(held));
    without_allocations(|| drop(batch));
    assert_eq!(wakes.0.load(SeqCst), 0);
    without_allocations(|| drop(scope));
    assert_eq!(
        budget.reserved_bytes(),
        ReleaseRegistration::allocation_layout().size()
    );
    assert!(wakes.0.load(SeqCst) > 0);
    assert!(poll(&mut wait, &wakes).is_ready());
    without_allocations(|| budget.with_deferred_refund_notifications(|_| {}));
}
