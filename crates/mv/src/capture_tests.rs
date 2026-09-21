//! Capture slots keep exact journals and callbacks behind every physical sibling.

use crate::{
    BlockCapture, BlockMode, ReleaseNotification,
    allocation::{AllocationBudget, AllocationCharge},
    cell::{self, Cell, CellAllocationCharges},
    storage::{self, Storage, StorageReadOnly},
};
use concread::release::{ReleaseFuture, ReleaseWait};
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

struct Control {
    target: Mutex<Weak<Target>>,
    budget: AllocationBudget,
    armed: AtomicBool,
    fail_clone: AtomicBool,
    clones: AtomicUsize,
    wakes: AtomicUsize,
    wake_busy: AtomicUsize,
    payload_drops: AtomicUsize,
    payload_busy: AtomicUsize,
    charge_drops: AtomicUsize,
    charge_busy: AtomicUsize,
    admission_drops: AtomicUsize,
    admission_busy: AtomicUsize,
}
struct Payload {
    value: u64,
    copied: bool,
    control: Arc<Control>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        self.control.clones.fetch_add(1, SeqCst);
        assert!(
            !self.control.fail_clone.load(SeqCst),
            "injected native payload copy"
        );
        Self {
            value: self.value,
            copied: true,
            control: Arc::clone(&self.control),
        }
    }
}
impl Drop for Payload {
    fn drop(&mut self) {
        if self.copied && self.control.armed.load(SeqCst) {
            self.control.payload_drops.fetch_add(1, SeqCst);
            if self.control.any_busy() {
                self.control.payload_busy.fetch_add(1, SeqCst);
            }
        }
    }
}
struct Charge {
    credit: AllocationCharge,
    control: Arc<Control>,
    watched: bool,
}
impl Drop for Charge {
    fn drop(&mut self) {
        if self.watched && self.control.armed.load(SeqCst) {
            self.control.charge_drops.fetch_add(1, SeqCst);
            if self.control.any_busy() {
                self.control.charge_busy.fetch_add(1, SeqCst);
            }
        }
        let _ = self.credit.layout();
    }
}
struct Admission {
    control: Arc<Control>,
    panic_on_drop: bool,
}
impl Drop for Admission {
    fn drop(&mut self) {
        if self.control.armed.load(SeqCst) {
            self.control.admission_drops.fetch_add(1, SeqCst);
            if self.control.any_busy() {
                self.control.admission_busy.fetch_add(1, SeqCst);
            }
        }
        assert!(
            !self.panic_on_drop,
            "injected post-unlock admission cleanup"
        );
    }
}
struct Target {
    cell: Cell<Payload, Charge>,
    map: Storage<u64, Payload>,
}
impl Control {
    fn any_busy(&self) -> bool {
        let Ok(target) = self.target.try_lock() else {
            return true;
        };
        let Some(target) = target.upgrade() else {
            return false;
        };
        // Actual raw acquisitions clone no payload and expose poison separately.
        // Each temporary ends before probing the next mutex. No assertion in Wake.
        [
            target.cell.revert.try_acquire_writer().is_none(),
            target.cell.blocks.try_acquire_writer().is_none(),
            target.map.revert.try_acquire_writer().is_none(),
            target.map.blocks.try_acquire_writer().is_none(),
        ]
        .into_iter()
        .any(|busy| busy)
    }
    fn payload(self: &Arc<Self>, value: u64) -> Payload {
        Payload {
            value,
            copied: false,
            control: Arc::clone(self),
        }
    }
    fn charges(self: &Arc<Self>, watched: bool) -> CellAllocationCharges<Charge> {
        let [current, undo] = Cell::<Payload, Charge>::allocation_layouts();
        let mut credit = self.budget.try_reserve_layouts([current, undo]).unwrap();
        CellAllocationCharges::new(
            Charge {
                credit: credit.try_split(current).unwrap(),
                control: Arc::clone(self),
                watched,
            },
            Charge {
                credit: credit.try_split(undo).unwrap(),
                control: Arc::clone(self),
                watched,
            },
        )
    }
    fn admission(self: &Arc<Self>) -> Admission {
        Admission {
            control: Arc::clone(self),
            panic_on_drop: false,
        }
    }
}
impl Wake for Control {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, SeqCst);
        if self.any_busy() {
            self.wake_busy.fetch_add(1, SeqCst);
        }
    }
}
fn fixture() -> (Arc<Target>, Arc<Control>) {
    let control = Arc::new(Control {
        target: Mutex::new(Weak::new()),
        budget: AllocationBudget::new(1 << 20),
        armed: AtomicBool::new(false),
        fail_clone: AtomicBool::new(false),
        clones: AtomicUsize::new(0),
        wakes: AtomicUsize::new(0),
        wake_busy: AtomicUsize::new(0),
        payload_drops: AtomicUsize::new(0),
        payload_busy: AtomicUsize::new(0),
        charge_drops: AtomicUsize::new(0),
        charge_busy: AtomicUsize::new(0),
        admission_drops: AtomicUsize::new(0),
        admission_busy: AtomicUsize::new(0),
    });
    let map = Storage::from_iter([(0, control.payload(10))]);
    {
        let mut tip = map.block();
        tip.insert(0, control.payload(20));
        tip.commit();
    }
    let target = Arc::new(Target {
        cell: Cell::from_values_charged(
            control.payload(20),
            Some(control.payload(10)),
            control.charges(false),
        ),
        map,
    });
    *control.target.lock().unwrap() = Arc::downgrade(&target);
    (target, control)
}
struct Pending<'a> {
    cell: Option<cell::BlockCaptureSlot<'a, Payload, Admission, Charge>>,
    map: Option<storage::BlockCaptureSlot<'a, u64, Payload, Admission>>,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Some(cell) = self.cell.as_mut() {
            cell.release();
        }
        if let Some(map) = self.map.as_mut() {
            map.release();
        }
    }
}
fn originals<'a>(
    target: &'a Target,
    control: &Arc<Control>,
    mode: BlockMode,
) -> (
    cell::Block<'a, Payload, Charge>,
    storage::Block<'a, u64, Payload>,
) {
    let mut cell = match mode {
        BlockMode::Ordinary => target.cell.block_charged(control.charges(true)),
        BlockMode::Replace => target.cell.block_and_revert_charged(control.charges(true)),
    };
    let mut map = match mode {
        BlockMode::Ordinary => target.map.block(),
        BlockMode::Replace => target.map.block_and_revert(),
    };
    cell.get_mut().value = 30;
    map.insert(0, control.payload(30));
    (cell, map)
}
fn register(source: &ReleaseNotification, control: &Arc<Control>) -> (ReleaseWait, ReleaseFuture) {
    let observation = source.observe();
    let mut future = observation.clone().wait_for_release();
    let waker = Waker::from(Arc::clone(control));
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    (observation, future)
}
fn assert_clean(control: &Control) {
    assert!(!control.any_busy());
    assert_eq!(control.wake_busy.load(SeqCst), 0);
    assert_eq!(control.payload_busy.load(SeqCst), 0);
    assert_eq!(control.charge_busy.load(SeqCst), 0);
    assert_eq!(control.admission_busy.load(SeqCst), 0);
}

