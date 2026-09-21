//! Draft regressions for original EBR pair construction and unwind cleanup.
//!
//! Intended as a child module of mv::cell. The only pending test seam is the
//! native EbrCell::try_acquire_writer phase described in README.md. Cell entry
//! points below are the existing public production methods. No test is executed
//! merely by keeping this ignored draft here.

use super::*;
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, OnceLock, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

type Target = Cell<Payload, Charge>;
const UNDO: u64 = 10;
const CURRENT: u64 = 20;

struct Control {
    target: OnceLock<Weak<Target>>,
    fail_current_clone: AtomicBool,
    fail_undo_clone: AtomicBool,
    observe_clones: AtomicBool,
    undo_clone_busy: AtomicUsize,
    current_clone_busy: AtomicUsize,
    observe_cleanup: AtomicBool,
    undo_clones: AtomicUsize,
    current_clones: AtomicUsize,
    undo_payload_drops: AtomicUsize,
    current_payload_drops: AtomicUsize,
    current_payload_busy: AtomicUsize,
    skip_one_current_edit_drop: AtomicBool,
    undo_charge_drops: AtomicUsize,
    current_charge_drops: AtomicUsize,
    current_charge_busy: AtomicUsize,
    payload_busy: AtomicUsize,
    charge_busy: AtomicUsize,
    sequence: AtomicUsize,
    payload_event: AtomicUsize,
    charge_event: AtomicUsize,
    undo_wakes: AtomicUsize,
    current_wakes: AtomicUsize,
    undo_wake_busy: AtomicUsize,
    current_wake_busy: AtomicUsize,
}
impl Control {
    fn new(observe_cleanup: bool) -> Arc<Self> {
        Arc::new(Self {
            target: OnceLock::new(),
            fail_current_clone: AtomicBool::new(false),
            fail_undo_clone: AtomicBool::new(false),
            observe_clones: AtomicBool::new(false),
            undo_clone_busy: AtomicUsize::new(0),
            current_clone_busy: AtomicUsize::new(0),
            observe_cleanup: AtomicBool::new(observe_cleanup),
            undo_clones: AtomicUsize::new(0),
            current_clones: AtomicUsize::new(0),
            undo_payload_drops: AtomicUsize::new(0),
            current_payload_drops: AtomicUsize::new(0),
            current_payload_busy: AtomicUsize::new(0),
            skip_one_current_edit_drop: AtomicBool::new(false),
            undo_charge_drops: AtomicUsize::new(0),
            current_charge_drops: AtomicUsize::new(0),
            current_charge_busy: AtomicUsize::new(0),
            payload_busy: AtomicUsize::new(0),
            charge_busy: AtomicUsize::new(0),
            sequence: AtomicUsize::new(0),
            payload_event: AtomicUsize::new(0),
            charge_event: AtomicUsize::new(0),
            undo_wakes: AtomicUsize::new(0),
            current_wakes: AtomicUsize::new(0),
            undo_wake_busy: AtomicUsize::new(0),
            current_wake_busy: AtomicUsize::new(0),
        })
    }

    fn busy(&self) -> usize {
        let Some(target) = self.target.get().and_then(Weak::upgrade) else {
            return 4; // Report a missing original owner; never panic in cleanup.
        };
        // Required native seam: both poisoned and healthy acquired locks are
        // Some. No clone, generation allocation, EBR pin or notification occurs.
        let undo = target.revert.try_acquire_writer();
        let current = target.blocks.try_acquire_writer();
        let mask = usize::from(undo.is_none()) | (usize::from(current.is_none()) << 1);
        drop((undo, current));
        mask
    }
}

