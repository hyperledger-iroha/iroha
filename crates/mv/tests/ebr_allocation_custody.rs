//! Black-box custody checks against the actual EBR allocator and epoch collector.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    time::{Duration, Instant},
};

use concread::ebrcell::EbrCell;

struct ObservedAllocator;

// Tests serialize observation of one exact allocation; the epoch collector may
// free it on another thread, so the allocator observations are process-wide.
static TEST_SERIAL: Mutex<()> = Mutex::new(());
static WATCHED_ALLOCATION: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicBool = AtomicBool::new(false);
static FREED_SIZE: AtomicUsize = AtomicUsize::new(0);
static FREED_ALIGN: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the allocator contract unchanged to System.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let watched = pointer as usize == WATCHED_ALLOCATION.load(SeqCst);
        // SAFETY: forward the original pointer and layout before reporting free.
        unsafe { System.dealloc(pointer, layout) };
        if watched {
            FREED_SIZE.store(layout.size(), SeqCst);
            FREED_ALIGN.store(layout.align(), SeqCst);
            DEALLOCATED.store(true, SeqCst);
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

struct Observations {
    clones: AtomicUsize,
    admitted: [AtomicBool; 4],
    value_drops: [AtomicUsize; 4],
    charge_drops: [AtomicUsize; 4],
    panic_on_clone: AtomicBool,
    panic_on_drop: AtomicUsize,
}

impl Observations {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            clones: AtomicUsize::new(0),
            admitted: std::array::from_fn(|_| AtomicBool::new(false)),
            value_drops: std::array::from_fn(|_| AtomicUsize::new(0)),
            charge_drops: std::array::from_fn(|_| AtomicUsize::new(0)),
            panic_on_clone: AtomicBool::new(false),
            panic_on_drop: AtomicUsize::new(usize::MAX),
        })
    }
}

struct Value {
    generation: usize,
    observations: Arc<Observations>,
    value: u64,
}

impl Clone for Value {
    fn clone(&self) -> Self {
        let generation = self.observations.clones.fetch_add(1, SeqCst) + 1;
        assert!(self.observations.admitted[generation].load(SeqCst));
        assert!(
            !self.observations.panic_on_clone.load(SeqCst),
            "injected clone panic"
        );
        Self {
            generation,
            observations: Arc::clone(&self.observations),
            value: self.value,
        }
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        self.observations.value_drops[self.generation].fetch_add(1, SeqCst);
        assert_ne!(
            self.observations.panic_on_drop.load(SeqCst),
            self.generation,
            "injected payload destructor panic"
        );
    }
}

// Deliberately not Clone: every generation must retain its original charge.
struct Charge {
    generation: usize,
    observations: Arc<Observations>,
    check_allocation_free: bool,
}

impl Charge {
    fn admit(observations: &Arc<Observations>, generation: usize, check_free: bool) -> Self {
        assert!(!observations.admitted[generation].swap(true, SeqCst));
        Self {
            generation,
            observations: Arc::clone(observations),
            check_allocation_free: check_free,
        }
    }
}

impl Drop for Charge {
    fn drop(&mut self) {
        assert_eq!(
            self.observations.value_drops[self.generation].load(SeqCst),
            1
        );
        if self.check_allocation_free {
            assert!(
                DEALLOCATED.load(SeqCst),
                "charge returned before allocation free"
            );
            let layout = EbrCell::<Value, Charge>::allocation_layout();
            assert_eq!(FREED_SIZE.load(SeqCst), layout.size());
            assert_eq!(FREED_ALIGN.load(SeqCst), layout.align());
            WATCHED_ALLOCATION.store(0, SeqCst);
        }
        self.observations.charge_drops[self.generation].fetch_add(1, SeqCst);
    }
}

fn cell(observations: &Arc<Observations>) -> EbrCell<Value, Charge> {
    let charge = Charge::admit(observations, 0, false);
    EbrCell::new_charged(
        Value {
            generation: 0,
            observations: Arc::clone(observations),
            value: 7,
        },
        charge,
    )
}

