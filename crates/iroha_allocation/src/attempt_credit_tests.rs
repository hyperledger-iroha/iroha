//! An attempt's own rollback refunds cannot wake its original capacity refusal.

use super::*;
use crate::{
    release::{ReleaseRegistration, ReleaseWait},
    test_support::without_allocations,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    task::{Context, Poll, Waker},
};

const LIMIT: usize = 4096;

fn registration(budget: &AllocationBudget) -> ReleaseRegistration {
    ReleaseRegistration::from_reservation(
        &mut budget
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap()
}

/// One callback and one native-thread registration observe both release sequences.
struct Waiters {
    callback: ReleaseRegistration,
    thread: ReleaseRegistration,
}

impl Waiters {
    fn new(budget: &AllocationBudget) -> Self {
        let callback = registration(budget);
        let mut thread = registration(budget);
        thread.bind_thread(std::thread::current());
        Self { callback, thread }
    }

    fn ready(&mut self, wait: &ReleaseWait) -> (bool, bool) {
        let mut context = Context::from_waker(Waker::noop());
        (
            self.callback.poll_wait(wait, &mut context) == Poll::Ready(()),
            self.thread.poll_wait(wait, &mut context) == Poll::Ready(()),
        )
    }
}

fn capacity_release(refusal: AllocationRefusal) -> ReleaseWait {
    let AllocationRefusal::Capacity { release, .. } = refusal else {
        panic!("expected temporary capacity refusal: {refusal}");
    };
    release
}

#[test]
fn refused_attempt_keeps_its_own_refunds_until_an_external_release() {
    let budget = AllocationBudget::new(LIMIT);
    let mut waiters = Waiters::new(&budget);
    let baseline = budget.reserved_bytes();
    let blocker = budget.try_reserve_bytes(LIMIT - baseline - 64).unwrap();
    let (release, retained) = budget.with_attempt_credits(None, || {
        // A transient refund stays with the attempt and pays for its next reservation.
        drop(budget.try_reserve_bytes(16).unwrap());
        assert_eq!(budget.pool.reserved.load(Ordering::Acquire), LIMIT - 48);
        assert_eq!(budget.reserved_bytes(), LIMIT - 64);
        let prefix = without_allocations(|| budget.try_reserve_bytes(64).unwrap());
        assert_eq!(budget.reserved_bytes(), LIMIT);
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        assert!(matches!(
            refusal,
            AllocationRefusal::Capacity {
                requested_bytes: 1,
                reserved_bytes: LIMIT,
                ..
            }
        ));
        // The rolled-back prefix stays with the refused attempt.
        drop(prefix);
        assert_eq!(budget.pool.reserved.load(Ordering::Acquire), LIMIT);
        assert_eq!(budget.reserved_bytes(), LIMIT - 64);
        capacity_release(refusal)
    });
    let retained = retained.expect("the refused attempt keeps its rolled-back prefix");
    assert_eq!(retained.retained_bytes(), 64);
    assert!(retained.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), LIMIT);
    assert_eq!(
        waiters.ready(&release),
        (false, false),
        "an attempt's own rollback cannot satisfy its original release wait"
    );
    let foreign = AllocationBudget::new(8);
    drop(foreign.try_reserve_bytes(8).unwrap());
    assert_eq!(waiters.ready(&release), (false, false));
    drop(blocker);
    assert_eq!(waiters.ready(&release), (true, true));
    drop(retained);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn retry_spends_retained_credits_before_the_pool_and_keeps_them_on_refusal() {
    let budget = AllocationBudget::new(LIMIT);
    let mut waiters = Waiters::new(&budget);
    let baseline = budget.reserved_bytes();
    let blocker = budget.try_reserve_bytes(LIMIT - baseline - 64).unwrap();
    let ((), first) = budget.with_attempt_credits(None, || {
        let prefix = budget.try_reserve_bytes(64).unwrap();
        assert!(budget.try_reserve_bytes(32).is_err());
        drop(prefix);
    });
    let first = first.unwrap();
    // The same job's next attempt reaches the same refusal on its own credits.
    let (release, second) = budget.with_attempt_credits(Some(first), || {
        let prefix = without_allocations(|| budget.try_reserve_bytes(64).unwrap());
        assert_eq!(budget.reserved_bytes(), LIMIT);
        let refusal = budget.try_reserve_bytes(32).unwrap_err();
        assert!(matches!(
            refusal,
            AllocationRefusal::Capacity {
                requested_bytes: 32,
                reserved_bytes: LIMIT,
                limit_bytes: LIMIT,
                ..
            }
        ));
        drop(prefix);
        capacity_release(refusal)
    });
    let second = second.unwrap();
    assert_eq!(second.retained_bytes(), 64);
    assert_eq!(budget.reserved_bytes(), LIMIT);
    assert_eq!(waiters.ready(&release), (false, false));
    // An external release lets the next attempt spend its credits, then the pool.
    drop(blocker);
    assert_eq!(waiters.ready(&release), (true, true));
    let other_owner = budget.pool.released.observe();
    let ((), third) = budget.with_attempt_credits(Some(second), || {
        let prefix = budget.try_reserve_bytes(64 + 32).unwrap();
        assert_eq!(budget.reserved_bytes(), baseline + 64 + 32);
        drop(prefix);
    });
    assert_eq!(
        waiters.ready(&other_owner),
        (false, false),
        "other owners wake when the attempt returns its credits, not before"
    );
    let third = third.unwrap();
    assert_eq!(third.retained_bytes(), 96);
    drop(third);
    assert_eq!(waiters.ready(&other_owner), (true, true));
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn foreign_credits_unwind_and_other_pools_keep_ordinary_refunds() {
    let budget = AllocationBudget::new(128);
    let other = AllocationBudget::new(128);
    let other_blocker = other.try_reserve_bytes(96).unwrap();
    let ((), foreign) = other.with_attempt_credits(None, || {
        let prefix = other.try_reserve_bytes(32).unwrap();
        assert!(other.try_reserve_bytes(1).is_err());
        drop(prefix);
    });
    drop(other_blocker);
    let foreign = foreign.unwrap();
    assert_eq!(other.reserved_bytes(), 32);
    // Credits of another pool return to that pool before this scope starts.
    let ((), kept) = budget.with_attempt_credits(Some(foreign), || {
        assert_eq!(other.reserved_bytes(), 0);
        let mine = budget.try_reserve_bytes(96).unwrap();
        assert!(budget.try_reserve_bytes(64).is_err());
        // This pool's refusal does not keep another pool's refunds.
        drop(other.try_reserve_bytes(8).unwrap());
        assert_eq!(other.reserved_bytes(), 0);
        // Kept credits pay for later reservations of the same attempt.
        drop(mine);
        assert_eq!(budget.pool.reserved.load(Ordering::Acquire), 96);
        assert_eq!(budget.reserved_bytes(), 0);
        let spent = budget.try_reserve_bytes(16).unwrap();
        assert_eq!(budget.pool.reserved.load(Ordering::Acquire), 96);
        assert_eq!(budget.reserved_bytes(), 16);
        drop(spent);
    });
    assert_eq!(kept.unwrap().retained_bytes(), 96);
    assert_eq!(budget.reserved_bytes(), 0);
    // An unwind abandons the attempt and returns its kept credits.
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        budget.with_attempt_credits(None, || {
            let prefix = budget.try_reserve_bytes(128).unwrap();
            assert!(budget.try_reserve_bytes(1).is_err());
            drop(prefix);
            assert_eq!(budget.pool.reserved.load(Ordering::Acquire), 128);
            panic!("abandoned attempt");
        })
    }));
    assert!(unwound.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(AttemptCreditScope::innermost(&budget.pool).is_none());
}

