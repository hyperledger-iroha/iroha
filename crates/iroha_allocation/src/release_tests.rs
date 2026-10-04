//! Release observations survive registration races without owning data locks.

use super::*;
use crate::AllocationBudget;

fn registrations<const N: usize>(budget: &AllocationBudget) -> [ReleaseRegistration; N] {
    std::array::from_fn(|_| {
        ReleaseRegistration::from_reservation(
            &mut budget
                .try_reserve(ReleaseRegistration::allocation_layout())
                .unwrap(),
        )
        .unwrap()
    })
}
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Wake,
};

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(wait: &mut ReleaseFuture<'_>, wake: &Arc<WakeCount>) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::clone(wake))))
}
#[test]
fn release_before_registration_is_retained_and_other_sources_do_not_wake() {
    let registration_budget =
        AllocationBudget::new(2 * ReleaseRegistration::allocation_layout().size());
    let [mut registration_0, mut registration_1] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let other = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.guard(lock.lock().unwrap());
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let wake = Arc::new(WakeCount::default());
    drop(other.guard(()));
    assert!(poll(&mut wait, &wake).is_pending());
    let mut before_first_poll = source.observe().wait_for_release(&mut registration_1);
    drop(guard);
    assert!(lock.try_lock().is_ok());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
    assert!(poll(&mut before_first_poll, &wake).is_ready());
}

