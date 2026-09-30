//! Actual read-row allocator refusal and final backing/credit release ordering.

use super::{Layout, SERIAL, SeqCst, memory_stats};
use iroha_allocation::AllocationBudget;
use std::{
    cell::Cell,
    sync::{Arc, OnceLock, atomic::AtomicUsize},
};

static REQUESTS: AtomicUsize = AtomicUsize::new(0);
static FIRST_POINTER: AtomicUsize = AtomicUsize::new(0);
static WATCHED_POINTER: AtomicUsize = AtomicUsize::new(0);
static CREDIT_AT_FREE: AtomicUsize = AtomicUsize::new(0);
static RETENTION_AT_FREE: AtomicUsize = AtomicUsize::new(0);
static BUDGET: OnceLock<AllocationBudget> = OnceLock::new();
thread_local! {
    static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static FAIL_SIZE: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(super) fn refuse_allocation(layout: Layout) -> bool {
    if !OBSERVE.try_with(Cell::get).unwrap_or(false) {
        return false;
    }
    REQUESTS.fetch_add(1, SeqCst);
    FAIL_SIZE
        .try_with(|size| {
            if size.get() == Some(layout.size()) {
                size.set(None);
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}

pub(super) fn allocated(pointer: *mut u8) {
    if !pointer.is_null() && OBSERVE.try_with(Cell::get).unwrap_or(false) {
        let _ = FIRST_POINTER.compare_exchange(0, pointer as usize, SeqCst, SeqCst);
    }
}

pub(super) fn deallocated(pointer: *mut u8) {
    if WATCHED_POINTER
        .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
        .is_ok()
    {
        CREDIT_AT_FREE.store(
            BUDGET
                .get()
                .expect("read observer initialized")
                .reserved_bytes(),
            SeqCst,
        );
        RETENTION_AT_FREE.store(memory_stats().measured_resident_bytes(), SeqCst);
    }
}

struct Observation;
impl Drop for Observation {
    fn drop(&mut self) {
        OBSERVE.set(false);
        FAIL_SIZE.set(None);
        WATCHED_POINTER.store(0, SeqCst);
    }
}
fn start() {
    REQUESTS.store(0, SeqCst);
    FIRST_POINTER.store(0, SeqCst);
    OBSERVE.set(true);
}
fn stop() -> usize {
    OBSERVE.set(false);
    REQUESTS.load(SeqCst)
}

#[test]
fn read_rows_reserve_before_allocating_and_free_before_original_and_retention_refunds() {
    let _serial = SERIAL.lock().unwrap();
    let _observation = Observation;
    let row = std::mem::size_of::<ivm::AccessRange>();
    let budget = AllocationBudget::new(256 * 1024 * 1024);
    BUDGET
        .set(budget.clone())
        .expect("one read allocator observer");
    let vm = ivm::IVM::try_new_with_memory_budget(257, &budget).unwrap();
    let base = budget.reserved_bytes();
    let gas = vm.remaining_gas();
    let mut output = [0xa5; 2];
    budget.set_limit_bytes(base + 4 * row - 1);
    start();
    let refused = vm.memory.load_bytes(ivm::Memory::OUTPUT_START, &mut output);
    assert_eq!(stop(), 0, "admission precedes the first backing request");
    assert!(matches!(refused, Err(ivm::VMError::AllocationDeferred(_))));
    assert_eq!(output, [0xa5; 2]);
    assert_eq!(budget.reserved_bytes(), base);
    assert_eq!(vm.remaining_gas(), gas);

    budget.set_limit_bytes(base + 4 * row);
    FAIL_SIZE.set(Some(4 * row));
    start();
    let refused = vm.memory.load_bytes(ivm::Memory::OUTPUT_START, &mut output);
    assert_eq!(stop(), 1, "real null allocation of the initial row array");
    assert!(matches!(refused, Err(ivm::VMError::ExecutionDeferred(_))));
    assert_eq!(output, [0xa5; 2]);
    assert_eq!(budget.reserved_bytes(), base);
    start();
    for _ in 0..4 {
        vm.memory
            .load_bytes(ivm::Memory::OUTPUT_START, &mut output)
            .unwrap();
    }
    assert_eq!(stop(), 1, "three appends reuse the original fixed capacity");
    assert_eq!(output, [0; 2]);
    let original = FIRST_POINTER.load(SeqCst);
    assert_ne!(original, 0);
    WATCHED_POINTER.store(original, SeqCst);
    let occupied = budget.reserved_bytes();
    assert_eq!(occupied, base + 4 * row);

    budget.set_limit_bytes(occupied + 8 * row - 1);
    output.fill(0x5a);
    start();
    let refused = vm.memory.load_bytes(ivm::Memory::OUTPUT_START, &mut output);
    assert_eq!(stop(), 0, "nonempty overlap admitted before allocation");
    assert!(matches!(refused, Err(ivm::VMError::AllocationDeferred(_))));
    assert_eq!(WATCHED_POINTER.load(SeqCst), original);
    assert_eq!(output, [0x5a; 2]);
    budget.set_limit_bytes(occupied + 8 * row);
    FAIL_SIZE.set(Some(8 * row));
    start();
    let refused = vm.memory.load_bytes(ivm::Memory::OUTPUT_START, &mut output);
    assert_eq!(stop(), 1);
    assert!(matches!(refused, Err(ivm::VMError::ExecutionDeferred(_))));
    assert_eq!(WATCHED_POINTER.load(SeqCst), original);
    assert_eq!(budget.reserved_bytes(), occupied);
    assert_eq!(output, [0x5a; 2]);
    start();
    vm.memory
        .load_bytes(ivm::Memory::OUTPUT_START, &mut output)
        .unwrap();
    assert_eq!(stop(), 1);
    assert_eq!(WATCHED_POINTER.load(SeqCst), 0);
    assert_eq!(CREDIT_AT_FREE.load(SeqCst), occupied + 8 * row);
    assert!(RETENTION_AT_FREE.load(SeqCst) >= 12 * row);
    assert_eq!(budget.reserved_bytes(), base + 8 * row);
    assert_eq!(output, [0; 2]);
    assert_eq!(vm.remaining_gas(), gas);

    let occupied = budget.reserved_bytes();
    let snapshot_bytes = 5 * row;
    budget.set_limit_bytes(occupied + snapshot_bytes - 1);
    start();
    let refused = vm.memory.try_read_log_snapshot();
    assert_eq!(stop(), 0);
    assert!(matches!(refused, Err(ivm::VMError::AllocationDeferred(_))));
    budget.set_limit_bytes(occupied + snapshot_bytes);
    FAIL_SIZE.set(Some(snapshot_bytes));
    start();
    let refused = vm.memory.try_read_log_snapshot();
    assert_eq!(stop(), 1);
    assert!(matches!(refused, Err(ivm::VMError::ExecutionDeferred(_))));
    assert_eq!(budget.reserved_bytes(), occupied);
    start();
    let snapshot = vm.memory.try_read_log_snapshot().unwrap();
    assert_eq!(stop(), 1);
    assert_eq!(snapshot.len(), 5);
    assert_eq!(FIRST_POINTER.load(SeqCst), snapshot.as_ptr() as usize);
    assert!(snapshot.iter().all(|range| *range
        == ivm::AccessRange {
            addr: ivm::Memory::OUTPUT_START,
            len: 2
        }));
    start();
    let refused = snapshot.try_clone();
    assert_eq!(stop(), 0);
    assert!(matches!(refused, Err(ivm::VMError::AllocationDeferred(_))));
    budget.set_limit_bytes(occupied + 2 * snapshot_bytes);
    start();
    let copy = snapshot.try_clone().unwrap();
    assert_eq!(stop(), 1);
    assert_ne!(copy.as_ptr(), snapshot.as_ptr());
    assert_eq!(copy, snapshot);
    drop(copy);
    vm.memory.clear_tracking();
    assert_eq!(budget.reserved_bytes(), occupied + snapshot_bytes);
    assert_eq!(snapshot.len(), 5);

    let pointer = snapshot.as_ptr() as usize;
    WATCHED_POINTER.store(pointer, SeqCst);
    let owner = Arc::new(snapshot);
    let borrower = Arc::clone(&owner);
    drop(vm);
    budget.set_limit_bytes(0);
    assert_eq!(budget.reserved_bytes(), snapshot_bytes);
    drop(owner);
    assert_eq!(WATCHED_POINTER.load(SeqCst), pointer);
    assert_eq!(budget.reserved_bytes(), snapshot_bytes);
    drop(borrower);
    assert_eq!(WATCHED_POINTER.load(SeqCst), 0);
    assert_eq!(CREDIT_AT_FREE.load(SeqCst), snapshot_bytes);
    assert!(RETENTION_AT_FREE.load(SeqCst) >= snapshot_bytes);
    assert_eq!(budget.reserved_bytes(), 0);
}