struct Payload {
    value: u64,
    private_clone: bool,
    control: Arc<Control>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        if self.value == UNDO {
            self.control.undo_clones.fetch_add(1, SeqCst);
            if self.control.observe_clones.load(SeqCst) {
                self.control
                    .undo_clone_busy
                    .fetch_or(self.control.busy(), SeqCst);
            }
            if self.control.fail_undo_clone.load(SeqCst) {
                panic!("undo payload clone while current charge remains unused");
            }
        } else {
            self.control.current_clones.fetch_add(1, SeqCst);
            if self.control.observe_clones.load(SeqCst) {
                self.control
                    .current_clone_busy
                    .fetch_or(self.control.busy(), SeqCst);
            }
            if self.control.fail_current_clone.load(SeqCst) {
                // This is an intentional production-callee fault. Wake
                // and Drop observers below never assert or deliberately panic.
                panic!("current payload clone after completed undo generation");
            }
        }
        Self {
            value: self.value,
            private_clone: true,
            control: Arc::clone(&self.control),
        }
    }
}
impl Drop for Payload {
    fn drop(&mut self) {
        if self.private_clone && self.value == CURRENT {
            self.control.current_payload_drops.fetch_add(1, SeqCst);
            let edit_drop = self.control.skip_one_current_edit_drop.swap(false, SeqCst);
            if self.control.observe_cleanup.load(SeqCst) && !edit_drop {
                self.control
                    .current_payload_busy
                    .fetch_or(self.control.busy(), SeqCst);
            }
        }
        if self.private_clone && self.value == UNDO {
            self.control.undo_payload_drops.fetch_add(1, SeqCst);
            if self.control.observe_cleanup.load(SeqCst) {
                self.control
                    .payload_busy
                    .fetch_or(self.control.busy(), SeqCst);
            }
            self.control
                .payload_event
                .store(self.control.sequence.fetch_add(1, SeqCst) + 1, SeqCst);
        }
    }
}

// Move-only custody witness for the original EBR generation charge. This is
// deliberately not a System deallocation witness or aggregate payload budget.
struct Charge {
    control: Weak<Control>,
    undo: bool,
}
impl Charge {
    fn quiet() -> Self {
        Self {
            control: Weak::new(),
            undo: false,
        }
    }
    fn original(control: &Arc<Control>, undo: bool) -> Self {
        Self {
            control: Arc::downgrade(control),
            undo,
        }
    }
}
impl Drop for Charge {
    fn drop(&mut self) {
        let Some(control) = self.control.upgrade() else {
            return;
        };
        if self.undo {
            control.undo_charge_drops.fetch_add(1, SeqCst);
            if control.observe_cleanup.load(SeqCst) {
                control.charge_busy.fetch_or(control.busy(), SeqCst);
            }
            control
                .charge_event
                .store(control.sequence.fetch_add(1, SeqCst) + 1, SeqCst);
        } else {
            control.current_charge_drops.fetch_add(1, SeqCst);
            if control.observe_cleanup.load(SeqCst) {
                control.current_charge_busy.fetch_or(control.busy(), SeqCst);
            }
        }
    }
}

struct Probe {
    control: Arc<Control>,
    undo: bool,
}
impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        let busy = self.control.busy();
        let (calls, observed) = if self.undo {
            (&self.control.undo_wakes, &self.control.undo_wake_busy)
        } else {
            (&self.control.current_wakes, &self.control.current_wake_busy)
        };
        observed.fetch_or(busy, SeqCst);
        calls.fetch_add(1, SeqCst);
    }
}

struct Waits {
    undo: crate::ReleaseWait,
    current: crate::ReleaseWait,
    undo_future: concread::release::ReleaseFuture,
    current_future: concread::release::ReleaseFuture,
    undo_waker: Waker,
    current_waker: Waker,
}
impl Waits {
    fn new(target: &Target, control: &Arc<Control>) -> Self {
        let undo = target.revert_released.observe();
        let current = target.blocks_released.observe();
        let mut result = Self {
            undo_future: undo.clone().wait_for_release(),
            current_future: current.clone().wait_for_release(),
            undo,
            current,
            undo_waker: Waker::from(Arc::new(Probe {
                control: Arc::clone(control),
                undo: true,
            })),
            current_waker: Waker::from(Arc::new(Probe {
                control: Arc::clone(control),
                undo: false,
            })),
        };
        assert!(!result.undo_ready() && !result.current_ready());
        result
    }
    fn undo_ready(&mut self) -> bool {
        Pin::new(&mut self.undo_future)
            .poll(&mut Context::from_waker(&self.undo_waker))
            .is_ready()
    }
    fn current_ready(&mut self) -> bool {
        Pin::new(&mut self.current_future)
            .poll(&mut Context::from_waker(&self.current_waker))
            .is_ready()
    }
}

fn fixture(observe_cleanup: bool) -> (Arc<Target>, Arc<Control>) {
    let control = Control::new(observe_cleanup);
    let payload = |value| Payload {
        value,
        private_clone: false,
        control: Arc::clone(&control),
    };
    // The same production constructor used for exact decoded current/undo
    // images. No synthetic predecessor or publication permission is minted.
    let target = Arc::new(Cell::from_values_charged(
        payload(CURRENT),
        Some(payload(UNDO)),
        CellAllocationCharges::new(Charge::quiet(), Charge::quiet()),
    ));
    assert!(control.target.set(Arc::downgrade(&target)).is_ok());
    (target, control)
}