#[test]
fn first_registered_wake_reenters_unlocked_source_after_original_node_is_detached() {
    struct FirstWake {
        source: Arc<ReleaseNotification>,
        calls: AtomicUsize,
    }
    impl Wake for FirstWake {
        fn wake(self: Arc<Self>) {
            assert!(self.source.state.try_lock().is_ok());
            assert_eq!(self.source.state.lock().unwrap().waiters.count, 0);
            drop(self.source.observe());
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);

    let source = Arc::new(ReleaseNotification::default());
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let wake = Arc::new(FirstWake {
        source: Arc::clone(&source),
        calls: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(source.guard(()));
    assert_eq!(wake.calls.load(Ordering::SeqCst), 1);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn panicking_first_waker_still_notifies_the_remaining_original_cohort() {
    struct FirstWake {
        source: Arc<ReleaseNotification>,
        calls: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
        drop_was_unlocked: Arc<AtomicBool>,
        panic_in_drop: bool,
    }
    impl Wake for FirstWake {
        fn wake(self: Arc<Self>) {
            assert!(self.source.state.try_lock().is_ok());
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(self.panic_in_drop, "first wake callback panicked");
        }
    }
    impl Drop for FirstWake {
        fn drop(&mut self) {
            // Never assert during an existing unwind: record lock ordering for
            // the caller to inspect after catching the original callback panic.
            self.drop_was_unlocked
                .store(self.source.state.try_lock().is_ok(), Ordering::SeqCst);
            self.drops.fetch_add(1, Ordering::SeqCst);
            assert!(!self.panic_in_drop, "first wake destructor panicked");
        }
    }

    let registration_budget =
        AllocationBudget::new(2 * ReleaseRegistration::allocation_layout().size());
    let [mut registration_0, mut registration_1] = registrations(&registration_budget);

    for panic_in_drop in [false, true] {
        let source = Arc::new(ReleaseNotification::default());
        let observation = source.observe();
        let mut first = observation.clone().wait_for_release(&mut registration_0);
        let mut survivor = observation.wait_for_release(&mut registration_1);
        let calls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let drop_was_unlocked = Arc::new(AtomicBool::new(false));
        let first_waker = Waker::from(Arc::new(FirstWake {
            source: Arc::clone(&source),
            calls: Arc::clone(&calls),
            drops: Arc::clone(&drops),
            drop_was_unlocked: Arc::clone(&drop_was_unlocked),
            panic_in_drop,
        }));
        assert!(
            Pin::new(&mut first)
                .poll(&mut Context::from_waker(&first_waker))
                .is_pending()
        );
        // The registration owns the final strong waker reference. Its consumed
        // wake must also run the destructor outside notification locks.
        drop(first_waker);
        let survivor_wakes = Arc::new(WakeCount::default());
        assert!(poll(&mut survivor, &survivor_wakes).is_pending());

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(source.guard(()));
        }))
        .expect_err("the original callback panic must propagate");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some(if panic_in_drop {
                "first wake destructor panicked"
            } else {
                "first wake callback panicked"
            })
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(drop_was_unlocked.load(Ordering::SeqCst));
        assert_eq!(survivor_wakes.0.load(Ordering::SeqCst), 1);
        let completed_wakes = Arc::new(WakeCount::default());
        assert!(poll(&mut first, &completed_wakes).is_ready());
        assert!(poll(&mut survivor, &survivor_wakes).is_ready());
        assert!(source.state.lock().unwrap().waiters.count == 0);
        drop(source.guard(()));
        assert_eq!(survivor_wakes.0.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn cancellation_and_waker_replacement_do_not_steal_another_wait() {
    let registration_budget =
        AllocationBudget::new(3 * ReleaseRegistration::allocation_layout().size());
    let [mut registration_0, mut registration_1, mut registration_2] =
        registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let observation = source.observe();
    let mut canceled = observation.clone().wait_for_release(&mut registration_0);
    let mut retained = observation.wait_for_release(&mut registration_1);
    let old = Arc::new(WakeCount::default());
    let new = Arc::new(WakeCount::default());
    assert!(poll(&mut canceled, &old).is_pending());
    assert!(poll(&mut retained, &old).is_pending());
    assert!(poll(&mut retained, &new).is_pending());
    drop(canceled);
    assert_eq!(source.state.lock().unwrap().waiters.count, 1);
    drop(source.guard(()));
    assert_eq!(old.0.load(Ordering::SeqCst), 0);
    assert_eq!(new.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut retained, &new).is_ready());
    assert!(source.state.lock().unwrap().waiters.count == 0);
    let mut canceled = source.observe().wait_for_release(&mut registration_2);
    assert!(poll(&mut canceled, &old).is_pending());
    drop(canceled);
    assert_eq!(source.state.lock().unwrap().waiters.count, 0);
}

struct ObserveOnDrop {
    source: Arc<ReleaseNotification>,
    state_was_unlocked: Arc<AtomicBool>,
    drops: Arc<AtomicUsize>,
}

impl Wake for ObserveOnDrop {
    fn wake(self: Arc<Self>) {}
}

impl Drop for ObserveOnDrop {
    fn drop(&mut self) {
        // Detect the deadlock without blocking the test on a reentrant observe.
        let unlocked = self.source.state.try_lock().is_ok();
        self.state_was_unlocked.store(unlocked, Ordering::SeqCst);
        self.drops.fetch_add(1, Ordering::SeqCst);
        if unlocked {
            drop(self.source.observe());
        }
    }
}

#[test]
fn replacing_a_waker_allows_its_destructor_to_observe_the_same_source() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = Arc::new(ReleaseNotification::default());
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let state_was_unlocked = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Waker::from(Arc::new(ObserveOnDrop {
        source: Arc::clone(&source),
        state_was_unlocked: Arc::clone(&state_was_unlocked),
        drops: Arc::clone(&drops),
    }));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&old))
            .is_pending()
    );
    drop(old);

    let replacement = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &replacement).is_pending());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(state_was_unlocked.load(Ordering::SeqCst));
    drop(source.guard(()));
    assert_eq!(replacement.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &replacement).is_ready());
}

