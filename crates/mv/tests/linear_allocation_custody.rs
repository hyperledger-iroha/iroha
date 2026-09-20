//! Actual linear-cell shell reclamation, prepaid MV credits and reader custody.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

use concread::internals::lincowcell::{
    LinCowCell, LinCowCellCapable, OwnedWriteError, WriterAdmission, WriterCharges, WriterLayouts,
};
use mv::allocation::{AllocationBudget, AllocationCharge, AllocationRefusal};

static SERIAL: Mutex<()> = Mutex::new(());
static RECORDS: [Record; 64] = [const { Record::new() }; 64];
static CREATED_WRITERS: AtomicUsize = AtomicUsize::new(0);
static PANIC_CREATE: AtomicBool = AtomicBool::new(false);
static PANIC_PAYLOAD: AtomicUsize = AtomicUsize::new(usize::MAX);
static PANIC_CHARGE: AtomicUsize = AtomicUsize::new(usize::MAX);

struct Record {
    pointer: AtomicUsize,
    size: AtomicUsize,
    align: AtomicUsize,
    freed: AtomicBool,
    payload_drops: AtomicUsize,
    charge_drops: AtomicUsize,
}

impl Record {
    const fn new() -> Self {
        Self {
            pointer: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
            align: AtomicUsize::new(0),
            freed: AtomicBool::new(false),
            payload_drops: AtomicUsize::new(0),
            charge_drops: AtomicUsize::new(0),
        }
    }
}

#[derive(Clone, Copy)]
struct Expected {
    id: usize,
    layout: Layout,
}