#[test]
fn exhausting_the_pool_inside_an_attempt_also_spends_its_kept_credits() {
    let budget = AllocationBudget::new(LIMIT);
    let other = budget.try_reserve_bytes(LIMIT / 2).unwrap();
    let ((), retained) = budget.with_attempt_credits(None, || {
        drop(budget.try_reserve_bytes(64).unwrap());
        assert_eq!(budget.reserved_bytes(), LIMIT / 2);
        let all = budget
            .try_reserve_bytes(LIMIT - budget.reserved_bytes())
            .unwrap();
        assert_eq!(budget.reserved_bytes(), LIMIT);
        assert!(matches!(
            budget.try_reserve_bytes(1),
            Err(AllocationRefusal::Capacity {
                reserved_bytes: LIMIT,
                ..
            })
        ));
        drop(all);
    });
    assert_eq!(retained.unwrap().retained_bytes(), LIMIT - LIMIT / 2);
    drop(other);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn refunds_on_other_threads_still_wake_the_refusal() {
    let budget = AllocationBudget::new(LIMIT);
    let mut waiters = Waiters::new(&budget);
    let held = budget
        .try_reserve_bytes(LIMIT - budget.reserved_bytes())
        .unwrap();
    let (release, retained) = budget.with_attempt_credits(None, || {
        let release = capacity_release(budget.try_reserve_bytes(1).unwrap_err());
        std::thread::scope(|threads| {
            threads.spawn(move || drop(held));
        });
        release
    });
    assert!(retained.is_none());
    assert_eq!(waiters.ready(&release), (true, true));
}