#[test]
fn ready_wait_releases_its_last_waker_outside_the_notification_lock() {
    use std::sync::mpsc;

    struct PausedWake {
        started: mpsc::SyncSender<()>,
        resume: Mutex<mpsc::Receiver<()>>,
    }
    impl Wake for PausedWake {
        fn wake(self: Arc<Self>) {
            self.started.send(()).unwrap();
            self.resume.lock().unwrap().recv().unwrap();
        }
    }

    let registration_budget =
        AllocationBudget::new(2 * ReleaseRegistration::allocation_layout().size());
    let [mut registration_0, mut registration_1] = registrations(&registration_budget);

    let source = Arc::new(ReleaseNotification::default());
    let observation = source.observe();
    let mut first = observation.clone().wait_for_release(&mut registration_0);
    let mut ready = observation.wait_for_release(&mut registration_1);
    let (started, started_rx) = mpsc::sync_channel(0);
    let (resume, resume_rx) = mpsc::sync_channel(0);
    let first_waker = Waker::from(Arc::new(PausedWake {
        started,
        resume: Mutex::new(resume_rx),
    }));
    assert!(
        Pin::new(&mut first)
            .poll(&mut Context::from_waker(&first_waker))
            .is_pending()
    );

    let state_was_unlocked = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Waker::from(Arc::new(ObserveOnDrop {
        source: Arc::clone(&source),
        state_was_unlocked: Arc::clone(&state_was_unlocked),
        drops: Arc::clone(&drops),
    }));
    assert!(
        Pin::new(&mut ready)
            .poll(&mut Context::from_waker(&old))
            .is_pending()
    );
    drop(old);

    std::thread::scope(|scope| {
        let source = Arc::clone(&source);
        let release = scope.spawn(move || drop(source.guard(())));
        // The release advanced the sequence and took both registrations, but
        // its first callback prevents it from upgrading the second weak owner.
        started_rx.recv().unwrap();
        let result = poll(&mut ready, &Arc::new(WakeCount::default()));
        resume.send(()).unwrap();
        release.join().unwrap();
        assert!(result.is_ready());
    });
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(state_was_unlocked.load(Ordering::SeqCst));
}

#[test]
fn release_racing_first_poll_cannot_be_lost() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    for _ in 0..128 {
        let source = ReleaseNotification::default();
        let mut wait = source.observe().wait_for_release(&mut registration_0);
        let wake = Arc::new(WakeCount::default());
        std::thread::scope(|scope| {
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let release_barrier = Arc::clone(&barrier);
            let source = &source;
            let release = scope.spawn(move || {
                let guard = source.guard(());
                release_barrier.wait();
                drop(guard);
            });
            barrier.wait();
            let first = poll(&mut wait, &wake);
            release.join().unwrap();
            if first.is_pending() {
                assert_eq!(wake.0.load(Ordering::SeqCst), 1);
            }
            assert!(poll(&mut wait, &wake).is_ready());
        });
    }
}

#[test]
fn ownership_phase_transfer_defers_original_release_until_final_owner_drops() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.poisoning_guard(lock.lock().unwrap());
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    let guard = guard.map_preserving_release(|guard| (guard, 7));
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    assert!(lock.try_lock().is_err());
    let guard = guard.map_preserving_release(|(guard, value)| {
        drop(guard);
        value
    });
    assert!(lock.try_lock().is_ok());
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    drop(guard);
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
}

#[test]
fn ownership_phase_transfer_unwind_releases_and_poisons_original_observation() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.poisoning_guard(lock.lock().unwrap());
    let observation = source.observe();
    let mut wait = observation.clone().wait_for_release(&mut registration_0);
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            guard.map_preserving_release::<()>(|guard| {
                drop(guard);
                panic!("phase transfer refused");
            });
        }))
        .is_err()
    );
    assert!(lock.try_lock().is_ok());
    assert!(observation.is_poisoned());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
}

#[test]
fn physical_release_disarms_only_later_retirement_poisoning() {
    struct PanickingRetirement;
    impl Drop for PanickingRetirement {
        fn drop(&mut self) {
            panic!("cleanup after physical release");
        }
    }

    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);

    for released in [false, true] {
        let source = ReleaseNotification::default();
        let lock = Mutex::new(());
        let guard = source.poisoning_guard(lock.lock().unwrap());
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release(&mut registration_0);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let retirement = guard.release_retaining(|guard| {
                    assert!(released, "failure with physical owner still held");
                    drop(guard);
                    PanickingRetirement
                });
                assert_eq!(wake.0.load(Ordering::SeqCst), 0);
                drop(retirement);
            }))
            .is_err()
        );
        assert_eq!(lock.is_poisoned(), !released);
        assert_eq!(observation.is_poisoned(), !released);
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &wake).is_ready());
    }
}