fn watch(value: &Value) {
    DEALLOCATED.store(false, SeqCst);
    FREED_SIZE.store(0, SeqCst);
    FREED_ALIGN.store(0, SeqCst);
    // Concread's allocation is repr(C), with the non-ZST payload first. This
    // records the actual allocation pointer, not an estimated object footprint.
    WATCHED_ALLOCATION.store(std::ptr::from_ref(value) as usize, SeqCst);
}

fn collect_until(mut reclaimed: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !reclaimed() {
        assert!(
            Instant::now() < deadline,
            "epoch reclamation did not complete"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}

#[test]
fn admission_refusal_and_contention_never_clone_and_abort_frees_before_charge() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let observations = Observations::new();
    let cell = cell(&observations);
    let refused = cell.write_charged(|original, layout| {
        assert_eq!(original.value, 7);
        assert_eq!(layout, EbrCell::<Value, Charge>::allocation_layout());
        Err::<Charge, _>("capacity exhausted")
    });
    assert!(matches!(refused, Err("capacity exhausted")));
    drop(refused);
    assert_eq!(observations.clones.load(SeqCst), 0);
    let mut writer = cell
        .try_write_charged(|_, _| Ok::<_, ()>(Charge::admit(&observations, 1, true)))
        .unwrap()
        .expect("refusal released original writer");
    writer.get_mut().value = 23;
    assert_eq!(cell.read().value, 7);
    watch(&writer);
    let contended = cell.try_write_charged(|_, _| -> Result<Charge, ()> {
        panic!("contention must not invoke admission")
    });
    assert!(matches!(contended, Ok(None)));
    drop(contended);
    assert_eq!(observations.clones.load(SeqCst), 1);
    drop(writer);
    assert_eq!(observations.value_drops[1].load(SeqCst), 1);
    assert_eq!(observations.charge_drops[1].load(SeqCst), 1);
    assert_eq!(cell.read().value, 7);
    drop(cell);
    collect_until(|| observations.charge_drops[0].load(SeqCst) == 1);
}

#[test]
fn committed_allocation_and_charge_wait_for_unrelated_epoch_pin() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let observations = Observations::new();
    let cell = cell(&observations);
    let unrelated_pin = crossbeam_epoch::pin();
    let original_reader = cell.read();
    let mut writer = cell
        .write_charged(|_, _| Ok::<_, ()>(Charge::admit(&observations, 1, true)))
        .unwrap();
    writer.value = 29;
    watch(&writer);
    let original_writer_allocation = std::ptr::from_ref::<Value>(&writer);
    writer.commit();
    let current_reader = cell.read();
    assert_eq!(
        std::ptr::from_ref::<Value>(&current_reader),
        original_writer_allocation
    );
    assert_eq!(original_reader.value, 7);
    assert_eq!(current_reader.value, 29);
    drop(original_reader);
    drop(current_reader);
    drop(cell);
    // No reader or cell remains. Only this unrelated epoch pin blocks collection.
    unrelated_pin.flush();
    std::thread::spawn(|| {
        for _ in 0..1024 {
            crossbeam_epoch::pin().flush();
        }
    })
    .join()
    .unwrap();
    for generation in 0..2 {
        assert_eq!(observations.value_drops[generation].load(SeqCst), 0);
        assert_eq!(observations.charge_drops[generation].load(SeqCst), 0);
    }
    assert!(!DEALLOCATED.load(SeqCst));
    drop(unrelated_pin);
    collect_until(|| {
        (0..2).all(|generation| observations.charge_drops[generation].load(SeqCst) == 1)
    });
    assert!(DEALLOCATED.load(SeqCst));
}