#[test]
fn capture_slots_keep_exact_ordinary_and_replacement_journals_until_all_writers_release() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        let (target, control) = fixture();
        let baseline_credit = control.budget.reserved_bytes();
        let reader = target.map.snapshot();
        let (cell, map) = originals(&target, &control, mode);
        let before = if mode == BlockMode::Ordinary { 20 } else { 10 };
        let cell_ptr = cell.get() as *const Payload;
        let map_ptr = map.get(&0).unwrap() as *const Payload;
        let clones = control.clones.load(SeqCst);
        let mut pending = Pending {
            cell: Some(cell.capture_slot()),
            map: Some(map.capture_slot()),
        };
        let waits = [
            register(&target.cell.blocks_released, &control),
            register(&target.map.blocks_released, &control),
        ];
        control.armed.store(true, SeqCst);
        pending
            .cell
            .as_mut()
            .unwrap()
            .try_capture(|_| Ok::<_, ()>(control.admission()))
            .unwrap();
        assert_eq!(control.wakes.load(SeqCst), 0);
        assert!(target.cell.blocks.try_acquire_writer().is_some());
        assert!(target.map.blocks.try_acquire_writer().is_none());
        pending
            .map
            .as_mut()
            .unwrap()
            .try_capture(|_| Ok::<_, ()>(control.admission()))
            .unwrap();
        assert_eq!(control.wakes.load(SeqCst), 0);
        let (cell, cell_cleanup) = pending.cell.take().unwrap().into_detached();
        let (map, map_cleanup) = pending.map.take().unwrap().into_detached();
        drop(pending);
        assert_eq!(
            control.clones.load(SeqCst),
            clones,
            "capture must move original payloads"
        );
        assert_eq!(cell.mode(), mode);
        assert_eq!(map.mode(), mode);
        let touched = cell.touched_value().unwrap();
        assert_eq!((touched.before.value, touched.after.value), (before, 30));
        assert_eq!(touched.after as *const Payload, cell_ptr);
        let touched = map.touched_entries().next().unwrap();
        assert_eq!(touched.before.unwrap().value, before);
        assert_eq!(touched.after.unwrap() as *const Payload, map_ptr);
        assert!(cell.matches_current(&target.cell));
        assert!(map.matches_current(&target.map));
        assert_eq!(reader.current().get(&0).unwrap().value, 20);
        assert_eq!(
            reader.revert_map().get(&0).unwrap().as_ref().unwrap().value,
            10
        );
        drop((cell_cleanup, map_cleanup));
        assert_eq!(control.wakes.load(SeqCst), 2);
        assert!(waits.iter().all(|(w, _)| !w.is_poisoned()));
        assert_eq!(control.charge_drops.load(SeqCst), 0);
        drop((cell, map));
        assert_eq!(control.charge_drops.load(SeqCst), 2);
        assert_eq!(control.budget.reserved_bytes(), baseline_credit);
        assert_eq!(control.admission_drops.load(SeqCst), 2);
        assert_clean(&control);
        control.armed.store(false, SeqCst);
    }
}