fn attempt(target: &Target, control: &Arc<Control>, replacement: bool) {
    let charges = CellAllocationCharges::new(
        Charge::original(control, false),
        Charge::original(control, true),
    );
    if replacement {
        drop(target.block_and_revert_charged(charges));
    } else {
        drop(target.block_charged(charges));
    }
}

fn assert_original(target: &Target, before: &CapturedPublication) {
    assert_eq!(target.view().value, CURRENT);
    assert_eq!(
        target.predecessor_view().as_ref().map(|v| v.value),
        Some(UNDO)
    );
    let (result, cleanup) = before.try_check_current::<()>(&target.publication);
    drop(cleanup);
    assert_eq!(result, Ok(()));
}

#[test]
fn cell_second_clone_panic_releases_both_before_native_notifications() {
    for replacement in [false, true] {
        let (target, control) = fixture(false);
        let predecessor = target.publication.capture();
        let mut waits = Waits::new(&target, &control);
        control.fail_current_clone.store(true, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| attempt(&target, &control, replacement)));
        control.fail_current_clone.store(false, SeqCst);
        assert!(result.is_err());
        assert_eq!(control.undo_clones.load(SeqCst), 1);
        assert_eq!(control.current_clones.load(SeqCst), 1);
        assert_eq!(control.undo_payload_drops.load(SeqCst), 1);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 0);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        assert!(waits.undo.is_poisoned() && waits.current.is_poisoned());
        assert!(target.revert.is_poisoned() && target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
    }
}

#[test]
fn cell_second_clone_panic_reclaims_completed_undo_only_after_pair_unlock() {
    for replacement in [false, true] {
        let (target, control) = fixture(true);
        let predecessor = target.publication.capture();
        // No waiters: this case isolates payload/charge retirement ordering
        // from the native release notification regression above.
        control.fail_current_clone.store(true, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| attempt(&target, &control, replacement)));
        control.fail_current_clone.store(false, SeqCst);
        assert!(result.is_err());
        assert_eq!(control.undo_clones.load(SeqCst), 1);
        assert_eq!(control.current_clones.load(SeqCst), 1);
        assert_eq!(control.undo_payload_drops.load(SeqCst), 1);
        assert_eq!(control.undo_charge_drops.load(SeqCst), 1);
        assert_eq!(control.payload_busy.load(SeqCst), 0);
        assert_eq!(control.charge_busy.load(SeqCst), 0);
        assert!(control.payload_event.load(SeqCst) > 0);
        assert!(control.charge_event.load(SeqCst) > control.payload_event.load(SeqCst));
        // Existing EBR contract retains current's charge on arbitrary Clone
        // panic; this test does not claim that partial payload was reclaimed.
        assert_eq!(control.current_charge_drops.load(SeqCst), 0);
        assert_original(&target, &predecessor);
    }
}

#[test]
fn cell_known_undo_poison_rejects_before_waiting_for_current() {
    use std::{sync::mpsc, time::Duration};

    for replacement in [false, true] {
        let (target, control) = fixture(false);
        let predecessor = target.publication.capture();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _undo = target.revert.try_acquire_writer().unwrap();
                panic!("poison original undo without constructing a generation");
            }))
            .is_err()
        );
        let held = target
            .blocks_released
            .poisoning_guard(target.blocks.try_acquire_writer().unwrap());
        let mut waits = Waits::new(&target, &control);
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (finished_tx, finished_rx) = mpsc::sync_channel(1);
        let worker_target = Arc::clone(&target);
        let worker_control = Arc::clone(&control);
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                attempt(&worker_target, &worker_control, replacement);
            }))
            .is_err();
            finished_tx.send(rejected).unwrap();
        });
        let started = started_rx.recv_timeout(Duration::from_secs(5));
        let finished_before_release = finished_rx.recv_timeout(Duration::from_secs(2));
        let undo_ready_before_release = waits.undo_ready();
        let current_ready_before_release = waits.current_ready();
        let current_wakes_before_release = control.current_wakes.load(SeqCst);
        // Unblock and join even on timeout: assertions cannot strand a worker.
        drop(held);
        let joined = worker.join();
        assert!(started.is_ok() && joined.is_ok());
        assert!(
            matches!(finished_before_release, Ok(true)),
            "known undo poison waited for current: {finished_before_release:?}"
        );
        assert!(undo_ready_before_release && !current_ready_before_release);
        assert_eq!(current_wakes_before_release, 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 2);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert_eq!(control.undo_clones.load(SeqCst), 0);
        assert_eq!(control.current_clones.load(SeqCst), 0);
        assert!(waits.undo.is_poisoned() && !waits.current.is_poisoned());
        assert!(target.revert.is_poisoned() && !target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
    }
}

