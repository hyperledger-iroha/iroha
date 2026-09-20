//! Finite capacity, prepaid splitting and scoped reclamation notifications.

use super::*;
use std::{
    alloc::{GlobalAlloc, System},
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{Barrier, Mutex, TryLockError, atomic::Ordering::SeqCst},
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

thread_local! {
    static ALLOCATION_COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

struct ObservedAllocator;

fn record_allocation() {
    let _ = ALLOCATION_COUNT.try_with(|count| {
        if let Some(previous) = count.get() {
            count.set(Some(previous.saturating_add(1)));
        }
    });
}

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: forward the unchanged request to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: preserve the original zeroing and layout contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: preserve the live allocation, old layout and requested size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: forward the original allocation and its exact layout.
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn without_allocations<R>(operation: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_COUNT.with(|count| count.set(None));
        }
    }
    assert!(
        ALLOCATION_COUNT
            .with(|count| count.replace(Some(0)))
            .is_none()
    );
    let reset = Reset;
    let output = operation();
    let allocations = ALLOCATION_COUNT.with(|count| count.replace(None)).unwrap();
    drop(reset);
    assert_eq!(allocations, 0, "lexical refund handling allocated storage");
    output
}

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

#[test]
fn nested_original_pool_scopes_return_credits_immediately_and_coalesce_without_allocation() {
    let budget = AllocationBudget::new(8);
    let same_pool = budget.clone();
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut wait = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());

    without_allocations(|| {
        budget.with_deferred_refund_notifications(|| {});
        assert_eq!(wakes.0.load(SeqCst), 0);
        budget.with_deferred_refund_notifications(|| {
            same_pool.with_deferred_refund_notifications(|| {
                drop(owner);
                assert_eq!(budget.reserved_bytes(), 0);
                assert_eq!(wakes.0.load(SeqCst), 0);
            });
            // Refunded capacity is reusable before any deferred notification.
            let reused = budget.try_reserve(layout(8)).unwrap();
            assert_eq!(budget.reserved_bytes(), 8);
            drop(reused);
            assert_eq!(budget.reserved_bytes(), 0);
            assert_eq!(wakes.0.load(SeqCst), 0);
        });
    });
    assert_eq!(wakes.0.load(SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
}

#[test]
fn nested_different_pool_scopes_flush_independently() {
    let first = AllocationBudget::new(8);
    let second = AllocationBudget::new(8);
    let first_owner = first.try_reserve(layout(8)).unwrap();
    let second_owner = second.try_reserve(layout(8)).unwrap();
    let mut first_wait = capacity_wait(first.try_reserve(layout(1)).unwrap_err());
    let mut second_wait = capacity_wait(second.try_reserve(layout(1)).unwrap_err());
    let first_wakes = Arc::new(WakeCount::default());
    let second_wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut first_wait, &first_wakes).is_pending());
    assert!(poll(&mut second_wait, &second_wakes).is_pending());

    first.with_deferred_refund_notifications(|| {
        second.with_deferred_refund_notifications(|| {
            // Finding the exact pool crosses an unrelated inner scope.
            drop(first_owner);
            drop(second_owner);
            assert_eq!(first.reserved_bytes(), 0);
            assert_eq!(second.reserved_bytes(), 0);
            assert_eq!(first_wakes.0.load(SeqCst), 0);
            assert_eq!(second_wakes.0.load(SeqCst), 0);
        });
        assert_eq!(first_wakes.0.load(SeqCst), 0);
        assert_eq!(second_wakes.0.load(SeqCst), 1);
        assert!(poll(&mut second_wait, &second_wakes).is_ready());
    });
    assert_eq!(first_wakes.0.load(SeqCst), 1);
    assert!(poll(&mut first_wait, &first_wakes).is_ready());
}

#[test]
fn another_threads_refund_notifies_while_this_threads_scope_is_still_active() {
    let budget = AllocationBudget::new(8);
    let local = budget.try_reserve(layout(4)).unwrap();
    let remote = budget.try_reserve(layout(4)).unwrap();
    let mut wait = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    budget.with_deferred_refund_notifications(|| {
        drop(local);
        assert_eq!(budget.reserved_bytes(), 4);
        assert_eq!(wakes.0.load(SeqCst), 0);
        std::thread::spawn(move || drop(remote)).join().unwrap();
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(wakes.0.load(SeqCst), 1);
        assert!(poll(&mut wait, &wakes).is_ready());
    });
    assert_eq!(wakes.0.load(SeqCst), 1);
}