#[test]
fn paired_release_uses_actual_poison_and_unlocks_both_before_callback_unwind() {
    struct Tail(bool);
    impl Drop for Tail {
        fn drop(&mut self) {
            assert!(!self.0, "payload cleanup after physical unlock");
        }
    }
    struct Owner<'a> {
        _lock: std::sync::MutexGuard<'a, ()>,
        _tail: Tail,
    }
    struct Probe {
        first: Arc<Mutex<()>>,
        second: Arc<Mutex<()>>,
        calls: AtomicUsize,
        panic_once: AtomicBool,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            for lock in [&self.first, &self.second] {
                assert!(
                    !matches!(lock.try_lock(), Err(std::sync::TryLockError::WouldBlock)),
                    "both physical owners must already be released"
                );
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                !self.panic_once.swap(false, Ordering::SeqCst),
                "wake interruption"
            );
        }
    }

    let registration_budget =
        AllocationBudget::new(2 * ReleaseRegistration::allocation_layout().size());
    let [mut registration_0, mut registration_1] = registrations(&registration_budget);
    for mode in 0..4 {
        let first = Arc::new(Mutex::new(()));
        let second = Arc::new(Mutex::new(()));
        let source_a = ReleaseNotification::default();
        let source_b = ReleaseNotification::default();
        let a = source_a.poisoning_guard(Owner {
            _lock: first.lock().unwrap(),
            _tail: Tail(mode == 2),
        });
        let b = source_b.poisoning_guard(Owner {
            _lock: second.lock().unwrap(),
            _tail: Tail(false),
        });
        let probe = Arc::new(Probe {
            first: Arc::clone(&first),
            second: Arc::clone(&second),
            calls: AtomicUsize::new(0),
            panic_once: AtomicBool::new(mode == 3),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut wait_a = std::pin::pin!(source_a.observe().wait_for_release(&mut registration_0));
        let mut wait_b = std::pin::pin!(source_b.observe().wait_for_release(&mut registration_1));
        assert!(wait_a.as_mut().poll(&mut context).is_pending());
        assert!(wait_b.as_mut().poll(&mut context).is_pending());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            a.release_pair_with(
                b,
                |a, b| {
                    let owners = (a, b);
                    assert!(mode != 1, "caller interruption while both locks are held");
                    drop(owners);
                    73
                },
                || (first.is_poisoned(), second.is_poisoned()),
            )
        }));
        assert_eq!(result.is_err(), mode != 0);
        if mode == 0 {
            assert_eq!(result.unwrap(), 73);
        }
        let expected = match mode {
            1 => (true, true),
            2 => (false, true),
            _ => (false, false),
        };
        assert_eq!((first.is_poisoned(), second.is_poisoned()), expected);
        assert_eq!(
            (
                source_a.observe().is_poisoned(),
                source_b.observe().is_poisoned()
            ),
            expected
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
        assert!(wait_a.as_mut().poll(&mut context).is_ready());
        assert!(wait_b.as_mut().poll(&mut context).is_ready());
    }
}

#[test]
fn observed_release_reports_existing_physical_poison_and_excludes_later_wake_panic() {
    struct Probe {
        lock: Arc<Mutex<()>>,
        unavailable: AtomicBool,
        calls: AtomicUsize,
        panic: bool,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            self.unavailable.store(
                matches!(
                    self.lock.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ),
                Ordering::SeqCst,
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(!self.panic, "wake after physical release");
        }
    }

    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    for already_poisoned in [false, true] {
        for panic_after_release in [false, true] {
            let lock = Arc::new(Mutex::new(()));
            if already_poisoned {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _held = lock.lock().unwrap();
                    panic!("preexisting physical poison");
                }));
            }
            let source = ReleaseNotification::default();
            let before = source.observe();
            let mut wait = before.clone().wait_for_release(&mut registration_0);
            let probe = Arc::new(Probe {
                lock: Arc::clone(&lock),
                unavailable: AtomicBool::new(false),
                calls: AtomicUsize::new(0),
                panic: panic_after_release,
            });
            let waker = Waker::from(Arc::clone(&probe));
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            let owner = source.poisoning_guard(
                lock.lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                owner.release_with_observed_poison(drop, || lock.is_poisoned());
            }));
            assert_eq!(result.is_err(), panic_after_release);
            assert!(!probe.unavailable.load(Ordering::SeqCst));
            assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
            assert_eq!(before.is_poisoned(), already_poisoned);
            assert_eq!(lock.is_poisoned(), already_poisoned);
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
            );
        }
    }
}