fn attempt_kind(target: &Target, control: &Arc<Control>, kind: u8) {
    if kind < 2 {
        attempt(target, control, kind == 1);
    } else {
        let charges = CellAllocationCharges::new(
            Charge::original(control, false),
            Charge::original(control, true),
        );
        drop(target.current_replacement_charged(charges));
    }
}

#[test]
fn cell_first_clone_panic_releases_pair_before_unused_current_charge() {
    // Ordinary, replacement block, and same-cut current replacement all use
    // the same partial-pair constructor. None may consume current's charge.
    for kind in 0..3 {
        let (target, control) = fixture(true);
        let predecessor = target.publication.capture();
        let mut waits = Waits::new(&target, &control);
        control.fail_undo_clone.store(true, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| attempt_kind(&target, &control, kind)));
        control.fail_undo_clone.store(false, SeqCst);
        assert!(result.is_err());
        assert_eq!(control.undo_clones.load(SeqCst), 1);
        assert_eq!(control.current_clones.load(SeqCst), 0);
        assert_eq!(control.undo_payload_drops.load(SeqCst), 0);
        // Undo's attempted Clone keeps its own conservative charge. Current
        // never cloned and must release its unused original charge exactly once.
        assert_eq!(control.undo_charge_drops.load(SeqCst), 0);
        assert_eq!(control.current_charge_drops.load(SeqCst), 1);
        assert_eq!(control.current_charge_busy.load(SeqCst), 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 0);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        assert!(waits.undo.is_poisoned() && waits.current.is_poisoned());
        assert!(target.revert.is_poisoned() && target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
    }
}

#[test]
fn cell_known_current_poison_precedes_both_clones_and_charge_cleanup() {
    for kind in 0..3 {
        let (target, control) = fixture(true);
        let predecessor = target.publication.capture();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _current = target.blocks.try_acquire_writer().unwrap();
                panic!("poison current without cloning either payload");
            }))
            .is_err()
        );
        let mut waits = Waits::new(&target, &control);
        let result = catch_unwind(AssertUnwindSafe(|| attempt_kind(&target, &control, kind)));
        assert!(result.is_err());
        assert_eq!(control.undo_clones.load(SeqCst), 0);
        assert_eq!(control.current_clones.load(SeqCst), 0);
        assert_eq!(control.undo_payload_drops.load(SeqCst), 0);
        assert_eq!(control.undo_charge_drops.load(SeqCst), 1);
        assert_eq!(control.current_charge_drops.load(SeqCst), 1);
        assert_eq!(control.charge_busy.load(SeqCst), 0);
        assert_eq!(control.current_charge_busy.load(SeqCst), 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 0);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        assert!(waits.undo.is_poisoned() && waits.current.is_poisoned());
        assert!(target.revert.is_poisoned() && target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
    }
}

