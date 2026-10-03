//! Reusable original-pool waiters preserve physical custody through every arm.

use super::*;
use crate::test_support::{refusing_allocation, without_allocations};
use crate::{AllocationBudget, AllocationRefusal, PrepaidSharedError};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::SeqCst},
};
use std::task::Wake;

fn registration(budget: &AllocationBudget) -> ReleaseRegistration {
    ReleaseRegistration::from_reservation(
        &mut budget
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap()
}

#[derive(Default)]
struct Count(AtomicUsize);
impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, SeqCst);
    }
}

#[test]
fn exact_registration_refusal_preserves_reservation_and_original_pool() {
    let layout = ReleaseRegistration::allocation_layout();
    let budget = AllocationBudget::new(layout.size());
    let foreign = AllocationBudget::new(layout.size());
    let mut short = budget.try_reserve_bytes(layout.size() - 1).unwrap();
    let error = without_allocations(|| ReleaseRegistration::from_reservation(&mut short))
        .expect_err("short original reservation");
    assert!(matches!(error, PrepaidSharedError::Reservation(_)));
    assert_eq!(short.remaining_bytes(), layout.size() - 1);
    assert_eq!(budget.reserved_bytes(), layout.size() - 1);
    drop(short);
    let mut original = budget.try_reserve(layout).unwrap();
    let error = refusing_allocation(layout, || {
        ReleaseRegistration::from_reservation(&mut original)
    })
    .expect_err("physical allocator refusal");
    assert_eq!(
        error,
        PrepaidSharedError::Allocator {
            requested_bytes: layout.size()
        }
    );
    assert_eq!(original.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
    let owner = registration(&budget);
    assert!(owner.belongs_to(&budget));
    assert!(!owner.belongs_to(&foreign));
    assert!(matches!(
        budget.try_reserve_bytes(1),
        Err(AllocationRefusal::Capacity { .. })
    ));
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_registration_rearms_cancels_and_releases_without_allocating() {
    let budget = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut slot = registration(&budget);
    let source = ReleaseNotification::default();
    let foreign = ReleaseNotification::default();
    let wakes = Arc::new(Count::default());
    let waker = Waker::from(Arc::clone(&wakes));
    let mut context = Context::from_waker(&waker);
    let pointer = slot.pointer();
    for _ in 0..32 {
        let observed = source.observe();
        without_allocations(|| {
            assert!(slot.poll_wait(&observed, &mut context).is_pending());
            assert!(slot.poll_wait(&observed, &mut context).is_pending());
            drop(foreign.guard(()));
            assert!(slot.poll_wait(&observed, &mut context).is_pending());
            slot.cancel();
            assert_eq!(source.state.lock().unwrap().waiters.count, 0);
            assert!(slot.poll_wait(&observed, &mut context).is_pending());
            drop(source.guard(()));
            assert!(slot.poll_wait(&observed, &mut context).is_ready());
            slot.cancel();
        });
        assert_eq!(slot.pointer(), pointer);
        assert_eq!(
            budget.reserved_bytes(),
            ReleaseRegistration::allocation_layout().size()
        );
    }
    assert_eq!(wakes.0.load(SeqCst), 32);
    without_allocations(|| drop(slot));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn source_replacement_and_old_releases_never_consume_new_callback() {
    let budget = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut slot = registration(&budget);
    let old = ReleaseNotification::default();
    let new = ReleaseNotification::default();
    let old_wakes = Arc::new(Count::default());
    let new_wakes = Arc::new(Count::default());
    let old_waker = Waker::from(Arc::clone(&old_wakes));
    let new_waker = Waker::from(Arc::clone(&new_wakes));
    let observed_old = old.observe();
    let observed_new = new.observe();
    assert!(
        slot.poll_wait(&observed_old, &mut Context::from_waker(&old_waker))
            .is_pending()
    );
    assert!(
        slot.poll_wait(&observed_new, &mut Context::from_waker(&new_waker))
            .is_pending()
    );
    assert_eq!(old.state.lock().unwrap().waiters.count, 0);
    drop(old.guard(()));
    assert_eq!(old_wakes.0.load(SeqCst), 0);
    assert_eq!(new_wakes.0.load(SeqCst), 0);
    assert!(
        slot.poll_wait(&observed_new, &mut Context::from_waker(&new_waker))
            .is_pending()
    );
    drop(new.guard(()));
    assert_eq!(new_wakes.0.load(SeqCst), 1);
    assert!(
        slot.poll_wait(&observed_new, &mut Context::from_waker(&new_waker))
            .is_ready()
    );
    slot.cancel();
}

#[test]
fn wake_callback_rearming_same_node_stays_outside_original_cohort() {
    struct Rearm {
        source: ReleaseNotification,
        slot: Mutex<ReleaseRegistration>,
        wakes: AtomicUsize,
        next: Waker,
    }
    impl Wake for Rearm {
        fn wake(self: Arc<Self>) {
            assert!(self.source.state.try_lock().is_ok());
            self.wakes.fetch_add(1, SeqCst);
            let observation = self.source.observe();
            assert!(
                self.slot
                    .lock()
                    .unwrap()
                    .poll_wait(&observation, &mut Context::from_waker(&self.next))
                    .is_pending()
            );
        }
    }
    let budget = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let next = Arc::new(Count::default());
    let owner = Arc::new(Rearm {
        source: ReleaseNotification::default(),
        slot: Mutex::new(registration(&budget)),
        wakes: AtomicUsize::new(0),
        next: Waker::from(Arc::clone(&next)),
    });
    let original_waker = Waker::from(Arc::clone(&owner));
    let observed = owner.source.observe();
    assert!(
        owner
            .slot
            .lock()
            .unwrap()
            .poll_wait(&observed, &mut Context::from_waker(&original_waker))
            .is_pending()
    );
    without_allocations(|| drop(owner.source.guard(())));
    assert_eq!(owner.wakes.load(SeqCst), 1);
    assert_eq!(
        next.0.load(SeqCst),
        0,
        "rearm cannot join the released cohort"
    );
    assert_eq!(owner.source.state.lock().unwrap().waiters.count, 1);
    without_allocations(|| drop(owner.source.guard(())));
    assert_eq!(next.0.load(SeqCst), 1);
    owner.slot.lock().unwrap().cancel();
    drop(original_waker);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn forgotten_borrowed_future_still_unlinks_and_refunds_its_original_registration() {
    let layout = ReleaseRegistration::allocation_layout();
    let source_layout = ReleaseNotification::allocation_layout::<crate::AllocationCharge>();
    let budget = AllocationBudget::new(layout.size() + source_layout.size());
    let mut prepaid = budget.try_reserve_layouts([layout, source_layout]).unwrap();
    let source =
        ReleaseNotification::try_new_charged(prepaid.try_split(source_layout).unwrap()).unwrap();
    let mut slot = ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    drop(prepaid);
    let mut future = source.observe().wait_for_release(&mut slot);
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    std::mem::forget(future);
    assert_eq!(source.state.lock().unwrap().waiters.count, 1);
    drop(slot);
    assert_eq!(source.state.lock().unwrap().waiters.count, 0);
    assert_eq!(budget.reserved_bytes(), source_layout.size());
    drop(source);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn registration_retains_completed_source_until_cancel_and_saturation_is_ready() {
    let layout = ReleaseRegistration::allocation_layout();
    let source_layout = ReleaseNotification::allocation_layout::<crate::AllocationCharge>();
    let budget = AllocationBudget::new(layout.size() + source_layout.size());
    let mut prepaid = budget.try_reserve_layouts([layout, source_layout]).unwrap();
    let source =
        ReleaseNotification::try_new_charged(prepaid.try_split(source_layout).unwrap()).unwrap();
    let mut slot = ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    drop(prepaid);
    source.state.lock().unwrap().sequence = u64::MAX;
    let observed = source.observe();
    assert!(
        slot.poll_wait(&observed, &mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    assert_eq!(source.state.lock().unwrap().waiters.count, 0);
    drop(observed);
    drop(source);
    assert_eq!(
        budget.reserved_bytes(),
        layout.size() + source_layout.size()
    );
    slot.cancel();
    assert_eq!(budget.reserved_bytes(), layout.size());
    drop(slot);
    assert_eq!(budget.reserved_bytes(), 0);
}