#[test]
fn pair_construction_transfers_both_original_guards_without_early_release() {
    let registration_budget =
        AllocationBudget::new(2 * ReleaseRegistration::allocation_layout().size());
    let [mut registration_0, mut registration_1] = registrations(&registration_budget);
    let first = Mutex::new(());
    let second = Mutex::new(());
    let source_a = ReleaseNotification::default();
    let source_b = ReleaseNotification::default();
    let a = source_a.poisoning_guard(first.lock().unwrap());
    let b = source_b.poisoning_guard(second.lock().unwrap());
    let before_a = source_a.observe();
    let before_b = source_b.observe();
    let wake = Arc::new(WakeCount::default());
    let mut wait_a = before_a.clone().wait_for_release(&mut registration_0);
    let mut wait_b = before_b.clone().wait_for_release(&mut registration_1);
    assert!(poll(&mut wait_a, &wake).is_pending());
    assert!(poll(&mut wait_b, &wake).is_pending());
    let (a, b) = a
        .try_map_pair_preserving_release(
            b,
            |a, b| Ok::<_, ()>(((a, 7), (b, 11))),
            || panic!("successful construction releases neither guard"),
        )
        .unwrap();
    assert_eq!(a.1, 7);
    assert_eq!(b.1, 11);
    assert!(first.try_lock().is_err());
    assert!(second.try_lock().is_err());
    assert_eq!(source_a.observe(), before_a);
    assert_eq!(source_b.observe(), before_b);
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    a.release_pair_with(
        b,
        |a, b| drop((a, b)),
        || (first.is_poisoned(), second.is_poisoned()),
    );
    assert!(poll(&mut wait_a, &wake).is_ready());
    assert!(poll(&mut wait_b, &wake).is_ready());
    assert_eq!(wake.0.load(Ordering::SeqCst), 2);
    assert!(!source_a.observe().is_poisoned());
    assert!(!source_b.observe().is_poisoned());
}

#[test]
fn deferred_release_keeps_original_wait_and_ignores_later_cleanup_unwind() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    for panic_after in [false, true] {
        let source = ReleaseNotification::default();
        let physical = Mutex::new(());
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release(&mut registration_0);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        let deferred = source
            .poisoning_guard(physical.lock().unwrap())
            .release_deferred(drop);
        assert!(physical.try_lock().is_ok());
        drop(source);
        assert!(poll(&mut wait, &wake).is_pending());
        assert_eq!(wake.0.load(Ordering::SeqCst), 0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let retained = deferred;
            assert!(!panic_after, "later aggregate cleanup failed");
            drop(retained);
        }));
        assert_eq!(result.is_err(), panic_after);
        assert!(poll(&mut wait, &wake).is_ready());
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(!physical.is_poisoned());
        assert!(!observation.is_poisoned());
    }
}

#[test]
fn fallible_phase_transfer_retains_the_original_guard_and_owned_cleanup() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    #[expect(
        clippy::mutex_integer,
        reason = "exercise physical guard custody and original value through transfer"
    )]
    let physical = Mutex::new(7_u64);
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    let guard = source.poisoning_guard(physical.lock().unwrap());
    let original = std::ptr::from_ref(&**guard);
    let (guard, error) = guard
        .try_map_preserving_release(|guard| Err::<(), _>((guard, "busy")))
        .err()
        .unwrap();
    assert_eq!(error, "busy");
    assert_eq!(std::ptr::from_ref(&**guard), original);
    assert!(physical.try_lock().is_err());
    assert!(poll(&mut wait, &wake).is_pending());
    let guard = guard
        .try_map_preserving_release(|guard| Ok::<_, (_, ())>((guard, 41)))
        .unwrap_or_else(|_| panic!("original guard transfers"));
    let (retained, release) = guard.release_deferred(|(guard, retained)| {
        drop(guard);
        retained
    });
    assert_eq!(retained, 41);
    assert_eq!(*physical.try_lock().unwrap(), 7);
    assert!(poll(&mut wait, &wake).is_pending());
    drop(release);
    assert!(poll(&mut wait, &wake).is_ready());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
}

