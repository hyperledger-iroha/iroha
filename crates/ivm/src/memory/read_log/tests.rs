//! Exact read-row admission, diagnostic atomicity and original-owner lifetime.

use super::*;
use std::sync::{Arc, Barrier};

fn append(log: &mut ReadLog, addr: u64) {
    let replacement = log.prepare_growth().unwrap();
    log.record_prepared(replacement, AccessRange { addr, len: 1 });
}

#[test]
fn exact_growth_keeps_old_rows_until_publication_and_admits_overlap() {
    let row = std::mem::size_of::<AccessRange>();
    let budget = AllocationBudget::new(4 * row - 1);
    let mut log = ReadLog::with_memory_budget(&budget);
    assert!(matches!(
        log.prepare_growth(),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(log.capacity(), 0);
    budget.set_limit_bytes(4 * row);
    for address in 0..4 {
        append(&mut log, address);
    }
    let pointer = log.as_ptr();
    budget.set_limit_bytes(12 * row - 1);
    assert!(matches!(
        log.prepare_growth(),
        Err(VMError::AllocationDeferred(_))
    ));
    assert_eq!(log.as_ptr(), pointer);
    assert_eq!(log.len(), 4);
    assert_eq!(budget.reserved_bytes(), 4 * row);
    budget.set_limit_bytes(12 * row);
    let replacement = log.prepare_growth().unwrap();
    assert_eq!(log.as_ptr(), pointer);
    assert_eq!(log.len(), 4);
    assert_eq!(budget.reserved_bytes(), 12 * row);
    log.record_prepared(replacement, AccessRange { addr: 9, len: 2 });
    assert_ne!(log.as_ptr(), pointer);
    assert_eq!(log.capacity(), 8);
    assert_eq!(log[4], AccessRange { addr: 9, len: 2 });
    assert_eq!(budget.reserved_bytes(), 8 * row);
    budget.set_limit_bytes(0);
    append(&mut log, 10); // Spare initialized capacity needs no new admission.
    assert_eq!(budget.reserved_bytes(), 8 * row);
    log.clear();
    assert!(log.is_empty());
    assert_eq!(log.capacity(), 8);
    assert_eq!(budget.reserved_bytes(), 8 * row);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn independent_snapshots_and_clones_keep_the_original_finite_pool() {
    let row = std::mem::size_of::<AccessRange>();
    let budget = AllocationBudget::new(4 * row);
    let mut log = ReadLog::with_memory_budget(&budget);
    append(&mut log, 7);
    assert!(matches!(
        log.try_snapshot(),
        Err(VMError::AllocationDeferred(_))
    ));
    budget.set_limit_bytes(5 * row);
    let snapshot = log.try_snapshot().unwrap();
    assert_eq!(snapshot.log.capacity(), 1);
    assert_ne!(snapshot.as_ptr(), log.as_ptr());
    assert!(matches!(
        snapshot.try_clone(),
        Err(VMError::AllocationDeferred(_))
    ));
    budget.set_limit_bytes(6 * row);
    let clone = snapshot.try_clone().unwrap();
    assert_eq!(clone, snapshot);
    assert_ne!(clone.as_ptr(), snapshot.as_ptr());
    log.clear();
    assert_eq!(snapshot[0], AccessRange { addr: 7, len: 1 });
    drop(log);
    assert_eq!(budget.reserved_bytes(), 2 * row);
    budget.set_limit_bytes(0);
    assert!(matches!(
        snapshot.try_clone(),
        Err(VMError::AllocationDeferred(_))
    ));
    drop(snapshot);
    assert_eq!(budget.reserved_bytes(), row);
    drop(clone);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn template_copy_partitions_exact_original_credit_after_budget_shrink() {
    let row = std::mem::size_of::<AccessRange>();
    let budget = AllocationBudget::new(7 * row);
    let mut log = ReadLog::with_memory_budget(&budget);
    for address in 0..3 {
        append(&mut log, address);
    }
    let plan = log.memory_plan().unwrap();
    assert_eq!(plan.requested_bytes(), 3 * row);
    let mut parent = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    budget.set_limit_bytes(0);
    let copy = log.try_copy(Some(&mut parent)).unwrap();
    assert_eq!(copy, log);
    assert_ne!(copy.as_ptr(), log.as_ptr());
    assert_eq!(copy.capacity(), 3);
    assert_eq!(parent.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), 7 * row);
    assert!(log.try_copy(Some(&mut parent)).is_err());
    assert_eq!(parent.remaining_bytes(), 0);
    drop(parent);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 3 * row);
    drop(copy);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn foreign_or_local_source_cannot_substitute_a_template_pool() {
    let row = std::mem::size_of::<AccessRange>();
    let budget = AllocationBudget::new(8 * row);
    let foreign = AllocationBudget::new(8 * row);
    let mut log = ReadLog::with_memory_budget(&budget);
    append(&mut log, 3);
    let plan = log.memory_plan().unwrap();
    let mut parent = ExecutionMemoryLease::reserve(&foreign, plan).unwrap();
    assert!(log.try_copy(Some(&mut parent)).is_err());
    assert_eq!(parent.remaining_bytes(), row);
    assert_eq!(budget.reserved_bytes(), 4 * row);
    let mut local = ReadLog::default();
    append(&mut local, 5);
    assert!(local.try_copy(Some(&mut parent)).is_err());
    assert_eq!(parent.remaining_bytes(), row);
    let local_copy = local.try_copy(None).unwrap();
    assert_eq!(local_copy, local);
    assert!(local_copy.active_budget.is_none());
    drop(parent);
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(log);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn local_growth_and_snapshot_refusal_preserve_retention_custody() {
    let row = std::mem::size_of::<AccessRange>();
    let budget = crate::cache_memory::TestMemoryBudget::new(8 * row);
    let mut log = ReadLog {
        rows: Rows::Local(budget.empty_rows()),
        active_budget: None,
    };
    for address in 0..4 {
        append(&mut log, address);
    }
    let pointer = log.as_ptr();
    crate::cache_memory::refuse_next_owned_vec_growth_for_test();
    assert!(log.prepare_growth().is_err());
    assert_eq!(log.as_ptr(), pointer);
    assert_eq!(log.capacity(), 4);
    assert_eq!(budget.stats().measured_resident_bytes(), 4 * row);
    crate::cache_memory::refuse_next_owned_vec_growth_for_test();
    assert!(log.try_snapshot().is_err());
    assert_eq!(budget.stats().measured_resident_bytes(), 4 * row);
    let snapshot = log.try_snapshot().unwrap();
    assert!(log.try_retain());
    assert!(snapshot.log.try_retain());
    budget.set_limit(0);
    assert!(!snapshot.log.try_retain());
    log.make_active();
    assert_eq!(budget.stats().measured_resident_bytes(), 8 * row);
    log.clear();
    assert_eq!(snapshot.len(), 4);
    drop(log);
    assert_eq!(budget.stats().measured_resident_bytes(), 4 * row);
    drop(snapshot);
    assert_eq!(budget.stats().measured_resident_bytes(), 0);
}

#[test]
fn abandoned_growth_snapshot_unwind_and_final_borrowers_release_exactly_once() {
    let row = std::mem::size_of::<AccessRange>();
    let budget = AllocationBudget::new(16 * row);
    let mut log = ReadLog::with_memory_budget(&budget);
    for address in 0..4 {
        append(&mut log, address);
    }
    let pointer = log.as_ptr();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _replacement = log.prepare_growth().unwrap();
        assert_eq!(budget.reserved_bytes(), 12 * row);
        panic!("abandon read replacement before diagnostic acceptance");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 4 * row);
    assert_eq!(log.as_ptr(), pointer);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _snapshot = log.try_snapshot().unwrap();
        panic!("abandon independent read snapshot");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 4 * row);
    let owner = Arc::new(log.try_snapshot().unwrap());
    drop(log);
    budget.set_limit_bytes(0);
    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let borrower = Arc::clone(&owner);
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(borrower.len(), 4);
                barrier.wait();
            });
        }
        drop(owner);
        assert_eq!(budget.reserved_bytes(), 4 * row);
        barrier.wait();
        assert_eq!(budget.reserved_bytes(), 4 * row);
        barrier.wait();
    });
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn diagnostic_refusal_preserves_rows_capacity_and_caller_output() {
    use crate::{Memory, execution_memory_recorder::DiagnosticMemoryAccessRecorder};
    for funded in [false, true] {
        let budget = AllocationBudget::new(64 * 1024 * 1024);
        let mut memory = if funded {
            Memory::new_with_stack_limit_funded(Memory::MIN_STACK_SIZE, &budget).unwrap()
        } else {
            Memory::new_with_stack_limit(Memory::MIN_STACK_SIZE).unwrap()
        };
        for _ in 0..4 {
            memory.load_u8(Memory::OUTPUT_START).unwrap();
        }
        let recorder = DiagnosticMemoryAccessRecorder::try_new(0, &budget).unwrap();
        recorder.begin_run(false).unwrap();
        memory
            .install_diagnostic_access_recorder(recorder.shared())
            .unwrap();
        let pointer = memory.read_log.lock().as_ptr();
        let occupied = budget.reserved_bytes();
        let mut output = [0xa5; 2];
        assert!(matches!(
            memory.load_bytes(Memory::OUTPUT_START, &mut output),
            Err(VMError::ExecutionDeferred(
                ExecutionDeferral::ActiveMemoryCapacity
            ))
        ));
        assert_eq!(output, [0xa5; 2]);
        assert!(recorder.is_empty());
        assert_eq!(memory.read_log.lock().as_ptr(), pointer);
        assert_eq!(memory.read_log.lock().len(), 4);
        assert_eq!(memory.read_log.lock().capacity(), 4);
        assert_eq!(budget.reserved_bytes(), occupied);
        memory.clear_diagnostic_access_recorder();
        memory
            .load_bytes(Memory::OUTPUT_START, &mut output)
            .unwrap();
        assert_eq!(output, [0; 2]);
        assert_eq!(memory.read_log.lock().len(), 5);
    }
}