thread_local! {
    static EXPECTED: Cell<[Option<Expected>; 2]> = const { Cell::new([None; 2]) };
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

struct ObservedAllocator;

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: preserve the original allocation contract.
        let pointer = unsafe { System.alloc(layout) };
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        if !pointer.is_null() {
            let _ = EXPECTED.try_with(|pending| {
                let mut slots = pending.get();
                if let Some(index) = slots
                    .iter()
                    .position(|slot| slot.is_some_and(|value| value.layout == layout))
                {
                    let expected = slots[index].take().unwrap();
                    let record = &RECORDS[expected.id];
                    record.size.store(layout.size(), SeqCst);
                    record.align.store(layout.align(), SeqCst);
                    record.pointer.store(pointer as usize, SeqCst);
                    pending.set(slots);
                }
            });
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // Observe successful physical deallocation, not entry into payload Drop.
        unsafe { System.dealloc(pointer, layout) };
        for record in &RECORDS {
            if record.pointer.load(SeqCst) == pointer as usize {
                record.freed.store(true, SeqCst);
            }
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn reset() {
    for record in &RECORDS {
        record.pointer.store(0, SeqCst);
        record.size.store(0, SeqCst);
        record.align.store(0, SeqCst);
        record.freed.store(false, SeqCst);
        record.payload_drops.store(0, SeqCst);
        record.charge_drops.store(0, SeqCst);
    }
    CREATED_WRITERS.store(0, SeqCst);
    PANIC_CREATE.store(false, SeqCst);
    PANIC_PAYLOAD.store(usize::MAX, SeqCst);
    PANIC_CHARGE.store(usize::MAX, SeqCst);
    EXPECTED.with(|pending| pending.set([None; 2]));
}

fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    ALLOCATIONS.with(|count| assert!(count.replace(Some(0)).is_none()));
    let result = operation();
    let count = ALLOCATIONS.with(|count| count.replace(None).unwrap());
    assert_eq!(count, 0);
    result
}

#[derive(Debug)]
struct Charge {
    id: usize,
    credit: AllocationCharge,
}

impl Drop for Charge {
    fn drop(&mut self) {
        let record = &RECORDS[self.id];
        assert_ne!(record.pointer.load(SeqCst), 0);
        assert!(record.freed.load(SeqCst), "refund preceded actual free");
        assert_eq!(record.size.load(SeqCst), self.credit.layout().size());
        assert_eq!(record.align.load(SeqCst), self.credit.layout().align());
        assert_eq!(record.charge_drops.fetch_add(1, SeqCst), 0);
        assert_ne!(PANIC_CHARGE.load(SeqCst), self.id, "charge drop panic");
    }
}

#[derive(Debug)]
struct Data {
    value: usize,
}

// Distinct alignment makes the two original allocations independently observable
// without depending on field offsets or the layout of a standard-library Arc.
#[derive(Debug)]
#[repr(align(128))]
struct Reader {
    id: usize,
    value: usize,
}

#[derive(Debug)]
#[repr(align(256))]
struct Writer {
    cursor_id: usize,
    reader_id: usize,
    value: usize,
}

impl Drop for Reader {
    fn drop(&mut self) {
        assert_eq!(RECORDS[self.id].payload_drops.fetch_add(1, SeqCst), 0);
        assert_eq!(RECORDS[self.id].charge_drops.load(SeqCst), 0);
        assert_ne!(PANIC_PAYLOAD.load(SeqCst), self.id, "reader drop panic");
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        assert_eq!(
            RECORDS[self.cursor_id].payload_drops.fetch_add(1, SeqCst),
            0
        );
        assert_eq!(RECORDS[self.cursor_id].charge_drops.load(SeqCst), 0);
    }
}

impl LinCowCellCapable<Reader, Writer> for Data {
    type WriterInput = usize;

    fn create_reader(&self) -> Reader {
        assert_ne!(RECORDS[0].pointer.load(SeqCst), 0);
        Reader {
            id: 0,
            value: self.value,
        }
    }

    fn create_writer(&self, cursor_id: Self::WriterInput) -> Writer {
        CREATED_WRITERS.fetch_add(1, SeqCst);
        assert_ne!(RECORDS[cursor_id].pointer.load(SeqCst), 0);
        assert_ne!(RECORDS[cursor_id + 1].pointer.load(SeqCst), 0);
        assert!(!PANIC_CREATE.load(SeqCst), "cursor construction panic");
        Writer {
            cursor_id,
            reader_id: cursor_id + 1,
            value: self.value,
        }
    }

    fn pre_commit(&mut self, new: Writer, _prev: &Reader) -> Reader {
        assert!(RECORDS[new.cursor_id].freed.load(SeqCst));
        assert_eq!(RECORDS[new.cursor_id].charge_drops.load(SeqCst), 0);
        self.value = new.value;
        Reader {
            id: new.reader_id,
            value: new.value,
        }
    }
}

type CellOwner = LinCowCell<Data, Reader, Writer, Charge>;

fn cell(budget: &AllocationBudget) -> CellOwner {
    let layout = CellOwner::reader_allocation_layout();
    let mut prepaid = budget.try_reserve(layout).unwrap();
    let charge = Charge {
        id: 0,
        credit: prepaid.try_split(layout).unwrap(),
    };
    EXPECTED.with(|pending| pending.set([Some(Expected { id: 0, layout }), None]));
    CellOwner::new_charged(Data { value: 7 }, charge)
}

fn charges(
    budget: &AllocationBudget,
    _data: &Data,
    layouts: WriterLayouts,
    id: usize,
) -> Result<WriterAdmission<Charge, usize>, AllocationRefusal> {
    let mut prepaid = budget.try_reserve_layouts([layouts.cursor, layouts.reader])?;
    let cursor = Charge {
        id,
        credit: prepaid.try_split(layouts.cursor).unwrap(),
    };
    let reader = Charge {
        id: id + 1,
        credit: prepaid.try_split(layouts.reader).unwrap(),
    };
    EXPECTED.with(|pending| {
        pending.set([
            Some(Expected {
                id,
                layout: layouts.cursor,
            }),
            Some(Expected {
                id: id + 1,
                layout: layouts.reader,
            }),
        ])
    });
    Ok(WriterAdmission {
        charges: WriterCharges { cursor, reader },
        input: id,
    })
}

fn refunded(id: usize, payloads: usize) {
    let record = &RECORDS[id];
    assert!(record.freed.load(SeqCst));
    assert_eq!(record.payload_drops.load(SeqCst), payloads);
    assert_eq!(record.charge_drops.load(SeqCst), 1);
}

#[test]
fn complete_prepaid_shell_refusal_and_busy_do_not_construct_or_allocate() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let initial = CellOwner::reader_allocation_layout().size();
    let budget = AllocationBudget::new(initial);
    let owner = cell(&budget);
    without_allocations(|| {
        let refused = owner.write_charged(|data, layouts| charges(&budget, data, layouts, 1));
        assert!(matches!(
            refused,
            Err(AllocationRefusal::ExceedsLimit { .. })
        ));
    });
    assert_eq!(CREATED_WRITERS.load(SeqCst), 0);
    assert_eq!(budget.reserved_bytes(), initial);
    drop(owner);
    refunded(0, 1);
    assert_eq!(budget.reserved_bytes(), 0);

    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = cell(&budget);
    let writer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 1))
        .unwrap();
    without_allocations(|| {
        assert!(
            owner
                .try_write_charged::<()>(|_, _| panic!("busy writer admitted another cursor"))
                .unwrap()
                .is_none()
        );
    });
    drop(writer);
    refunded(1, 1);
    refunded(2, 0);
    drop(owner);
    refunded(0, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_shells_survive_detach_busy_abort_and_retained_reader_chain() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = cell(&budget);
    let oldest = owner.read();
    let mut writer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 1))
        .unwrap();
    writer.value = 11;
    let original = std::ptr::from_ref(&*writer);
    let detached = without_allocations(|| writer.detach());
    let held = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 3))
        .unwrap();
    let (detached, reason) = without_allocations(|| owner.try_write_owned(detached).unwrap_err());
    assert_eq!(reason, OwnedWriteError::Busy);
    drop(held);
    refunded(3, 1);
    refunded(4, 0);
    let writer = without_allocations(|| owner.try_write_owned(detached).unwrap());
    assert_eq!(std::ptr::from_ref(&*writer), original);
    without_allocations(|| writer.commit());
    refunded(1, 1);
    assert_eq!(oldest.value, 7);
    let middle = owner.read();
    assert_eq!(middle.value, 11);

    let mut writer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 5))
        .unwrap();
    writer.value = 19;
    without_allocations(|| writer.commit());
    refunded(5, 1);
    drop(middle);
    assert_eq!(RECORDS[2].charge_drops.load(SeqCst), 0);
    without_allocations(|| drop(oldest));
    refunded(0, 1);
    refunded(2, 1);
    assert_eq!(
        budget.reserved_bytes(),
        CellOwner::reader_allocation_layout().size()
    );
    without_allocations(|| drop(owner));
    refunded(6, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn changed_detached_writer_keeps_original_charges_after_source_cell_drop() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = cell(&budget);
    let writer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 1))
        .unwrap();
    let original = std::ptr::from_ref(&*writer);
    let detached = writer.detach();
    let mut newer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 3))
        .unwrap();
    newer.value = 23;
    newer.commit();
    refunded(3, 1);
    let reserved = budget.reserved_bytes();
    let (detached, reason) = without_allocations(|| owner.try_write_owned(detached).unwrap_err());
    assert_eq!(reason, OwnedWriteError::Changed);
    assert_eq!(std::ptr::from_ref(detached.as_ref()), original);
    assert_eq!(budget.reserved_bytes(), reserved);
    without_allocations(|| drop(owner));
    assert_eq!(budget.reserved_bytes(), reserved);
    for id in [0, 1, 2, 4] {
        assert!(!RECORDS[id].freed.load(SeqCst));
        assert_eq!(RECORDS[id].charge_drops.load(SeqCst), 0);
    }
    without_allocations(|| drop(detached));
    refunded(1, 1);
    refunded(2, 0);
    refunded(0, 1);
    refunded(4, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn construction_unwind_frees_both_empty_shells_and_preserves_original_reader() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = cell(&budget);
    let old = owner.read();
    PANIC_CREATE.store(true, SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || owner.write_charged(|data, layouts| charges(&budget, data, layouts, 1))
        ))
        .is_err()
    );
    assert!(owner.is_poisoned());
    assert_eq!(old.value, 7);
    refunded(1, 0);
    refunded(2, 0);
    assert_eq!(
        budget.reserved_bytes(),
        CellOwner::reader_allocation_layout().size()
    );
    drop(old);
    drop(owner);
    refunded(0, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn payload_unwind_frees_control_block_without_falsely_refunding_its_charge() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = cell(&budget);
    PANIC_PAYLOAD.store(0, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| drop(owner))).is_err());
    assert!(RECORDS[0].freed.load(SeqCst));
    assert_eq!(RECORDS[0].payload_drops.load(SeqCst), 1);
    assert_eq!(RECORDS[0].charge_drops.load(SeqCst), 0);
    assert_eq!(
        budget.reserved_bytes(),
        CellOwner::reader_allocation_layout().size()
    );
}