#[test]
fn scope_unwind_notifies_after_its_physical_writer_has_unlocked() {
    struct Probe {
        physical: Arc<Mutex<()>>,
        released: AtomicUsize,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            // Report after catch_unwind rather than panicking during an active
            // unwind if notification regresses to running under the lock.
            let observed = match self.physical.try_lock() {
                Err(TryLockError::WouldBlock) => 1,
                Err(TryLockError::Poisoned(_)) => 2,
                Ok(_) => 3,
            };
            self.released.fetch_add(observed, SeqCst);
        }
    }
    let budget = AllocationBudget::new(8);
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut wait = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let physical = Arc::new(Mutex::new(()));
    let wake = Arc::new(Probe {
        physical: Arc::clone(&physical),
        released: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|| {
                let _held = physical.lock().unwrap();
                drop(owner);
                assert_eq!(budget.reserved_bytes(), 0);
                assert_eq!(wake.released.load(SeqCst), 0);
                panic!("original operation failed while its writer was held");
            });
        }))
        .is_err()
    );
    assert_eq!(wake.released.load(SeqCst), 2);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );

    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut next = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let next_wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut next, &next_wakes).is_pending());
    drop(owner);
    assert_eq!(next_wakes.0.load(SeqCst), 1);
    assert!(poll(&mut next, &next_wakes).is_ready());
}

#[test]
fn caught_inner_unwind_remains_deferred_until_the_original_outer_scope_exits() {
    let budget = AllocationBudget::new(8);
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut wait = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    budget.with_deferred_refund_notifications(|| {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                budget.with_deferred_refund_notifications(|| {
                    drop(owner);
                    panic!("inner operation abandoned");
                });
            }))
            .is_err()
        );
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(wakes.0.load(SeqCst), 0);
    });
    assert_eq!(wakes.0.load(SeqCst), 1);
    assert!(poll(&mut wait, &wakes).is_ready());
}

#[test]
fn refund_callback_can_reenter_scopes_and_its_panic_leaves_no_stale_tls_owner() {
    struct Reenter {
        budget: AllocationBudget,
        calls: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            self.budget.with_deferred_refund_notifications(|| {
                let owner = self.budget.try_reserve(layout(8)).unwrap();
                drop(owner);
            });
            self.calls.fetch_add(1, SeqCst);
            panic!("reentrant wake failed after its nested scope completed");
        }
    }
    let budget = AllocationBudget::new(8);
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut wait = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let wake = Arc::new(Reenter {
        budget: budget.clone(),
        calls: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|| drop(owner));
        }))
        .is_err()
    );
    assert_eq!(wake.calls.load(SeqCst), 1);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );

    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut next = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let next_wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut next, &next_wakes).is_pending());
    drop(owner);
    assert_eq!(next_wakes.0.load(SeqCst), 1);
    assert!(poll(&mut next, &next_wakes).is_ready());
}

