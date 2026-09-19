//! Finite capacity, prepaid splitting and actual EBR reclamation controls.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{Barrier, atomic::Ordering::SeqCst},
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, SeqCst);
    }
}

fn poll(wait: &mut crate::ReleaseFuture, wakes: &Arc<WakeCount>) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::clone(wakes))))
}

fn capacity_wait(error: AllocationRefusal) -> crate::ReleaseFuture {
    let AllocationRefusal::Capacity { release, .. } = error else {
        panic!("expected temporary capacity refusal: {error}");
    };
    release.wait_for_release()
}

fn layout(size: usize) -> Layout {
    Layout::from_size_align(size, 1).unwrap()
}

#[test]
fn finite_limit_overflow_and_zero_never_change_credit_on_refusal() {
    let budget = AllocationBudget::new(10);
    assert_eq!(budget.limit_bytes(), 10);
    assert!(matches!(
        budget.try_reserve(layout(11)),
        Err(AllocationRefusal::ExceedsLimit {
            requested_bytes: 11,
            limit_bytes: 10
        })
    ));
    let largest = layout(isize::MAX as usize);
    assert_eq!(
        budget.try_reserve_layouts([largest; 3]).unwrap_err(),
        AllocationRefusal::DemandOverflow
    );
    assert_eq!(budget.reserved_bytes(), 0);
    let full = budget.try_reserve(layout(10)).unwrap();
    assert!(matches!(
        budget.try_reserve(layout(1)),
        Err(AllocationRefusal::Capacity {
            requested_bytes: 1,
            reserved_bytes: 10,
            limit_bytes: 10,
            ..
        })
    ));
    assert_eq!(budget.reserved_bytes(), 10);
    drop(full);
    assert_eq!(budget.reserved_bytes(), 0);

    let zero = AllocationBudget::new(0);
    let mut empty = zero.try_reserve(layout(0)).unwrap();
    let charge = empty.try_split(layout(0)).unwrap();
    assert_eq!(charge.layout().size(), 0);
    assert!(matches!(
        zero.try_reserve(layout(1)),
        Err(AllocationRefusal::ExceedsLimit { .. })
    ));
    drop(empty);
    drop(charge);
    assert_eq!(zero.reserved_bytes(), 0);
}

#[test]
fn splitting_prepaid_credits_refunds_only_unused_remainder_and_owned_charges() {
    let budget = AllocationBudget::new(24);
    let mut prepaid = budget.try_reserve_layouts([layout(8); 3]).unwrap();
    let first = prepaid.try_split(layout(8)).unwrap();
    let second = prepaid.try_split(layout(8)).unwrap();
    assert_eq!(prepaid.remaining_bytes(), 8);
    assert_eq!(
        prepaid.try_split(layout(9)).unwrap_err(),
        InsufficientReservation {
            requested_bytes: 9,
            remaining_bytes: 8
        }
    );
    assert_eq!(prepaid.remaining_bytes(), 8);
    assert_eq!(budget.reserved_bytes(), 24);
    drop(prepaid);
    assert_eq!(budget.reserved_bytes(), 16);
    let reused = budget.try_reserve(layout(8)).unwrap();
    assert_eq!(budget.reserved_bytes(), 24);
    drop(second);
    assert_eq!(budget.reserved_bytes(), 16);
    drop(first);
    drop(reused);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn exact_pool_release_wakes_waiters_including_before_their_first_poll() {
    let budget = AllocationBudget::new(8);
    let other = AllocationBudget::new(8);
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut registered = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let mut unregistered = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut registered, &wakes).is_pending());
    drop(other.try_reserve(layout(8)).unwrap());
    assert_eq!(wakes.0.load(SeqCst), 0);
    assert!(poll(&mut registered, &wakes).is_pending());
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(wakes.0.load(SeqCst), 1);
    assert!(poll(&mut registered, &wakes).is_ready());
    assert!(poll(&mut unregistered, &wakes).is_ready());
    drop(budget.try_reserve(layout(8)).unwrap());
}

#[test]
fn concurrent_reservations_cannot_oversubscribe_the_same_finite_pool() {
    for _ in 0..16 {
        let budget = AllocationBudget::new(32);
        let barrier = Barrier::new(8);
        let successes = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let budget = &budget;
                let barrier = &barrier;
                let successes = &successes;
                scope.spawn(move || {
                    barrier.wait();
                    let result = budget.try_reserve(layout(32));
                    if result.is_ok() {
                        successes.fetch_add(1, SeqCst);
                    } else {
                        assert!(matches!(result, Err(AllocationRefusal::Capacity { .. })));
                    }
                    barrier.wait();
                    drop(result);
                });
            }
        });
        assert_eq!(successes.load(SeqCst), 1);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn real_epoch_reclamation_returns_capacity_and_its_release_notification() {
    use concread::ebrcell::EbrCell;

    let allocation = EbrCell::<u64, AllocationCharge>::allocation_layout();
    let budget = AllocationBudget::new(allocation.size());
    let mut prepaid = budget.try_reserve(allocation).unwrap();
    let cell = EbrCell::new_charged(7_u64, prepaid.try_split(allocation).unwrap());
    drop(prepaid);
    let unrelated_pin = crossbeam_epoch::pin();
    let mut wait = capacity_wait(budget.try_reserve(allocation).unwrap_err());
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(cell);
    unrelated_pin.flush();
    std::thread::spawn(|| {
        for _ in 0..256 {
            crossbeam_epoch::pin().flush();
        }
    })
    .join()
    .unwrap();
    assert_eq!(budget.reserved_bytes(), allocation.size());
    assert_eq!(wakes.0.load(SeqCst), 0);
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(unrelated_pin);
    let deadline = Instant::now() + Duration::from_secs(5);
    while budget.reserved_bytes() != 0 {
        assert!(
            Instant::now() < deadline,
            "retired allocation was not reclaimed"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
    assert!(poll(&mut wait, &wakes).is_ready());
    drop(budget.try_reserve(allocation).unwrap());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn charge_keeps_original_pool_alive_after_budget_handle_is_dropped() {
    let budget = AllocationBudget::new(8);
    let observer = Arc::downgrade(&budget.pool);
    let mut prepaid = budget.try_reserve(layout(8)).unwrap();
    let charge = prepaid.try_split(layout(8)).unwrap();
    drop(prepaid);
    drop(budget);
    assert_eq!(observer.upgrade().unwrap().reserved.load(SeqCst), 8);
    std::thread::spawn(move || drop(charge)).join().unwrap();
    assert!(observer.upgrade().is_none());
}