#[test]
fn refunds_notify_after_memory_unlocks_on_publication_refusal_and_unwind() {
    use crate::{Memory, execution_memory_recorder::DiagnosticMemoryAccessRecorder};
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
                    .store(memory.read_log.try_lock().is_none(), Ordering::SeqCst);
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }
    for outcome in 0..3 {
        let budget = AllocationBudget::new(64 * 1024 * 1024);
        let mut memory =
            Memory::new_with_stack_limit_funded(Memory::MIN_STACK_SIZE, &budget).unwrap();
        for _ in 0..4 {
            memory.load_u8(Memory::OUTPUT_START).unwrap();
        }
        if outcome == 1 {
            let recorder = DiagnosticMemoryAccessRecorder::try_new(0, &budget).unwrap();
            recorder.begin_run(false).unwrap();
            memory.install_diagnostic_access_recorder(recorder).unwrap();
        }
        let memory = Arc::new(memory);
        let registration_layout =
            iroha_allocation::release::ReleaseRegistration::allocation_layout();
        let mut registration = iroha_allocation::release::ReleaseRegistration::from_reservation(
            &mut budget.try_reserve(registration_layout).unwrap(),
        )
        .unwrap();
        assert!(registration.belongs_to(&budget));
        let occupied = budget.reserved_bytes();
        let next = 8 * std::mem::size_of::<AccessRange>();
        budget.set_limit_bytes(occupied + next);
        let refusal = budget.try_reserve_bytes(next + 1).unwrap_err();
        let iroha_allocation::AllocationRefusal::Capacity { release, .. } = refusal else {
            panic!("actual occupied-pool refusal");
        };
        let probe = Arc::new(Probe {
            memory: Arc::downgrade(&memory),
            calls: AtomicUsize::new(0),
            held: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut future = release.wait_for_release(&mut registration);
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        match outcome {
            0 => {
                memory.load_u8(Memory::OUTPUT_START).unwrap();
            }
            1 => {
                assert!(memory.load_u8(Memory::OUTPUT_START).is_err());
            }
            _ => {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    memory.with_read_log(|log| {
                        let _replacement = log.prepare_growth().unwrap();
                        panic!("abandon replacement while holding read mutex");
                    });
                }));
                assert!(result.is_err());
            }
        }
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert!(!probe.held.load(Ordering::SeqCst));
        assert_eq!(
            memory.read_log.lock().len(),
            if outcome == 0 { 5 } else { 4 }
        );
        drop(memory);
        assert_eq!(budget.reserved_bytes(), registration_layout.size());
        drop(future);
        drop(registration);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn memory_template_partitions_its_nonempty_read_plan_into_the_actual_copied_owner() {
    use crate::Memory;
    let row = std::mem::size_of::<AccessRange>();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let mut source = Memory::new_with_stack_limit_funded(Memory::MIN_STACK_SIZE, &budget).unwrap();
    source.load_u8(Memory::OUTPUT_START).unwrap();
    let source_rows = source.read_log.lock().as_ptr();
    let occupied = budget.reserved_bytes();
    let plan = source.runtime_template_memory_plan().unwrap();
    let expected_rows = source.with_read_log(|log| log.memory_plan().unwrap().requested_bytes());
    assert_eq!(expected_rows, row);
    budget.set_limit_bytes(occupied + plan.requested_bytes());
    let mut parent = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    budget.set_limit_bytes(0);
    let mut copy = source
        .try_clone_for_runtime_template(Some(&mut parent))
        .unwrap();
    assert_eq!(
        parent.remaining_bytes(),
        0,
        "read credit is held by its actual copied backing"
    );
    assert_eq!(budget.reserved_bytes(), occupied + plan.requested_bytes());
    assert_eq!(copy.read_log.lock().capacity(), 1);
    assert_ne!(copy.read_log.lock().as_ptr(), source_rows);
    assert_eq!(&**copy.read_log.lock(), &**source.read_log.lock());
    assert_eq!(copy.current_root(), source.current_root());
    drop(parent);
    let owner = Arc::new(copy);
    let borrower = Arc::clone(&owner);
    drop(source);
    assert_eq!(budget.reserved_bytes(), plan.requested_bytes());
    drop(owner);
    assert_eq!(budget.reserved_bytes(), plan.requested_bytes());
    assert_eq!(
        borrower.read_log.lock()[0],
        AccessRange {
            addr: Memory::OUTPUT_START,
            len: 1
        }
    );
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
}