#[test]
fn deferred_flush_preserves_first_panic_and_wakes_the_remaining_cohort_after_unlock() {
    struct FirstWake(AtomicUsize);
    impl Wake for FirstWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, SeqCst);
            panic!("deferred refund callback panicked");
        }
    }
    struct Survivor {
        physical: Arc<Mutex<()>>,
        wakes: AtomicUsize,
        unlocked: AtomicUsize,
    }
    impl Wake for Survivor {
        fn wake(self: Arc<Self>) {
            // The first callback is already unwinding. Observe without causing
            // a second panic if lock ordering regresses.
            self.unlocked
                .store(usize::from(self.physical.try_lock().is_ok()), SeqCst);
            self.wakes.fetch_add(1, SeqCst);
        }
    }
    let budget = AllocationBudget::new(8);
    let owner = budget.try_reserve(layout(8)).unwrap();
    let mut first = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let mut second = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
    let physical = Arc::new(Mutex::new(()));
    let first_wake = Arc::new(FirstWake(AtomicUsize::new(0)));
    let first_waker = Waker::from(Arc::clone(&first_wake));
    let survivor = Arc::new(Survivor {
        physical: Arc::clone(&physical),
        wakes: AtomicUsize::new(0),
        unlocked: AtomicUsize::new(0),
    });
    let second_waker = Waker::from(Arc::clone(&survivor));
    assert!(
        Pin::new(&mut first)
            .poll(&mut Context::from_waker(&first_waker))
            .is_pending()
    );
    assert!(
        Pin::new(&mut second)
            .poll(&mut Context::from_waker(&second_waker))
            .is_pending()
    );
    let panic = catch_unwind(AssertUnwindSafe(|| {
        budget.with_deferred_refund_notifications(|| {
            let _held = physical.lock().unwrap();
            drop(owner);
            assert_eq!(budget.reserved_bytes(), 0);
            assert_eq!(first_wake.0.load(SeqCst), 0);
            assert_eq!(survivor.wakes.load(SeqCst), 0);
        });
    }))
    .expect_err("deferred callback panic must propagate");
    assert_eq!(
        panic.downcast_ref::<&str>().copied(),
        Some("deferred refund callback panicked")
    );
    assert_eq!(first_wake.0.load(SeqCst), 1);
    assert_eq!(survivor.wakes.load(SeqCst), 1);
    assert_eq!(survivor.unlocked.load(SeqCst), 1);
    assert!(physical.try_lock().is_ok());
    assert!(
        Pin::new(&mut first)
            .poll(&mut Context::from_waker(&first_waker))
            .is_ready()
    );
    assert!(
        Pin::new(&mut second)
            .poll(&mut Context::from_waker(&second_waker))
            .is_ready()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_retired_reader_refund_under_a_new_writer_waits_for_its_scope_to_unlock() {
    use concread::internals::lincowcell::{
        LinCowCell, LinCowCellCapable, WriterAdmission, WriterCharges,
    };

    struct Data(u64);
    impl LinCowCellCapable<u64, u64> for Data {
        type WriterInput = ();

        fn create_reader(&self) -> u64 {
            self.0
        }
        fn create_writer(&self, (): Self::WriterInput) -> u64 {
            self.0
        }
        fn pre_commit(&mut self, value: u64, _previous: &u64) -> u64 {
            self.0 = value;
            value
        }
    }
    type Owner = LinCowCell<Data, u64, u64, AllocationCharge>;
    struct Retry {
        owner: Arc<Owner>,
        wakes: AtomicUsize,
    }
    impl Wake for Retry {
        fn wake(self: Arc<Self>) {
            assert_eq!(*self.owner.read(), 11);
            assert!(matches!(
                self.owner
                    .try_write_charged(|_, _| Err::<WriterAdmission<AllocationCharge, ()>, _>(())),
                Err(())
            ));
            self.wakes.fetch_add(1, SeqCst);
        }
    }
    let layouts = Owner::writer_allocation_layouts();
    let budget = AllocationBudget::new(3 * layouts.reader.size() + layouts.cursor.size());
    let mut initial = budget.try_reserve(layouts.reader).unwrap();
    let owner = Arc::new(Owner::new_charged(
        Data(7),
        initial.try_split(layouts.reader).unwrap(),
    ));
    drop(initial);
    let oldest = owner.read();
    let admit = |_: &Data, layouts: concread::internals::lincowcell::WriterLayouts| {
        let mut prepaid = budget.try_reserve_layouts([layouts.cursor, layouts.reader])?;
        Ok::<_, AllocationRefusal>(WriterAdmission {
            charges: WriterCharges {
                cursor: prepaid.try_split(layouts.cursor).unwrap(),
                reader: prepaid.try_split(layouts.reader).unwrap(),
            },
            input: (),
        })
    };
    let mut writer = owner.write_charged(admit).unwrap();
    *writer = 11;
    writer.commit();
    let wake = Arc::new(Retry {
        owner: Arc::clone(&owner),
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut wait = None;
    budget.with_deferred_refund_notifications(|| {
        let held = owner.write_charged(admit).unwrap();
        let mut pending = capacity_wait(budget.try_reserve(layout(1)).unwrap_err());
        assert!(Pin::new(&mut pending).poll(&mut context).is_pending());
        wait = Some(pending);
        drop(oldest);
        assert_eq!(
            budget.reserved_bytes(),
            2 * layouts.reader.size() + layouts.cursor.size()
        );
        assert_eq!(wake.wakes.load(SeqCst), 0);
        drop(held);
        assert_eq!(budget.reserved_bytes(), layouts.reader.size());
        assert_eq!(wake.wakes.load(SeqCst), 0);
    });
    assert_eq!(wake.wakes.load(SeqCst), 1);
    assert!(
        Pin::new(wait.as_mut().unwrap())
            .poll(&mut context)
            .is_ready()
    );
    drop(wait);
    drop(waker);
    drop(wake);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn checked_aggregate_bytes_need_no_fabricated_single_allocation_layout() {
    let bytes = (isize::MAX as usize) + 1;
    assert!(Layout::from_size_align(bytes, 1).is_err());
    let budget = AllocationBudget::new(bytes);
    let mut reservation = without_allocations(|| budget.try_reserve_bytes(bytes).unwrap());
    let first = reservation.try_split(layout(isize::MAX as usize)).unwrap();
    let second = reservation.try_split(layout(1)).unwrap();
    assert_eq!(reservation.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(matches!(
        budget.try_reserve_bytes(1),
        Err(AllocationRefusal::Capacity { .. })
    ));
    drop(reservation);
    drop(first);
    assert_eq!(budget.reserved_bytes(), 1);
    drop(second);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(matches!(
        budget.try_reserve_bytes(bytes + 1),
        Err(AllocationRefusal::ExceedsLimit { .. })
    ));
}
