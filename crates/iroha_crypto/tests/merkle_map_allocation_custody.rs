//! Physical allocator refusal, rollback and final-release controls for resident maps.

use iroha_crypto::{Hash, MerkleMap, MerkleMapError};
use mv::allocation::{AllocationBudget, AllocationRefusal, PrepaidSharedError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::pin,
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

struct Observer;
const SLOTS: usize = 8;
static SERIAL: Mutex<()> = Mutex::new(());
static COUNT: AtomicUsize = AtomicUsize::new(0);
static FAIL: AtomicUsize = AtomicUsize::new(usize::MAX);
static POINTERS: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static SIZES: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static RESERVED: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static FREED: [AtomicBool; SLOTS] = [const { AtomicBool::new(false) }; SLOTS];
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static BUDGET: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
}

// Test-only exact System forwarding; no production unsafe or alternate owner.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let index = ENABLED
            .try_with(Cell::get)
            .unwrap_or(false)
            .then(|| COUNT.fetch_add(1, SeqCst));
        if let Some(index) = index.filter(|index| *index < SLOTS) {
            SIZES[index].store(layout.size(), SeqCst);
            let reserved = BUDGET
                .try_with(|budget| {
                    budget
                        .borrow()
                        .as_ref()
                        .map_or(0, AllocationBudget::reserved_bytes)
                })
                .unwrap_or(0);
            RESERVED[index].store(reserved, SeqCst);
            if FAIL.load(SeqCst) == index {
                return std::ptr::null_mut();
            }
        }
        // SAFETY: forwards the caller's exact nonzero allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        if let Some(index) = index.filter(|index| *index < SLOTS) {
            POINTERS[index].store(pointer as usize, SeqCst);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let index = POINTERS.iter().position(|slot| {
            slot.compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
                .is_ok()
        });
        // SAFETY: returns the original pointer and exact layout to System.
        unsafe { System.dealloc(pointer, layout) };
        if let Some(index) = index {
            FREED[index].store(true, SeqCst);
        }
    }
}
#[global_allocator]
static ALLOCATOR: Observer = Observer;

fn arm(budget: &AllocationBudget, fail: usize) {
    ENABLED.with(|enabled| enabled.set(false));
    for index in 0..SLOTS {
        assert_eq!(POINTERS[index].load(SeqCst), 0);
        SIZES[index].store(0, SeqCst);
        RESERVED[index].store(0, SeqCst);
        FREED[index].store(false, SeqCst);
    }
    BUDGET.with(|stored| *stored.borrow_mut() = Some(budget.clone()));
    FAIL.store(fail, SeqCst);
    COUNT.store(0, SeqCst);
    ENABLED.with(|enabled| enabled.set(true));
}
fn disarm() {
    ENABLED.with(|enabled| enabled.set(false));
}
fn key(byte: u8) -> Hash {
    let mut bytes = [0; Hash::LENGTH];
    bytes[0] = byte;
    Hash::prehashed(bytes)
}

#[test]
fn each_physical_path_failure_preserves_old_root_count_and_original_node_owners() {
    let _serial = SERIAL.lock().unwrap();
    let value = Hash::new(b"value");
    let changed = Hash::new(b"changed");
    for fail in 0..3 {
        let budget = AllocationBudget::new(1024 * 1024);
        let mut map = MerkleMap::new(&budget);
        map.replace(key(0), None, Some(value)).unwrap();
        let unit = budget.reserved_bytes();
        map.replace(key(128), None, Some(value)).unwrap();
        map.replace(key(64), None, Some(value)).unwrap();
        let before = map.clone();
        let root = before.root();
        assert_eq!(budget.reserved_bytes(), unit * 5);
        arm(&budget, fail);
        let result = map.replace(key(0), Some(value), Some(changed));
        disarm();
        assert!(
            matches!(result, Err(MerkleMapError::Allocation(PrepaidSharedError::Allocator { requested_bytes })) if requested_bytes == unit)
        );
        assert_eq!(COUNT.load(SeqCst), fail + 1);
        for index in 0..=fail {
            assert_eq!(SIZES[index].load(SeqCst), unit);
            assert_eq!(RESERVED[index].load(SeqCst), unit * 8);
            assert_eq!(FREED[index].load(SeqCst), index < fail);
        }
        assert_eq!(map.root(), root);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&key(0)), Some(value));
        assert_eq!(budget.reserved_bytes(), unit * 5);
        arm(&budget, usize::MAX);
        map.replace(key(0), Some(value), Some(changed)).unwrap();
        disarm();
        assert_eq!(COUNT.load(SeqCst), 3);
        assert_eq!(budget.reserved_bytes(), unit * 8);
        assert_eq!(before.root(), root);
        assert_ne!(map.root(), root);
        drop(map);
        for freed in FREED.iter().take(3) {
            assert!(freed.load(SeqCst));
        }
        assert_eq!(budget.reserved_bytes(), unit * 5);
        drop(before);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

struct AfterFree {
    budget: AllocationBudget,
    bytes: usize,
    wakes: AtomicUsize,
}
impl Wake for AfterFree {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        assert!(
            FREED[0].load(SeqCst),
            "refund preceded the final System deallocation"
        );
        assert_eq!(self.budget.reserved_bytes(), 0);
        drop(self.budget.try_reserve_bytes(self.bytes).unwrap());
        self.wakes.fetch_add(1, SeqCst);
    }
}

#[test]
fn concurrent_last_snapshot_refunds_only_after_the_actual_node_is_gone() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(1024 * 1024);
    let mut map = MerkleMap::new(&budget);
    let value = Hash::new(b"warm hash");
    arm(&budget, usize::MAX);
    map.replace(key(0), None, Some(value)).unwrap();
    disarm();
    let bytes = budget.reserved_bytes();
    assert_eq!(COUNT.load(SeqCst), 1);
    assert_eq!(SIZES[0].load(SeqCst), bytes);
    assert_eq!(RESERVED[0].load(SeqCst), bytes);
    budget.set_limit_bytes(bytes);
    let Err(AllocationRefusal::Capacity { release, .. }) = budget.try_reserve_bytes(1) else {
        panic!("node retains original credit");
    };
    let observer = Arc::new(AfterFree {
        budget: budget.clone(),
        bytes,
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&observer));
    let mut wait = pin!(release.wait_for_release());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let barrier = Barrier::new(4);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let snapshot = map.clone();
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                drop(snapshot);
            });
        }
        drop(map);
    });
    assert_eq!(observer.wakes.load(SeqCst), 1);
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn map_unwind_frees_the_real_node_and_refunds_its_original_pool() {
    let _serial = SERIAL.lock().unwrap();
    let budget = AllocationBudget::new(1024 * 1024);
    let mut map = MerkleMap::new(&budget);
    let value = Hash::new(b"warm hash");
    arm(&budget, usize::MAX);
    map.replace(key(0), None, Some(value)).unwrap();
    disarm();
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _map = map;
        panic!("exercise resident map unwind");
    }));
    assert!(result.is_err());
    assert!(FREED[0].load(SeqCst));
    assert_eq!(budget.reserved_bytes(), 0);
}
