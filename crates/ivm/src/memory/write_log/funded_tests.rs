//! Original finite-pool admission, independent copies and final-owner release.

use super::*;
use std::sync::{Arc, Barrier};

fn append(log: &mut WriteLog, address: u64, bytes: &[u8]) {
    let entry = log.prepare(address, bytes).unwrap();
    log.record_prepared(entry);
}

fn resident(log: &WriteLog) -> usize {
    log.rows.capacity() * std::mem::size_of::<WriteLogEntry>()
        + log
            .rows
            .iter()
            .map(|entry| entry.bytes().len())
            .sum::<usize>()
}

#[test]
fn exact_append_admission_refuses_before_rows_or_payload_and_preserves_nonempty_log() {
    let row = std::mem::size_of::<WriteLogEntry>();
    let budget = AllocationBudget::new(4 * row + 2 - 1);
    let mut log = WriteLog::with_memory_budget(&budget);
    assert!(matches!(
        log.prepare(3, &[1, 2]),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(log.rows.capacity(), 0);
    budget.set_limit_bytes(4 * row + 2);
    let prepared = log.prepare(3, &[1, 2]).unwrap();
    assert!(log.is_empty());
    assert_eq!(budget.reserved_bytes(), 4 * row + 2);
    log.record_prepared(prepared);
    let rows = log.rows.as_ptr();
    let payload = log.rows[0].bytes().as_ptr();
    budget.set_limit_bytes(0);
    assert!(matches!(
        log.prepare(4, &[9]),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(log.rows.as_ptr(), rows);
    assert_eq!(log.rows[0].bytes().as_ptr(), payload);
    assert_eq!(log.rows[0].bytes(), &[1, 2]);
    assert_eq!(budget.reserved_bytes(), 4 * row + 2);
    log.clear();
    assert_eq!(budget.reserved_bytes(), 4 * row);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn growth_precharges_old_new_overlap_and_keeps_existing_payload_pointers() {
    let row = std::mem::size_of::<WriteLogEntry>();
    let budget = AllocationBudget::new(4 * row + 4);
    let mut log = WriteLog::with_memory_budget(&budget);
    for index in 0..4 {
        append(&mut log, index, &[index as u8]);
    }
    let payload = log.rows[0].bytes().as_ptr();
    let previous = log.rows.as_ptr();
    let old = resident(&log);
    budget.set_limit_bytes(old + 8 * row); // One payload byte short, including overlap.
    assert!(matches!(
        log.prepare(9, &[9]),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(log.rows.as_ptr(), previous);
    assert_eq!(log.rows.len(), 4);
    assert_eq!(budget.reserved_bytes(), old);
    budget.set_limit_bytes(old + 8 * row + 1);
    append(&mut log, 9, &[9]);
    assert_ne!(log.rows.as_ptr(), previous);
    assert_eq!(log.rows.capacity(), 8);
    assert_eq!(log.rows[0].bytes().as_ptr(), payload);
    assert_eq!(log.rows[4].bytes(), &[9]);
    assert_eq!(budget.reserved_bytes(), 8 * row + 5);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn snapshots_and_clone_reserve_exact_independent_storage_from_original_pool() {
    let budget = AllocationBudget::new(64 * 1024);
    let mut log = WriteLog::with_memory_budget(&budget);
    append(&mut log, 7, &[4, 5, 6]);
    let original = resident(&log);
    let copy_bytes = log.memory_plan().unwrap().requested_bytes();
    budget.set_limit_bytes(original + copy_bytes - 1);
    assert!(matches!(
        log.try_snapshot(),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(budget.reserved_bytes(), original);
    budget.set_limit_bytes(original + copy_bytes);
    let snapshot = log.try_snapshot().unwrap();
    assert_eq!(snapshot.log.rows.capacity(), 1);
    assert_eq!(budget.reserved_bytes(), original + copy_bytes);
    assert_ne!(snapshot[0].bytes().as_ptr(), log.rows[0].bytes().as_ptr());
    assert!(matches!(
        snapshot.try_clone(),
        Err(VMError::AllocationDeferred(_))
    ));
    budget.set_limit_bytes(original + 2 * copy_bytes);
    let clone = snapshot.try_clone().unwrap();
    assert_eq!(clone, snapshot);
    assert_ne!(clone[0].bytes().as_ptr(), snapshot[0].bytes().as_ptr());
    log.clear();
    assert_eq!(snapshot[0].bytes(), &[4, 5, 6]);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 2 * copy_bytes);
    drop(snapshot);
    assert_eq!(budget.reserved_bytes(), copy_bytes);
    drop(clone);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn template_copy_partitions_original_prepaid_plan_without_second_admission() {
    let budget = AllocationBudget::new(64 * 1024);
    let mut log = WriteLog::with_memory_budget(&budget);
    append(&mut log, 9, &[1, 2, 3, 4]);
    let original = resident(&log);
    let plan = log.memory_plan().unwrap();
    budget.set_limit_bytes(original + plan.requested_bytes());
    let mut parent = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    budget.set_limit_bytes(0);
    let copy = log.try_copy(Some(&mut parent)).unwrap();
    assert_eq!(parent.remaining_bytes(), 0);
    assert_eq!(copy, log);
    assert_ne!(copy.rows[0].bytes().as_ptr(), log.rows[0].bytes().as_ptr());
    assert_eq!(budget.reserved_bytes(), original + plan.requested_bytes());
    assert!(log.try_copy(Some(&mut parent)).is_err());
    assert_eq!(parent.remaining_bytes(), 0);
    drop(parent);
    drop(log);
    assert_eq!(budget.reserved_bytes(), plan.requested_bytes());
    drop(copy);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn foreign_equal_limit_parent_cannot_replace_original_snapshot_pool() {
    let budget = AllocationBudget::new(64 * 1024);
    let mut log = WriteLog::with_memory_budget(&budget);
    append(&mut log, 2, &[5; 7]);
    let plan = log.memory_plan().unwrap();
    let foreign = AllocationBudget::new(64 * 1024);
    let mut lease = ExecutionMemoryLease::reserve(&foreign, plan).unwrap();
    assert!(!lease.belongs_to(&budget));
    let original = budget.reserved_bytes();
    assert!(log.try_copy(Some(&mut lease)).is_err());
    assert_eq!(lease.remaining_bytes(), plan.requested_bytes());
    assert_eq!(budget.reserved_bytes(), original);
    assert_eq!(log.rows[0].bytes(), &[5; 7]);
    let mut local = WriteLog::default();
    append(&mut local, 3, &[6; 7]);
    let local_payload = local.rows[0].bytes().as_ptr();
    assert!(local.try_copy(Some(&mut lease)).is_err());
    assert_eq!(lease.remaining_bytes(), plan.requested_bytes());
    assert_eq!(local.rows[0].bytes().as_ptr(), local_payload);
    assert_eq!(local.rows[0].bytes(), &[6; 7]);
    let local_copy = local.try_copy(None).unwrap();
    assert_eq!(local_copy, local);
    assert!(local_copy.active_budget.is_none());
    drop(local_copy);
    drop(local);
    drop(lease);
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn snapshot_unwind_and_concurrent_final_borrowers_keep_original_credit() {
    let budget = AllocationBudget::new(64 * 1024);
    let mut log = WriteLog::with_memory_budget(&budget);
    append(&mut log, 1, &[0xa5; 19]);
    let original = resident(&log);
    let prepared_unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _prepared = log.prepare(2, &[0x5a; 23]).unwrap();
        assert_eq!(budget.reserved_bytes(), original + 23);
        panic!("abandon prepared write before guest mutation");
    }));
    assert!(prepared_unwind.is_err());
    assert_eq!(budget.reserved_bytes(), original);
    assert_eq!(log.rows.len(), 1);
    assert_eq!(log.rows[0].bytes(), &[0xa5; 19]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _snapshot = log.try_snapshot().unwrap();
        assert!(budget.reserved_bytes() > original);
        panic!("abandon independent funded snapshot");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), original);
    let owner = Arc::new(log.try_snapshot().unwrap());
    let retained = resident(&owner.log);
    drop(log);
    assert!(owner.log.try_retain());
    budget.set_limit_bytes(0);
    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let borrower = Arc::clone(&owner);
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(borrower[0].bytes(), &[0xa5; 19]);
                barrier.wait();
            });
        }
        drop(owner);
        assert_eq!(budget.reserved_bytes(), retained);
        barrier.wait();
        assert_eq!(budget.reserved_bytes(), retained);
        barrier.wait();
    });
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn row_growth_refund_notifies_only_after_memory_releases_its_log_lock() {
    use crate::Memory;
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            Weak,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Wake, Waker},
    };

    struct Probe {
        memory: Weak<Memory>,
        calls: AtomicUsize,
        held: AtomicBool,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            if let Some(memory) = self.memory.upgrade() {
                self.held
                    .store(memory.write_log.try_lock().is_none(), Ordering::SeqCst);
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let memory =
        Arc::new(Memory::new_with_stack_limit_funded(Memory::MIN_STACK_SIZE, &budget).unwrap());
    for index in 0..4 {
        memory.with_write_log(|log| append(log, index, &[index as u8]));
    }
    let occupied = budget.reserved_bytes();
    let next = 8 * std::mem::size_of::<WriteLogEntry>() + 1;
    budget.set_limit_bytes(occupied + next);
    let refusal = budget.try_reserve_bytes(next + 1).unwrap_err();
    let mv::allocation::AllocationRefusal::Capacity { release, .. } = refusal else {
        panic!("actual occupied-pool capacity refusal");
    };
    let probe = Arc::new(Probe {
        memory: Arc::downgrade(&memory),
        calls: AtomicUsize::new(0),
        held: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&probe));
    let mut future = release.wait_for_release();
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    memory.with_write_log(|log| append(log, 7, &[7]));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert!(!probe.held.load(Ordering::SeqCst));
    assert_eq!(memory.write_log.lock().rows.len(), 5);
    drop(memory);
    assert_eq!(budget.reserved_bytes(), 0);
}