#[test]
fn capture_slots_keep_successful_sibling_through_admission_refusal_and_caught_panic() {
    for panic in [false, true] {
        let (target, control) = fixture();
        let baseline = control.budget.reserved_bytes();
        let (cell, map) = originals(&target, &control, BlockMode::Replace);
        let mut pending = Pending {
            cell: Some(cell.capture_slot()),
            map: Some(map.capture_slot()),
        };
        let waits = [
            register(&target.cell.blocks_released, &control),
            register(&target.map.blocks_released, &control),
        ];
        control.armed.store(true, SeqCst);
        pending
            .cell
            .as_mut()
            .unwrap()
            .try_capture(|_| Ok::<_, ()>(control.admission()))
            .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            pending.map.as_mut().unwrap().try_capture(|_| {
                assert!(!panic, "injected capture admission panic");
                Err::<Admission, _>("capacity")
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err("capacity"));
        }
        assert_eq!(control.wakes.load(SeqCst), 0);
        assert_eq!(control.admission_drops.load(SeqCst), 0);
        assert_eq!(control.charge_drops.load(SeqCst), 0);
        assert!(target.map.blocks.try_acquire_writer().is_none());
        drop(pending);
        assert_eq!(control.wakes.load(SeqCst), 2);
        assert!(waits.iter().all(|(w, _)| !w.is_poisoned()));
        assert_eq!(control.admission_drops.load(SeqCst), 1);
        assert_eq!(control.charge_drops.load(SeqCst), 2);
        assert_eq!(control.budget.reserved_bytes(), baseline);
        assert_eq!(target.map.view().get(&0).unwrap().value, 20);
        assert_clean(&control);
        control.armed.store(false, SeqCst);
    }
}

