//! Release observations survive registration races without owning data locks.

use super::*;
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::Wake,
};

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(wait: &mut ReleaseFuture, wake: &Arc<WakeCount>) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::clone(wake))))
}
#[test]
fn release_before_registration_is_retained_and_other_sources_do_not_wake() {
    let source = ReleaseNotification::default();
    let other = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.guard(lock.lock().unwrap());
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    drop(other.guard(()));
    assert!(poll(&mut wait, &wake).is_pending());
    let mut before_first_poll = source.observe().wait_for_release();
    drop(guard);
    assert!(lock.try_lock().is_ok());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
    assert!(poll(&mut before_first_poll, &wake).is_ready());
}

#[test]
fn first_registered_wake_can_reenter_both_initialized_notification_locks() {
    struct FirstWake {
        source: Arc<ReleaseNotification>,
        registration: std::sync::OnceLock<Weak<Mutex<Option<Waker>>>>,
        calls: AtomicUsize,
    }
    impl Wake for FirstWake {
        fn wake(self: Arc<Self>) {
            assert!(self.source.state.try_lock().is_ok());
            let registration = self.registration.get().unwrap().upgrade().unwrap();
            assert!(registration.try_lock().is_ok());
            drop(self.source.observe());
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    let source = Arc::new(ReleaseNotification::default());
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(FirstWake {
        source: Arc::clone(&source),
        registration: std::sync::OnceLock::new(),
        calls: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    assert!(wake
        .registration
        .set(Arc::downgrade(wait.registration.as_ref().unwrap()))
        .is_ok());
    drop(source.guard(()));
    assert_eq!(wake.calls.load(Ordering::SeqCst), 1);
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(&waker))
        .is_ready());
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
            if !self.panic_in_drop {
                panic!("first wake callback panicked");
            }
        }
    }
    impl Drop for FirstWake {
        fn drop(&mut self) {
            // Never assert during an existing unwind: record lock ordering for
            // the caller to inspect after catching the original callback panic.
            self.drop_was_unlocked
                .store(self.source.state.try_lock().is_ok(), Ordering::SeqCst);
            self.drops.fetch_add(1, Ordering::SeqCst);
            if self.panic_in_drop {
                panic!("first wake destructor panicked");
            }
        }
    }

    for panic_in_drop in [false, true] {
        let source = Arc::new(ReleaseNotification::default());
        let observation = source.observe();
        let mut first = observation.clone().wait_for_release();
        let mut survivor = observation.wait_for_release();
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
        assert!(Pin::new(&mut first)
            .poll(&mut Context::from_waker(&first_waker))
            .is_pending());
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
        assert!(source.state.lock().unwrap().waiters.is_empty());
        drop(source.guard(()));
        assert_eq!(survivor_wakes.0.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn cancellation_and_waker_replacement_do_not_steal_another_wait() {
    let source = ReleaseNotification::default();
    let observation = source.observe();
    let mut canceled = observation.clone().wait_for_release();
    let mut retained = observation.wait_for_release();
    let old = Arc::new(WakeCount::default());
    let new = Arc::new(WakeCount::default());
    assert!(poll(&mut canceled, &old).is_pending());
    assert!(poll(&mut retained, &old).is_pending());
    assert!(poll(&mut retained, &new).is_pending());
    drop(canceled);
    assert_eq!(source.state.lock().unwrap().waiters.len(), 1);
    drop(source.guard(()));
    assert_eq!(old.0.load(Ordering::SeqCst), 0);
    assert_eq!(new.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut retained, &new).is_ready());
    assert!(source.state.lock().unwrap().waiters.is_empty());
    let mut canceled = source.observe().wait_for_release();
    assert!(poll(&mut canceled, &old).is_pending());
    drop(canceled);
    assert_eq!(source.state.lock().unwrap().waiters.capacity(), 0);
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
    let source = Arc::new(ReleaseNotification::default());
    let mut wait = source.observe().wait_for_release();
    let state_was_unlocked = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Waker::from(Arc::new(ObserveOnDrop {
        source: Arc::clone(&source),
        state_was_unlocked: Arc::clone(&state_was_unlocked),
        drops: Arc::clone(&drops),
    }));
    assert!(Pin::new(&mut wait)
        .poll(&mut Context::from_waker(&old))
        .is_pending());
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

    let source = Arc::new(ReleaseNotification::default());
    let observation = source.observe();
    let mut first = observation.clone().wait_for_release();
    let mut ready = observation.wait_for_release();
    let (started, started_rx) = mpsc::sync_channel(0);
    let (resume, resume_rx) = mpsc::sync_channel(0);
    let first_waker = Waker::from(Arc::new(PausedWake {
        started,
        resume: Mutex::new(resume_rx),
    }));
    assert!(Pin::new(&mut first)
        .poll(&mut Context::from_waker(&first_waker))
        .is_pending());

    let state_was_unlocked = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Waker::from(Arc::new(ObserveOnDrop {
        source: Arc::clone(&source),
        state_was_unlocked: Arc::clone(&state_was_unlocked),
        drops: Arc::clone(&drops),
    }));
    assert!(Pin::new(&mut ready)
        .poll(&mut Context::from_waker(&old))
        .is_pending());
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
    for _ in 0..128 {
        let source = ReleaseNotification::default();
        let mut wait = source.observe().wait_for_release();
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
    let source = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.poisoning_guard(lock.lock().unwrap());
    let mut wait = source.observe().wait_for_release();
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
    let source = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.poisoning_guard(lock.lock().unwrap());
    let observation = source.observe();
    let mut wait = observation.clone().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        guard.map_preserving_release::<()>(|guard| {
            drop(guard);
            panic!("phase transfer refused");
        });
    }))
    .is_err());
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

    for released in [false, true] {
        let source = ReleaseNotification::default();
        let lock = Mutex::new(());
        let guard = source.poisoning_guard(lock.lock().unwrap());
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release();
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let retirement = guard.release_retaining(|guard| {
                assert!(released, "failure with physical owner still held");
                drop(guard);
                PanickingRetirement
            });
            assert_eq!(wake.0.load(Ordering::SeqCst), 0);
            drop(retirement);
        }))
        .is_err());
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
        let mut wait_a = std::pin::pin!(source_a.observe().wait_for_release());
        let mut wait_b = std::pin::pin!(source_b.observe().wait_for_release());
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
fn deferred_release_keeps_original_wait_and_ignores_later_cleanup_unwind() {
    for panic_after in [false, true] {
        let source = ReleaseNotification::default();
        let physical = Mutex::new(());
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release();
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
    let source = ReleaseNotification::default();
    let physical = Mutex::new(7_u64);
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    let guard = source.poisoning_guard(physical.lock().unwrap());
    let original = &**guard as *const u64;
    let (guard, error) = guard
        .try_map_preserving_release(|guard| Err::<(), _>((guard, "busy")))
        .err()
        .unwrap();
    assert_eq!(error, "busy");
    assert_eq!(&**guard as *const u64, original);
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