#[test]
fn clone_panic_conservatively_retains_admitted_charge() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let observations = Observations::new();
    let cell = cell(&observations);
    observations.panic_on_clone.store(true, SeqCst);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _writer =
            cell.write_charged(|_, _| Ok::<_, ()>(Charge::admit(&observations, 1, false)));
    }));
    assert!(panic.is_err());
    assert!(cell.is_poisoned());
    assert!(observations.admitted[1].load(SeqCst));
    assert_eq!(observations.value_drops[1].load(SeqCst), 0);
    assert_eq!(observations.charge_drops[1].load(SeqCst), 0);
    assert_eq!(cell.read().value, 7);
    drop(cell);
    collect_until(|| observations.charge_drops[0].load(SeqCst) == 1);
    assert_eq!(observations.charge_drops[1].load(SeqCst), 0);
}

#[test]
fn destructor_panic_conservatively_retains_charge_even_if_outer_allocation_frees() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let observations = Observations::new();
    let cell = cell(&observations);
    let writer = cell
        .write_charged(|_, _| Ok::<_, ()>(Charge::admit(&observations, 1, true)))
        .unwrap();
    watch(&writer);
    observations.panic_on_drop.store(1, SeqCst);
    let panic = catch_unwind(AssertUnwindSafe(|| drop(writer)));
    assert!(panic.is_err());
    assert_eq!(observations.value_drops[1].load(SeqCst), 1);
    assert!(DEALLOCATED.load(SeqCst));
    assert_eq!(observations.charge_drops[1].load(SeqCst), 0);
    WATCHED_ALLOCATION.store(0, SeqCst);
    drop(cell);
    collect_until(|| observations.charge_drops[0].load(SeqCst) == 1);
    assert_eq!(observations.charge_drops[1].load(SeqCst), 0);
}

#[test]
fn detached_generation_retries_with_original_allocation_and_no_installation_clone() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let observations = Observations::new();
    let cell = cell(&observations);
    assert!(!cell.is_poisoned());
    let original_reader = cell.read();
    let mut writer = cell
        .write_charged(|_, _| Ok::<_, ()>(Charge::admit(&observations, 1, true)))
        .unwrap();
    writer.value = 41;
    watch(&writer);
    let original_pointer = std::ptr::from_ref::<Value>(&writer);
    let owner = writer.detach();
    assert_eq!(std::ptr::from_ref::<Value>(&owner), original_pointer);
    assert_eq!(observations.charge_drops[1].load(SeqCst), 0);
    assert_eq!(cell.read().value, 7);

    let contender = cell
        .write_charged(|_, _| Ok::<_, ()>(Charge::admit(&observations, 2, false)))
        .unwrap();
    let owner = match cell.try_write_owned(owner) {
        Err(owner) => owner,
        Ok(_) => panic!("held writer must return original detached ownership"),
    };
    assert_eq!(std::ptr::from_ref::<Value>(&owner), original_pointer);
    assert_eq!(observations.clones.load(SeqCst), 2);
    drop(contender);
    let writer = cell
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("released writer must accept retained generation"));
    assert_eq!(std::ptr::from_ref::<Value>(&writer), original_pointer);
    assert_eq!(observations.clones.load(SeqCst), 2);

    let owner = writer.detach();
    let owner = std::thread::spawn(move || owner).join().unwrap();
    let writer = cell
        .try_write_owned(owner)
        .unwrap_or_else(|_| panic!("returned original generation must remain installable"));
    writer.commit();
    assert_eq!(cell.read().value, 41);
    assert_eq!(original_reader.value, 7);
    assert_eq!(observations.clones.load(SeqCst), 2);
    assert!(!cell.is_poisoned());
    assert_eq!(observations.charge_drops[1].load(SeqCst), 0);
    drop(original_reader);
    drop(cell);
    collect_until(|| observations.charge_drops[1].load(SeqCst) == 1);
    assert!(DEALLOCATED.load(SeqCst));
    collect_until(|| observations.charge_drops[0].load(SeqCst) == 1);
}