#[test]
fn capture_slots_failed_map_precheck_keeps_original_block_for_joint_abandonment() {
    let (target, control) = fixture();
    let baseline = control.budget.reserved_bytes();
    let mut cell = target.cell.block_charged(control.charges(true));
    cell.get_mut().value = 30;
    let mut map = target.map.block();
    // Force an actual edit failure after the healthy first construction; the
    // aggregate failure flag rejects capture before its admission callback.
    control.fail_clone.store(true, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| map.insert(9, control.payload(90)))).is_err());
    control.fail_clone.store(false, SeqCst);
    let mut pending = Pending {
        cell: Some(cell.capture_slot()),
        map: Some(map.capture_slot()),
    };
    let waits = [
        register(&target.cell.blocks_released, &control),
        register(&target.map.blocks_released, &control),
    ];
    control.armed.store(true, SeqCst);
    pending
        .cell
        .as_mut()
        .unwrap()
        .try_capture(|_| Ok::<_, ()>(control.admission()))
        .unwrap();
    let calls = AtomicUsize::new(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| pending
            .map
            .as_mut()
            .unwrap()
            .try_capture(|_| {
                calls.fetch_add(1, SeqCst);
                Ok::<_, ()>(control.admission())
            })))
        .is_err()
    );
    assert_eq!(calls.load(SeqCst), 0);
    assert_eq!(control.wakes.load(SeqCst), 0);
    drop(pending);
    assert_eq!(control.wakes.load(SeqCst), 2);
    assert!(waits.iter().all(|(w, _)| !w.is_poisoned()));
    assert_eq!(control.budget.reserved_bytes(), baseline);
    assert_eq!(target.map.view().get(&0).unwrap().value, 20);
    assert!(target.map.view().get(&9).is_none());
    assert_clean(&control);
    control.armed.store(false, SeqCst);
}

#[test]
fn capture_slots_outer_unwind_preserves_actual_attached_writer_poison_only() {
    let (target, control) = fixture();
    let (cell, map) = originals(&target, &control, BlockMode::Ordinary);
    let pending = Pending {
        cell: Some(cell.capture_slot()),
        map: Some(map.capture_slot()),
    };
    let waits = [
        register(&target.cell.blocks_released, &control),
        register(&target.map.blocks_released, &control),
    ];
    control.armed.store(true, SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut pending = pending;
            pending
                .cell
                .as_mut()
                .unwrap()
                .try_capture(|_| Ok::<_, ()>(control.admission()))
                .unwrap();
            let _ = pending
                .map
                .as_mut()
                .unwrap()
                .try_capture::<()>(|_| panic!("outer admission unwind"));
        }))
        .is_err()
    );
    assert_eq!(control.wakes.load(SeqCst), 2);
    assert!(!waits[0].0.is_poisoned());
    assert!(waits[1].0.is_poisoned());
    assert!(!target.cell.blocks.is_poisoned());
    assert!(!target.cell.revert.is_poisoned());
    assert!(target.map.blocks.is_poisoned());
    assert!(target.map.revert.is_poisoned());
    assert_clean(&control);
    control.armed.store(false, SeqCst);
}

#[test]
fn capture_slots_admission_cleanup_panic_happens_after_all_physical_unlocks() {
    let (target, control) = fixture();
    let (cell, map) = originals(&target, &control, BlockMode::Ordinary);
    let mut pending = Pending {
        cell: Some(cell.capture_slot()),
        map: Some(map.capture_slot()),
    };
    let waits = [
        register(&target.cell.blocks_released, &control),
        register(&target.map.blocks_released, &control),
    ];
    control.armed.store(true, SeqCst);
    pending
        .cell
        .as_mut()
        .unwrap()
        .try_capture(|_| {
            Ok::<_, ()>(Admission {
                control: Arc::clone(&control),
                panic_on_drop: true,
            })
        })
        .unwrap();
    // The map stays attached until Pending's release-all pass. Admission Drop
    // then panics, but the remaining field cleanup must retain its healthy verdict.
    assert!(catch_unwind(AssertUnwindSafe(|| drop(pending))).is_err());
    assert_eq!(control.wakes.load(SeqCst), 2);
    assert!(waits.iter().all(|(w, _)| !w.is_poisoned()));
    assert!(!target.map.blocks.is_poisoned());
    assert!(!target.map.revert.is_poisoned());
    assert_clean(&control);
    control.armed.store(false, SeqCst);
}