#[test]
fn cell_successful_pair_acquisition_keeps_clones_locked_and_notifications_pending() {
    for replacement in [false, true] {
        let (target, control) = fixture(false);
        let predecessor = target.publication.capture();
        let mut waits = Waits::new(&target, &control);
        control.observe_clones.store(true, SeqCst);
        let charges = CellAllocationCharges::new(
            Charge::original(&control, false),
            Charge::original(&control, true),
        );
        let block = if replacement {
            target.block_and_revert_charged(charges)
        } else {
            target.block_charged(charges)
        };
        assert_eq!(control.undo_clones.load(SeqCst), 1);
        assert_eq!(control.current_clones.load(SeqCst), 1);
        assert_eq!(control.undo_clone_busy.load(SeqCst), 3);
        assert_eq!(control.current_clone_busy.load(SeqCst), 3);
        assert_eq!(control.busy(), 3);
        assert_eq!(control.undo_wakes.load(SeqCst), 0);
        assert_eq!(control.current_wakes.load(SeqCst), 0);
        assert!(!waits.undo_ready() && !waits.current_ready());
        assert_eq!(block.get().value, if replacement { UNDO } else { CURRENT });
        assert_eq!(
            block.mode(),
            if replacement {
                BlockMode::Replace
            } else {
                BlockMode::Ordinary
            }
        );
        // Explicitly detach instead of relying on ordinary Block Drop. This
        // test's guarantee ends at successful acquisition; each original writer
        // may notify during the existing sequential detachment protocol.
        let detached = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
        assert_eq!(control.busy(), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert!(!waits.undo.is_poisoned() && !waits.current.is_poisoned());
        assert!(!target.revert.is_poisoned() && !target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
        drop(detached);
        assert_eq!(control.undo_charge_drops.load(SeqCst), 1);
        assert_eq!(control.current_charge_drops.load(SeqCst), 1);
    }
}

fn arm_completed_pair_cleanup(control: &Control) {
    assert_eq!(control.busy(), 3);
    assert_eq!(control.undo_wakes.load(SeqCst), 0);
    assert_eq!(control.current_wakes.load(SeqCst), 0);
    assert_eq!(control.undo_charge_drops.load(SeqCst), 0);
    assert_eq!(control.current_charge_drops.load(SeqCst), 0);
    // Block opening may already have cleared/replaced a private value. This
    // regression observes only abandonment after the complete owner returns.
    control.undo_payload_drops.store(0, SeqCst);
    control.current_payload_drops.store(0, SeqCst);
    control.observe_cleanup.store(true, SeqCst);
}

#[test]
fn cell_complete_block_and_current_replacement_abandonment_unlocks_before_cleanup() {
    for kind in 0..3 {
        let (target, control) = fixture(false);
        let predecessor = target.publication.capture();
        let mut waits = Waits::new(&target, &control);
        let charges = CellAllocationCharges::new(
            Charge::original(&control, false),
            Charge::original(&control, true),
        );
        match kind {
            0 => {
                let block = target.block_charged(charges);
                arm_completed_pair_cleanup(&control);
                drop(block);
            }
            1 => {
                let block = target.block_and_revert_charged(charges);
                arm_completed_pair_cleanup(&control);
                drop(block);
            }
            _ => {
                let replacement = target.current_replacement_charged(charges);
                arm_completed_pair_cleanup(&control);
                drop(replacement);
            }
        }
        assert_eq!(
            control.undo_payload_drops.load(SeqCst),
            usize::from(kind != 0)
        );
        assert_eq!(
            control.current_payload_drops.load(SeqCst),
            usize::from(kind != 1)
        );
        assert_eq!(control.undo_charge_drops.load(SeqCst), 1);
        assert_eq!(control.current_charge_drops.load(SeqCst), 1);
        assert_eq!(control.payload_busy.load(SeqCst), 0);
        assert_eq!(control.current_payload_busy.load(SeqCst), 0);
        assert_eq!(control.charge_busy.load(SeqCst), 0);
        assert_eq!(control.current_charge_busy.load(SeqCst), 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 0);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        assert!(!waits.undo.is_poisoned() && !waits.current.is_poisoned());
        assert!(!target.revert.is_poisoned() && !target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
    }
}

#[test]
fn cell_explicit_detach_keeps_original_generations_and_defers_both_notifications() {
    for replacement in [false, true] {
        let (target, control) = fixture(false);
        let predecessor = target.publication.capture();
        let mut waits = Waits::new(&target, &control);
        let charges = CellAllocationCharges::new(
            Charge::original(&control, false),
            Charge::original(&control, true),
        );
        let block = if replacement {
            target.block_and_revert_charged(charges)
        } else {
            target.block_charged(charges)
        };
        arm_completed_pair_cleanup(&control);
        let detached = block
            .try_detach(|held| {
                assert_eq!(control.busy(), 3);
                assert_eq!(held.get().value, if replacement { UNDO } else { CURRENT });
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(control.busy(), 0);
        assert_eq!(control.undo_payload_drops.load(SeqCst), 0);
        assert_eq!(control.current_payload_drops.load(SeqCst), 0);
        assert_eq!(control.undo_charge_drops.load(SeqCst), 0);
        assert_eq!(control.current_charge_drops.load(SeqCst), 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 0);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        assert!(!waits.undo.is_poisoned() && !waits.current.is_poisoned());
        assert_original(&target, &predecessor);
        drop(detached);
        assert_eq!(
            control.undo_payload_drops.load(SeqCst),
            usize::from(replacement)
        );
        assert_eq!(
            control.current_payload_drops.load(SeqCst),
            usize::from(!replacement)
        );
        assert_eq!(control.undo_charge_drops.load(SeqCst), 1);
        assert_eq!(control.current_charge_drops.load(SeqCst), 1);
        assert_eq!(control.payload_busy.load(SeqCst), 0);
        assert_eq!(control.current_payload_busy.load(SeqCst), 0);
        assert_eq!(control.charge_busy.load(SeqCst), 0);
        assert_eq!(control.current_charge_busy.load(SeqCst), 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
    }
}

#[test]
fn cell_publication_poison_preserves_pair_through_commit_refusal_cleanup() {
    for kind in 0..3 {
        let (target, control) = fixture(false);
        let predecessor = target.publication.capture();
        let mut waits = Waits::new(&target, &control);
        let charges = CellAllocationCharges::new(
            Charge::original(&control, false),
            Charge::original(&control, true),
        );
        // Poison only identity while the complete executing owner exists. The
        // panicking callback runs before identity rotation or native transfer.
        let poison_identity = || {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    target.publication.publish_retaining(
                        NextPublication::new(),
                        || panic!("poison only the original publication mutex"),
                        |()| (),
                    );
                }))
                .is_err()
            );
            assert!(!target.revert.is_poisoned() && !target.blocks.is_poisoned());
            assert_eq!(control.busy(), 3);
            assert_eq!(control.undo_wakes.load(SeqCst), 0);
            assert_eq!(control.current_wakes.load(SeqCst), 0);
        };
        let result = match kind {
            0 => {
                let block = target.block_charged(charges);
                arm_completed_pair_cleanup(&control);
                poison_identity();
                catch_unwind(AssertUnwindSafe(|| block.commit()))
            }
            1 => {
                let block = target.block_and_revert_charged(charges);
                arm_completed_pair_cleanup(&control);
                poison_identity();
                catch_unwind(AssertUnwindSafe(|| block.commit()))
            }
            _ => {
                let replacement = target.current_replacement_charged(charges);
                arm_completed_pair_cleanup(&control);
                poison_identity();
                // Replacing a value deliberately drops the old private payload
                // while executing under the writers. Exclude precisely that
                // first edit drop; the second, installed payload must be cleaned
                // only after both writers unlock during publication refusal.
                control.skip_one_current_edit_drop.store(true, SeqCst);
                let value = Payload {
                    value: CURRENT,
                    private_clone: true,
                    control: Arc::clone(&control),
                };
                catch_unwind(AssertUnwindSafe(|| replacement.publish(value)))
            }
        };
        assert!(result.is_err());
        assert!(!control.skip_one_current_edit_drop.load(SeqCst));
        assert_eq!(
            control.undo_payload_drops.load(SeqCst),
            usize::from(kind != 0)
        );
        assert_eq!(
            control.current_payload_drops.load(SeqCst),
            match kind {
                0 => 1,
                1 => 0,
                _ => 2,
            }
        );
        assert_eq!(control.undo_charge_drops.load(SeqCst), 1);
        assert_eq!(control.current_charge_drops.load(SeqCst), 1);
        assert_eq!(control.payload_busy.load(SeqCst), 0);
        assert_eq!(control.current_payload_busy.load(SeqCst), 0);
        assert_eq!(control.charge_busy.load(SeqCst), 0);
        assert_eq!(control.current_charge_busy.load(SeqCst), 0);
        assert_eq!(control.undo_wakes.load(SeqCst), 1);
        assert_eq!(control.current_wakes.load(SeqCst), 1);
        assert_eq!(control.undo_wake_busy.load(SeqCst), 0);
        assert_eq!(control.current_wake_busy.load(SeqCst), 0);
        assert!(waits.undo_ready() && waits.current_ready());
        // Identity acquisition now unwound through the retained native pair;
        // both real writers poison, and their original observations agree.
        assert!(waits.undo.is_poisoned() && waits.current.is_poisoned());
        assert!(target.revert.is_poisoned() && target.blocks.is_poisoned());
        assert_eq!(target.view().value, CURRENT);
        assert_eq!(
            target.predecessor_view().as_ref().map(|value| value.value),
            Some(UNDO)
        );
        let (status, cleanup) = predecessor.try_check_current::<()>(&target.publication);
        drop(cleanup);
        assert_eq!(status, Err(crate::PublicationPreparationError::Poisoned));
    }
}
