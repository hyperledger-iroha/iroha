//! Original caller-owned slots defer partial construction cleanup across cells.

use super::*;
use crate::{BlockAcquisition, BlockRetirement};
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};

struct Control {
    target: Mutex<Weak<Target>>,
    armed: AtomicBool,
    fail_clone: AtomicUsize,
    wakes: AtomicUsize,
    wake_busy: AtomicUsize,
    payloads: AtomicUsize,
    payload_busy: AtomicUsize,
    charges: AtomicUsize,
    charge_busy: AtomicUsize,
}

struct Payload {
    id: usize,
    cloned: bool,
    control: Arc<Control>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        assert_ne!(
            self.control.fail_clone.load(Ordering::SeqCst),
            self.id,
            "injected later clone"
        );
        Self {
            id: self.id,
            cloned: true,
            control: Arc::clone(&self.control),
        }
    }
}
impl Drop for Payload {
    fn drop(&mut self) {
        if self.cloned && self.control.armed.load(Ordering::SeqCst) {
            self.control.payloads.fetch_add(1, Ordering::SeqCst);
            if self.control.any_busy() {
                self.control.payload_busy.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}
struct Charge {
    control: Arc<Control>,
    watched: bool,
}
impl Drop for Charge {
    fn drop(&mut self) {
        if self.watched && self.control.armed.load(Ordering::SeqCst) {
            self.control.charges.fetch_add(1, Ordering::SeqCst);
            if self.control.any_busy() {
                self.control.charge_busy.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}
struct Target {
    first: Cell<Payload, Charge>,
    second: Cell<Payload, Charge>,
}
impl Control {
    fn any_busy(&self) -> bool {
        let Ok(target) = self.target.try_lock() else {
            return true;
        };
        let Some(target) = target.upgrade() else {
            return true;
        };
        // Native acquisition returns Some for acquired poison too; None means
        // physical contention only. Never invoke cloning from an observer.
        [
            target.first.revert.try_acquire_writer().is_none(),
            target.first.blocks.try_acquire_writer().is_none(),
            target.second.revert.try_acquire_writer().is_none(),
            target.second.blocks.try_acquire_writer().is_none(),
        ]
        .into_iter()
        .any(|busy| busy)
    }
    fn charges(self: &Arc<Self>) -> CellAllocationCharges<Charge> {
        CellAllocationCharges::new(
            Charge {
                control: Arc::clone(self),
                watched: true,
            },
            Charge {
                control: Arc::clone(self),
                watched: true,
            },
        )
    }
}
impl Wake for Control {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        if self.any_busy() {
            self.wake_busy.fetch_add(1, Ordering::SeqCst);
        }
    }
}
fn fixture() -> (Arc<Target>, Arc<Control>) {
    let control = Arc::new(Control {
        target: Mutex::new(Weak::new()),
        armed: AtomicBool::new(false),
        fail_clone: AtomicUsize::new(usize::MAX),
        wakes: AtomicUsize::new(0),
        wake_busy: AtomicUsize::new(0),
        payloads: AtomicUsize::new(0),
        payload_busy: AtomicUsize::new(0),
        charges: AtomicUsize::new(0),
        charge_busy: AtomicUsize::new(0),
    });
    let cell = |current, undo| {
        Cell::from_values_charged(
            Payload {
                id: current,
                cloned: false,
                control: Arc::clone(&control),
            },
            Some(Payload {
                id: undo,
                cloned: false,
                control: Arc::clone(&control),
            }),
            CellAllocationCharges::new(
                Charge {
                    control: Arc::clone(&control),
                    watched: false,
                },
                Charge {
                    control: Arc::clone(&control),
                    watched: false,
                },
            ),
        )
    };
    let target = Arc::new(Target {
        first: cell(1, 0),
        second: cell(3, 2),
    });
    *control.target.lock().unwrap() = Arc::downgrade(&target);
    (target, control)
}
struct Pending<'a> {
    first: BlockAcquisitionSlot<'a, Payload, Charge>,
    second: BlockAcquisitionSlot<'a, Payload, Charge>,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.first.release();
        self.second.release();
    }
}
fn register(
    source: &ReleaseNotification,
    control: &Arc<Control>,
) -> concread::release::ReleaseFuture {
    let mut wait = source.observe().wait_for_release();
    let waker = Waker::from(Arc::clone(control));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    wait
}
fn assert_cleanup(control: &Control) {
    assert_eq!(control.wake_busy.load(Ordering::SeqCst), 0);
    assert_eq!(control.payload_busy.load(Ordering::SeqCst), 0);
    assert_eq!(control.charge_busy.load(Ordering::SeqCst), 0);
    assert!(!control.any_busy());
}

#[test]
fn caller_owned_cell_slots_release_all_before_later_clone_unwind_cleanup() {
    for fail_clone in [2, 3] {
        let (target, control) = fixture();
        let mut pending = Pending {
            first: target.first.block_acquisition_charged(control.charges()),
            second: target.second.block_acquisition_charged(control.charges()),
        };
        pending.first.initialize(BlockMode::Ordinary);
        let first_wait = register(&target.first.revert_released, &control);
        let second_wait = register(&target.second.revert_released, &control);
        control.armed.store(true, Ordering::SeqCst);
        control.fail_clone.store(fail_clone, Ordering::SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| {
            pending.second.initialize(BlockMode::Ordinary)
        }));
        assert!(result.is_err());
        // Catching the callee unwind does not destroy the completed first slot,
        // completed second undo, unused charge, or original notification batch.
        assert_eq!(control.wakes.load(Ordering::SeqCst), 0);
        assert_eq!(control.charges.load(Ordering::SeqCst), 0);
        drop(pending);
        assert_eq!(control.wakes.load(Ordering::SeqCst), 2);
        assert_eq!(control.charges.load(Ordering::SeqCst), 3);
        assert!(control.payloads.load(Ordering::SeqCst) >= 1);
        assert_cleanup(&control);
        control.armed.store(false, Ordering::SeqCst);
        drop((first_wait, second_wait));
        assert_eq!(target.first.view().id, 1);
        assert_eq!(target.second.view().id, 3);
        // The failing Clone's charge is conservatively retained by the existing
        // native policy. Three other original charges demonstrably clean up.
    }
}

#[test]
fn caller_owned_cell_slots_retain_known_poison_until_earlier_slot_unlocks() {
    let (target, control) = fixture();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = target.second.revert.acquire_writer();
            panic!("poison only the later original undo");
        }))
        .is_err()
    );
    let mut pending = Pending {
        first: target.first.block_acquisition_charged(control.charges()),
        second: target.second.block_acquisition_charged(control.charges()),
    };
    pending.first.initialize(BlockMode::Ordinary);
    let first_wait = register(&target.first.revert_released, &control);
    let observation = target.second.revert_released.observe();
    let second_wait = register(&target.second.revert_released, &control);
    control.armed.store(true, Ordering::SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(|| pending
            .second
            .initialize(BlockMode::Ordinary)))
        .is_err()
    );
    assert_eq!(control.wakes.load(Ordering::SeqCst), 0);
    assert_eq!(control.charges.load(Ordering::SeqCst), 0);
    drop(pending);
    assert_eq!(control.wakes.load(Ordering::SeqCst), 2);
    assert_eq!(control.charges.load(Ordering::SeqCst), 4);
    assert!(observation.is_poisoned());
    assert_cleanup(&control);
    control.armed.store(false, Ordering::SeqCst);
    drop((first_wait, second_wait));
}

#[test]
fn completed_cell_slots_transfer_without_wake_and_release_without_retirement() {
    let (target, control) = fixture();
    let mut first = target.first.block_acquisition_charged(control.charges());
    let mut second = target.second.block_acquisition_charged(control.charges());
    first.initialize(BlockMode::Ordinary);
    second.initialize(BlockMode::Ordinary);
    let first_wait = register(&target.first.revert_released, &control);
    let second_wait = register(&target.second.revert_released, &control);
    control.armed.store(true, Ordering::SeqCst);
    let mut first = first.into_block();
    let mut second = second.into_block();
    assert_eq!(control.wakes.load(Ordering::SeqCst), 0);
    first.release_writers();
    second.release_writers();
    assert_eq!(control.wakes.load(Ordering::SeqCst), 0);
    assert_eq!(control.charges.load(Ordering::SeqCst), 0);
    assert!(!control.any_busy());
    first.release_writers();
    second.release_writers();
    drop((first, second));
    assert_eq!(control.wakes.load(Ordering::SeqCst), 2);
    assert_eq!(control.charges.load(Ordering::SeqCst), 4);
    assert_cleanup(&control);
    control.armed.store(false, Ordering::SeqCst);
    drop((first_wait, second_wait));
}