#[test]
fn release_batch_empty_and_foreign_transfer_preserve_original_custody() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let foreign = ReleaseNotification::default();
    #[expect(
        clippy::mutex_integer,
        reason = "exercise physical guard custody and original value through transfer"
    )]
    let physical = Mutex::new(7_u64);
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    drop(source.deferred_batch());
    assert!(poll(&mut wait, &wake).is_pending());
    let mut wrong = foreign.deferred_batch();
    let guard = source.guard(physical.lock().unwrap());
    let original = std::ptr::from_ref(&**guard);
    let Err(guard) = guard.try_release_into(&mut wrong, |_| -> () {
        panic!("foreign transfer called release")
    }) else {
        panic!("foreign batch returns original guard");
    };
    assert_eq!(std::ptr::from_ref(&**guard), original);
    assert!(physical.try_lock().is_err());
    drop(wrong);
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    let mut exact = source.deferred_batch();
    assert!(guard.try_release_into(&mut exact, drop).is_ok());
    assert!(physical.try_lock().is_ok());
    assert!(poll(&mut wait, &wake).is_pending());
    drop(source);
    drop(exact);
    assert!(poll(&mut wait, &wake).is_ready());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
}

#[test]
fn release_batch_coalesces_reacquisitions_without_allocating_or_early_wakes() {
    use crate::test_support::without_allocations;
    struct Reenter {
        source: Arc<ReleaseNotification>,
        physical: Arc<Mutex<()>>,
        outer: Arc<Mutex<()>>,
        wakes: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert!(self.physical.try_lock().is_ok());
            assert!(self.outer.try_lock().is_ok());
            assert!(self.source.state.try_lock().is_ok());
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }

    let registration_budget =
        AllocationBudget::new(5 * ReleaseRegistration::allocation_layout().size());
    let [
        mut registration_0,
        mut registration_1,
        mut registration_2,
        mut registration_3,
        mut registration_4,
    ] = registrations(&registration_budget);
    let source = Arc::new(ReleaseNotification::default());
    let physical = Arc::new(Mutex::new(()));
    // Initialize platform mutex storage before measuring release bookkeeping.
    drop(physical.lock().unwrap());
    let outer = Arc::new(Mutex::new(()));
    let outer_guard = outer.lock().unwrap();
    let mut batch = without_allocations(|| source.deferred_batch());
    let mut first = source.observe().wait_for_release(&mut registration_0);
    let callback = Arc::new(Reenter {
        source: Arc::clone(&source),
        physical: Arc::clone(&physical),
        outer: Arc::clone(&outer),
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    assert!(
        Pin::new(&mut first)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let references = source.state.strong_count();
    for _ in 0..128 {
        without_allocations(|| {
            let guard = source.guard(physical.lock().unwrap());
            assert!(guard.try_release_into(&mut batch, drop).is_ok());
        });
    }
    assert_eq!(source.state.strong_count(), references);
    assert_eq!(source.state.lock().unwrap().sequence, 0);
    // Observation after a reacquisition still waits on this same deferred cut.
    let guard = source.guard(physical.lock().unwrap());
    let mut later = source.observe().wait_for_release(&mut registration_1);
    let before_first_poll = source.observe();
    let mut canceled = source.observe().wait_for_release(&mut registration_2);
    for wait in [&mut later, &mut canceled] {
        assert!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    drop(canceled);
    without_allocations(|| assert!(guard.try_release_into(&mut batch, drop).is_ok()));
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
    drop(outer_guard);
    without_allocations(|| drop(batch));
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 2);
    assert_eq!(source.state.lock().unwrap().sequence, 1);
    for wait in [
        &mut first,
        &mut later,
        &mut before_first_poll.wait_for_release(&mut registration_3),
    ] {
        assert!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
    }
    let mut successor = source.observe().wait_for_release(&mut registration_4);
    assert!(
        Pin::new(&mut successor)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(source.guard(physical.lock().unwrap()));
    assert!(
        Pin::new(&mut successor)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
}

#[test]
fn release_batch_records_actual_physical_poison_without_later_cleanup_poison() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    for during_release in [false, true] {
        let source = ReleaseNotification::default();
        let physical = Mutex::new(());
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release(&mut registration_0);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        let mut batch = source.deferred_batch();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let guard = source.poisoning_guard(physical.lock().unwrap());
            let result = guard.try_release_into(&mut batch, |guard| {
                assert!(!during_release, "physical release callback unwound");
                drop(guard);
            });
            assert!(result.is_ok());
        }));
        assert_eq!(panic.is_err(), during_release);
        assert_eq!(physical.is_poisoned(), during_release);
        assert_eq!(wake.0.load(Ordering::SeqCst), 0);
        assert!(poll(&mut wait, &wake).is_pending());
        assert!(!observation.is_poisoned());
        let later = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _batch = batch;
            panic!("later aggregate cleanup unwound");
        }));
        assert!(later.is_err());
        assert_eq!(observation.is_poisoned(), during_release);
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &wake).is_ready());
    }
}