#[test]
fn cursor_charge_destructor_runs_only_after_complete_unlocked_publication() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = cell(&budget);
    let old = owner.read();
    let mut writer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 1))
        .unwrap();
    writer.value = 31;
    PANIC_CHARGE.store(1, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| writer.commit())).is_err());
    assert!(!owner.is_poisoned());
    assert_eq!(owner.read().value, 31);
    assert_eq!(old.value, 7);
    refunded(1, 1);
    drop(old);
    drop(owner);
    refunded(0, 1);
    refunded(2, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

struct Reenter {
    owner: Arc<CellOwner>,
    wakes: AtomicUsize,
    observed_value: AtomicUsize,
    writer_released: AtomicBool,
    poisoned: AtomicBool,
}

impl Wake for Reenter {
    fn wake(self: Arc<Self>) {
        self.observed_value.store(self.owner.read().value, SeqCst);
        let admitted = matches!(
            self.owner
                .try_write_charged(|_, _| Err::<WriterAdmission<Charge, usize>, _>(())),
            Err(())
        );
        let poisoned = self.owner.is_poisoned();
        self.writer_released.store(admitted || poisoned, SeqCst);
        self.poisoned.store(poisoned, SeqCst);
        for record in &RECORDS {
            if record.charge_drops.load(SeqCst) > 0 {
                assert!(record.freed.load(SeqCst));
            }
        }
        self.wakes.fetch_add(1, SeqCst);
    }
}

#[test]
fn last_concurrent_readers_refund_once_and_wake_reentrant_retry_after_free() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let layouts = CellOwner::writer_allocation_layouts();
    let budget = AllocationBudget::new(3 * layouts.reader.size() + layouts.cursor.size());
    let owner = Arc::new(cell(&budget));
    let oldest_a = owner.read();
    let oldest_b = owner.read();
    for (id, value) in [(1, 11), (3, 19)] {
        let mut writer = owner
            .write_charged(|data, layouts| charges(&budget, data, layouts, id))
            .unwrap();
        writer.value = value;
        writer.commit();
        refunded(id, 1);
    }
    let refusal = owner
        .try_write_charged(|data, layouts| charges(&budget, data, layouts, 5))
        .unwrap_err();
    let AllocationRefusal::Capacity { release, .. } = refusal else {
        panic!("original reader generations must hold the missing capacity");
    };
    let wake = Arc::new(Reenter {
        owner: Arc::clone(&owner),
        wakes: AtomicUsize::new(0),
        observed_value: AtomicUsize::new(0),
        writer_released: AtomicBool::new(false),
        poisoned: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut wait = Box::pin(release.wait_for_release());
    assert!(wait.as_mut().poll(&mut context).is_pending());
    assert_eq!(wake.wakes.load(SeqCst), 0);
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        let barrier = &barrier;
        scope.spawn(move || {
            barrier.wait();
            drop(oldest_a);
        });
        scope.spawn(move || {
            barrier.wait();
            drop(oldest_b);
        });
    });
    assert!(wait.as_mut().poll(&mut context).is_ready());
    assert_eq!(wake.wakes.load(SeqCst), 1);
    assert_eq!(wake.observed_value.load(SeqCst), 19);
    assert!(wake.writer_released.load(SeqCst));
    assert!(!wake.poisoned.load(SeqCst));
    refunded(0, 1);
    refunded(2, 1);
    assert_eq!(budget.reserved_bytes(), layouts.reader.size());
    let retry = owner
        .try_write_charged(|data, layouts| charges(&budget, data, layouts, 5))
        .unwrap()
        .unwrap();
    drop(retry);
    refunded(5, 1);
    refunded(6, 0);
    drop(wait);
    drop(waker);
    drop(wake);
    drop(owner);
    refunded(4, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn abort_and_construction_unwind_unlock_before_refund_reenters_writer() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for unwind in [false, true] {
        reset();
        let layouts = CellOwner::writer_allocation_layouts();
        let budget = AllocationBudget::new(2 * layouts.reader.size() + layouts.cursor.size());
        let owner = Arc::new(cell(&budget));
        let wake = Arc::new(Reenter {
            owner: Arc::clone(&owner),
            wakes: AtomicUsize::new(0),
            observed_value: AtomicUsize::new(0),
            writer_released: AtomicBool::new(false),
            poisoned: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&wake));
        let mut context = Context::from_waker(&waker);
        let mut wait = None;
        PANIC_CREATE.store(unwind, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let writer = owner
                .write_charged(|data, layouts| {
                    let charges = charges(&budget, data, layouts, 1)?;
                    let AllocationRefusal::Capacity { release, .. } =
                        budget.try_reserve(Layout::new::<u8>()).unwrap_err()
                    else {
                        panic!("original shells must occupy the entire pool");
                    };
                    let mut pending = Box::pin(release.wait_for_release());
                    assert!(pending.as_mut().poll(&mut context).is_pending());
                    wait = Some(pending);
                    Ok::<_, AllocationRefusal>(charges)
                })
                .unwrap();
            assert_eq!(wake.wakes.load(SeqCst), 0);
            without_allocations(|| drop(writer));
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(wake.wakes.load(SeqCst), 1);
        assert_eq!(wake.observed_value.load(SeqCst), 7);
        assert!(wake.writer_released.load(SeqCst));
        assert_eq!(wake.poisoned.load(SeqCst), unwind);
        assert!(
            wait.as_mut()
                .unwrap()
                .as_mut()
                .poll(&mut context)
                .is_ready()
        );
        refunded(1, usize::from(!unwind));
        refunded(2, 0);
        drop(wait);
        drop(waker);
        drop(wake);
        drop(owner);
        refunded(0, 1);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn scoped_old_reader_refund_frees_original_storage_before_unlock_notification() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let layouts = CellOwner::writer_allocation_layouts();
    let budget = AllocationBudget::new(3 * layouts.reader.size() + layouts.cursor.size());
    let owner = Arc::new(cell(&budget));
    let oldest = owner.read();
    let mut writer = owner
        .write_charged(|data, layouts| charges(&budget, data, layouts, 1))
        .unwrap();
    writer.value = 11;
    writer.commit();
    refunded(1, 1);
    let wake = Arc::new(Reenter {
        owner: Arc::clone(&owner),
        wakes: AtomicUsize::new(0),
        observed_value: AtomicUsize::new(0),
        writer_released: AtomicBool::new(false),
        poisoned: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut wait = None;
    budget.with_deferred_refund_notifications(|| {
        let held = owner
            .write_charged(|data, layouts| charges(&budget, data, layouts, 3))
            .unwrap();
        let AllocationRefusal::Capacity { release, .. } =
            budget.try_reserve(Layout::new::<u8>()).unwrap_err()
        else {
            panic!("original generations must fill the pool");
        };
        let mut pending = Box::pin(release.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        wait = Some(pending);
        without_allocations(|| {
            drop(oldest);
            // This is an allocator-level free witness even while the original
            // physical writer remains held, not just deferred charge Drop.
            refunded(0, 1);
            assert_eq!(
                budget.reserved_bytes(),
                2 * layouts.reader.size() + layouts.cursor.size()
            );
            assert_eq!(wake.wakes.load(SeqCst), 0);
            drop(held);
            refunded(3, 1);
            refunded(4, 0);
            assert_eq!(budget.reserved_bytes(), layouts.reader.size());
            assert_eq!(wake.wakes.load(SeqCst), 0);
        });
    });
    assert_eq!(wake.wakes.load(SeqCst), 1);
    assert_eq!(wake.observed_value.load(SeqCst), 11);
    assert!(wake.writer_released.load(SeqCst));
    assert!(!wake.poisoned.load(SeqCst));
    assert!(
        wait.as_mut()
            .unwrap()
            .as_mut()
            .poll(&mut context)
            .is_ready()
    );
    drop(wait);
    drop(waker);
    drop(wake);
    drop(owner);
    refunded(2, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

// This owns an actual separately allocated payload, not a stand-in reservation.
// Its original charge is released only after the observed Box is deallocated.
struct OperationInput {
    payload: std::mem::ManuallyDrop<Box<Reader>>,
    charge: std::mem::ManuallyDrop<Charge>,
}

impl Drop for OperationInput {
    fn drop(&mut self) {
        // SAFETY: both fields are uniquely owned and destroyed exactly once.
        // If payload destruction panics, retain its charge conservatively.
        unsafe {
            std::mem::ManuallyDrop::drop(&mut self.payload);
            std::mem::ManuallyDrop::drop(&mut self.charge);
        }
    }
}

struct InputData(Data);
struct InputWriter {
    base: Writer,
    input: OperationInput,
}

impl LinCowCellCapable<Reader, InputWriter> for InputData {
    type WriterInput = OperationInput;

    fn create_reader(&self) -> Reader {
        self.0.create_reader()
    }

    fn create_writer(&self, input: Self::WriterInput) -> InputWriter {
        InputWriter {
            base: self.0.create_writer(1),
            input,
        }
    }

    fn pre_commit(&mut self, writer: InputWriter, previous: &Reader) -> Reader {
        self.0.pre_commit(writer.base, previous)
    }
}

type InputCell = LinCowCell<InputData, Reader, InputWriter, Charge>;

fn input_cell(budget: &AllocationBudget) -> InputCell {
    let layout = InputCell::reader_allocation_layout();
    let mut prepaid = budget.try_reserve(layout).unwrap();
    EXPECTED.with(|pending| pending.set([Some(Expected { id: 0, layout }), None]));
    InputCell::new_charged(
        InputData(Data { value: 7 }),
        Charge {
            id: 0,
            credit: prepaid.try_split(layout).unwrap(),
        },
    )
}

fn input_admission(
    budget: &AllocationBudget,
    layouts: WriterLayouts,
) -> Result<WriterAdmission<Charge, OperationInput>, AllocationRefusal> {
    let payload_layout = Layout::new::<Reader>();
    let mut prepaid =
        budget.try_reserve_layouts([layouts.cursor, layouts.reader, payload_layout])?;
    EXPECTED.with(|pending| {
        pending.set([
            Some(Expected {
                id: 3,
                layout: payload_layout,
            }),
            None,
        ])
    });
    let input = OperationInput {
        payload: std::mem::ManuallyDrop::new(Box::new(Reader { id: 3, value: 29 })),
        charge: std::mem::ManuallyDrop::new(Charge {
            id: 3,
            credit: prepaid.try_split(payload_layout).unwrap(),
        }),
    };
    EXPECTED.with(|pending| {
        pending.set([
            Some(Expected {
                id: 1,
                layout: layouts.cursor,
            }),
            Some(Expected {
                id: 2,
                layout: layouts.reader,
            }),
        ])
    });
    Ok(WriterAdmission {
        charges: WriterCharges {
            cursor: Charge {
                id: 1,
                credit: prepaid.try_split(layouts.cursor).unwrap(),
            },
            reader: Charge {
                id: 2,
                credit: prepaid.try_split(layouts.reader).unwrap(),
            },
        },
        input,
    })
}

#[test]
fn original_admitted_input_survives_refusal_detach_and_reattach_without_readmission() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset();
    let budget = AllocationBudget::new(1 << 20);
    let owner = input_cell(&budget);
    let admissions = AtomicUsize::new(0);
    let input_pointer = AtomicUsize::new(0);
    without_allocations(|| {
        assert!(matches!(
            owner.write_charged(|_, _| Err::<WriterAdmission<Charge, OperationInput>, _>(())),
            Err(())
        ));
    });
    assert_eq!(CREATED_WRITERS.load(SeqCst), 0);
    let mut writer = owner
        .write_charged(|data, layouts| {
            assert_eq!(data.0.value, 7);
            assert!(
                owner
                    .try_write_charged::<()>(|_, _| panic!("admission must hold the original lock"))
                    .unwrap()
                    .is_none()
            );
            admissions.fetch_add(1, SeqCst);
            let admitted = input_admission(&budget, layouts)?;
            input_pointer.store(
                (&**admitted.input.payload) as *const Reader as usize,
                SeqCst,
            );
            Ok::<_, AllocationRefusal>(admitted)
        })
        .unwrap();
    let cursor_pointer = writer.as_ref() as *const InputWriter;
    assert_eq!(
        (&**writer.input.payload) as *const Reader as usize,
        input_pointer.load(SeqCst)
    );
    writer.base.value = writer.input.payload.value;
    let owned = without_allocations(|| writer.detach());
    let mut retained = Some(owned);
    without_allocations(|| {
        assert!(matches!(
            owner.write_charged(|_, _| {
                let owned = retained.take().unwrap();
                let (owned, error) = match owner.try_write_owned(owned) {
                    Err(refused) => refused,
                    Ok(_) => panic!("reattachment acquired an already held writer"),
                };
                assert_eq!(error, OwnedWriteError::Busy);
                assert_eq!(owned.as_ref() as *const InputWriter, cursor_pointer);
                assert_eq!(
                    (&**owned.as_ref().input.payload) as *const Reader as usize,
                    input_pointer.load(SeqCst)
                );
                retained = Some(owned);
                Err::<WriterAdmission<Charge, OperationInput>, _>(())
            }),
            Err(())
        ));
    });
    let writer = without_allocations(|| {
        owner
            .try_write_owned(retained.take().unwrap())
            .unwrap_or_else(|(_, error)| panic!("unchanged original owner refused: {error:?}"))
    });
    assert_eq!(writer.as_ref() as *const InputWriter, cursor_pointer);
    assert_eq!(
        (&**writer.input.payload) as *const Reader as usize,
        input_pointer.load(SeqCst)
    );
    budget.with_deferred_refund_notifications(|| without_allocations(|| writer.commit()));
    assert_eq!(owner.read().value, 29);
    assert_eq!(CREATED_WRITERS.load(SeqCst), 1);
    drop(owner);
    assert_eq!(admissions.load(SeqCst), 1);
    refunded(1, 1);
    refunded(2, 1);
    refunded(3, 1);
    refunded(0, 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_input_abort_and_constructor_panic_refund_after_original_scope_unlocks() {
    struct Retry {
        owner: Arc<InputCell>,
        wakes: AtomicUsize,
        unlocked: AtomicBool,
    }
    impl Wake for Retry {
        fn wake(self: Arc<Self>) {
            let probe = self
                .owner
                .try_write_charged(|_, _| Err::<WriterAdmission<Charge, OperationInput>, _>(()));
            let unlocked =
                matches!(probe, Err(())) || (matches!(probe, Ok(None)) && self.owner.is_poisoned());
            self.unlocked.store(unlocked, SeqCst);
            self.wakes.fetch_add(1, SeqCst);
        }
    }

    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for panic_create in [false, true] {
        reset();
        let layouts = InputCell::writer_allocation_layouts();
        let budget = AllocationBudget::new(
            2 * layouts.reader.size() + layouts.cursor.size() + Layout::new::<Reader>().size(),
        );
        let owner = Arc::new(input_cell(&budget));
        let wake = Arc::new(Retry {
            owner: Arc::clone(&owner),
            wakes: AtomicUsize::new(0),
            unlocked: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&wake));
        let mut context = Context::from_waker(&waker);
        let mut wait = None;
        PANIC_CREATE.store(panic_create, SeqCst);
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|| {
                let writer = owner
                    .write_charged(|_, layouts| {
                        let admission = input_admission(&budget, layouts)?;
                        let AllocationRefusal::Capacity { release, .. } =
                            budget.try_reserve(Layout::new::<u8>()).unwrap_err()
                        else {
                            panic!("the original shells and input must occupy the entire pool");
                        };
                        let mut pending = Box::pin(release.wait_for_release());
                        assert!(pending.as_mut().poll(&mut context).is_pending());
                        wait = Some(pending);
                        Ok::<_, AllocationRefusal>(admission)
                    })
                    .unwrap();
                without_allocations(|| drop(writer));
                assert_eq!(wake.wakes.load(SeqCst), 0);
            });
        }));
        assert_eq!(result.is_err(), panic_create);
        assert_eq!(wake.wakes.load(SeqCst), 1);
        assert!(wake.unlocked.load(SeqCst));
        assert_eq!(owner.is_poisoned(), panic_create);
        assert!(
            wait.as_mut()
                .unwrap()
                .as_mut()
                .poll(&mut context)
                .is_ready()
        );
        assert_eq!(CREATED_WRITERS.load(SeqCst), 1);
        refunded(1, usize::from(!panic_create));
        refunded(2, 0);
        refunded(3, 1);
        assert_eq!(budget.reserved_bytes(), layouts.reader.size());
        drop(wait);
        drop(waker);
        drop(wake);
        drop(owner);
        refunded(0, 1);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