#[test]
fn retained_phase_transfer_and_refusal_keep_original_source_without_early_wake() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let foreign = ReleaseNotification::default();
    #[expect(
        clippy::mutex_integer,
        reason = "exercise physical guard custody and original value through transfer"
    )]
    let lock = Mutex::new(17);
    let mut batch = source.deferred_batch();
    let mut foreign_batch = foreign.deferred_batch();
    let wake = Arc::new(WakeCount::default());
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    assert!(poll(&mut wait, &wake).is_pending());
    let guard = source.poisoning_guard(lock.lock().unwrap());
    let guard = guard
        .try_map_preserving_release_into(
            &mut foreign_batch,
            |_| -> Result<(), (_, ())> { panic!("foreign source must not invoke conversion") },
            || lock.is_poisoned(),
        )
        .err()
        .expect("return the same foreign-batch guard");
    let (guard, error) = guard
        .try_map_preserving_release_into(
            &mut batch,
            |guard| Err::<(), _>((guard, "refused")),
            || lock.is_poisoned(),
        )
        .unwrap_or_else(|_| panic!("original source"))
        .err()
        .expect("normal refusal retains actual guard");
    assert_eq!(error, "refused");
    assert!(lock.try_lock().is_err());
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    let guard = guard
        .try_map_preserving_release_into(
            &mut batch,
            |guard| Ok::<_, (_, ())>((guard, 41)),
            || lock.is_poisoned(),
        )
        .unwrap_or_else(|_| panic!("original source"))
        .unwrap_or_else(|_| panic!("successful phase transfer"));
    assert_eq!(*guard.0, 17);
    assert_eq!(guard.1, 41);
    guard
        .try_release_into_observed(&mut batch, drop, || lock.is_poisoned())
        .unwrap_or_else(|_| panic!("original release source"));
    assert!(lock.try_lock().is_ok());
    assert!(poll(&mut wait, &wake).is_pending());
    drop(foreign_batch);
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    drop(batch);
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
}

#[test]
fn retained_phase_unwind_records_actual_release_without_running_waiter() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let lock = Mutex::new(());
    let mut batch = source.deferred_batch();
    let observation = source.observe();
    let wake = Arc::new(WakeCount::default());
    let mut wait = observation.clone().wait_for_release(&mut registration_0);
    assert!(poll(&mut wait, &wake).is_pending());
    let guard = source.poisoning_guard(lock.lock().unwrap());
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _result = guard.try_map_preserving_release_into(
            &mut batch,
            |guard| -> Result<(), (_, ())> {
                let _original = guard;
                panic!("constructor panic")
            },
            || lock.is_poisoned(),
        );
    }));
    assert!(result.is_err());
    assert!(matches!(
        lock.try_lock(),
        Err(std::sync::TryLockError::Poisoned(_))
    ));
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    assert!(poll(&mut wait, &wake).is_pending());
    drop(batch);
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(observation.is_poisoned());
    assert!(poll(&mut wait, &wake).is_ready());
}

#[test]
fn retained_observed_release_preserves_poison_predating_normal_cleanup() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let source = ReleaseNotification::default();
    let lock = Mutex::new(());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = lock.lock().unwrap();
            panic!("preexisting native poison");
        }))
        .is_err()
    );
    let mut batch = source.deferred_batch();
    let observation = source.observe();
    let guard = source.poisoning_guard(
        lock.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    guard
        .try_release_into_observed(&mut batch, drop, || lock.is_poisoned())
        .unwrap_or_else(|_| panic!("original source"));
    assert!(!observation.is_poisoned());
    drop(batch);
    assert!(observation.is_poisoned());
}

#[test]
fn charged_notification_retains_original_control_through_observers_and_deferred_releases() {
    struct Charge(Arc<AtomicUsize>);
    impl Drop for Charge {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    assert_eq!(
        ReleaseNotification::allocation_layout::<Charge>(),
        Shared::<Mutex<State>, Charge>::layout(),
    );
    let refunds = Arc::new(AtomicUsize::new(0));
    let source = ReleaseNotification::new_charged(Charge(Arc::clone(&refunds)));
    let observation = source.observe();
    let same = observation.clone();
    let ((), deferred) = source.guard(()).release_deferred(drop);
    drop(source);
    assert_eq!(refunds.load(Ordering::SeqCst), 0);
    drop(deferred);
    assert_eq!(observation, same);
    let mut future = observation.wait_for_release(&mut registration_0);
    assert!(poll(&mut future, &Arc::new(WakeCount::default())).is_ready());
    drop(future);
    assert_eq!(refunds.load(Ordering::SeqCst), 0);
    drop(same);
    assert_eq!(refunds.load(Ordering::SeqCst), 1);
}

#[test]
fn deferred_notice_merge_retains_exact_source_and_never_wakes_early() {
    let registration_budget =
        AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let [mut registration_0] = registrations(&registration_budget);
    let source = ReleaseNotification::default();
    let foreign = ReleaseNotification::default();
    let physical = Mutex::new(());
    let mut batch = source.deferred_batch();
    let mut wrong = foreign.deferred_batch();
    let mut wait = source.observe().wait_for_release(&mut registration_0);
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    for _ in 0..3 {
        let ((), notice) = source
            .guard(physical.lock().unwrap())
            .release_deferred(drop);
        assert!(physical.try_lock().is_ok());
        let notice = match notice.try_merge_into(&mut wrong) {
            Err(original) => original,
            Ok(()) => panic!("foreign source accepted actual release"),
        };
        assert!(notice.try_merge_into(&mut batch).is_ok());
        assert_eq!(wake.0.load(Ordering::SeqCst), 0);
        assert!(poll(&mut wait, &wake).is_pending());
    }
    drop(wrong);
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    drop(batch);
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
}

#[test]
fn deferred_notice_merge_preserves_poison_after_rejected_transfer() {
    let source = ReleaseNotification::default();
    let foreign = ReleaseNotification::default();
    let mut exact = source.deferred_batch();
    let mut wrong = foreign.deferred_batch();
    let observation = source.observe();
    let physical = Mutex::new(());
    let _ = std::panic::catch_unwind(|| {
        let _guard = physical.lock().unwrap();
        panic!("actual original physical poison");
    });
    assert!(physical.is_poisoned());
    let guard = ReleaseGuard {
        inner: Some(
            physical
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        ),
        notification: &source,
        poison: PoisonPolicy::Fixed(physical.is_poisoned()),
    };
    let ((), notice) = guard.release_deferred(drop);
    let notice = match notice.try_merge_into(&mut wrong) {
        Err(original) => original,
        Ok(()) => panic!("foreign poison source accepted"),
    };
    assert!(notice.try_merge_into(&mut exact).is_ok());
    assert!(!observation.is_poisoned());
    drop(wrong);
    assert!(!foreign.observe().is_poisoned());
    drop(exact);
    assert!(observation.is_poisoned());
}
