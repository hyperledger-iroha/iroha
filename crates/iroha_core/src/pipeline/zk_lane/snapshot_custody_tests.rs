//! Actual original-pool refunds reenter retry queues only after physical unlock.

use super::*;
use iroha_allocation::{AllocationBudget, AllocationRefusal};
use ivm::zk::{DiagnosticRegisterSource, DiagnosticTraceSource};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Wake, Waker},
};

fn task(budget: &AllocationBudget, marker: u8) -> ZkTask {
    let state = [RegisterState {
        pc: u64::from(marker),
        gpr: [u64::from(marker); 256],
        tags: [true; 256],
    }];
    let snapshot = DiagnosticTraceSource {
        registers: DiagnosticRegisterSource::States(&state),
        constraints: &[],
        memory_events: &[],
        register_events: &[],
        steps: &[],
    }
    .try_snapshot(budget)
    .expect("fund retained queue fixture");
    ZkTask {
        tx_hash: Some(Hash::prehashed([marker; 32])),
        code_hash: [marker; 32],
        program: vec![marker].into(),
        header: None,
        snapshot,
        transport_capabilities: None,
        negotiated_capabilities: None,
    }
}

struct Reenter {
    ring: Arc<RetryRing>,
    replacement: std::sync::Mutex<Option<ZkTask>>,
    calls: AtomicUsize,
}
impl Wake for Reenter {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(
            self.ring.inner.try_lock().is_ok(),
            "refund callback must not hold RetryRing mutex"
        );
        if let Some(task) = self.replacement.lock().unwrap().take() {
            // A producer on another thread joins during the refund callback.
            // The old cleanup must stay bounded to its initial cohort.
            let ring = Arc::clone(&self.ring);
            std::thread::spawn(move || {
                assert!(
                    ring.inner.try_lock().is_ok(),
                    "concurrent producer observes released mutex"
                );
                assert!(ring.enqueue(task).is_ok());
            })
            .join()
            .unwrap();
        }
    }
}
fn observe(budget: &AllocationBudget, probe: &Arc<Reenter>) -> impl Future<Output = ()> + Unpin {
    budget.set_limit_bytes(budget.reserved_bytes());
    let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
    else {
        panic!("occupied pool must return original capacity observation");
    };
    let mut wait = release.wait_for_release();
    let waker = Waker::from(Arc::clone(probe));
    assert_eq!(
        Pin::new(&mut wait).poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    );
    wait
}
fn probe(ring: &Arc<RetryRing>) -> (Arc<Reenter>, AllocationBudget) {
    let replacement_budget = AllocationBudget::new(64 * 1024);
    (
        Arc::new(Reenter {
            ring: Arc::clone(ring),
            replacement: std::sync::Mutex::new(Some(task(&replacement_budget, 9))),
            calls: AtomicUsize::new(0),
        }),
        replacement_budget,
    )
}

#[test]
fn clear_refunds_after_unlock_and_retains_concurrent_new_owner() {
    let budget = AllocationBudget::new(64 * 1024);
    let ring = Arc::new(RetryRing::new(8, 2));
    assert!(ring.enqueue(task(&budget, 1)).is_ok());
    assert!(ring.enqueue(task(&budget, 2)).is_ok());
    let (probe, replacement_budget) = probe(&ring);
    let _wait = observe(&budget, &probe);
    assert_eq!(ring.clear(), 2);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(ring.depth(), 1);
    let retained = replacement_budget.reserved_bytes();
    assert!(retained > 0);
    let mut pending = Vec::new();
    let stats = ring.drain_into_pending(&mut pending, 8);
    assert_eq!(stats.replayed, 1);
    assert_eq!(stats.exhausted, 0);
    assert_eq!(pending[0].snapshot.states()[0].gpr[0], 9);
    assert_eq!(replacement_budget.reserved_bytes(), retained);
    drop(pending);
    assert_eq!(replacement_budget.reserved_bytes(), 0);
}

#[test]
fn exhausted_retry_cohort_drops_after_unlock_without_aging_new_enqueue() {
    let budget = AllocationBudget::new(64 * 1024);
    let ring = Arc::new(RetryRing::new(8, 1));
    assert!(ring.enqueue(task(&budget, 1)).is_ok());
    assert!(ring.enqueue(task(&budget, 2)).is_ok());
    let (probe, replacement_budget) = probe(&ring);
    let _wait = observe(&budget, &probe);
    let stats = ring.drain_into_pending(&mut Vec::new(), 0);
    assert_eq!(stats.exhausted, 2);
    assert_eq!(stats.replayed, 0);
    assert_eq!(stats.depth, 1);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(budget.reserved_bytes(), 0);
    {
        let queue = ring.inner.lock().unwrap();
        assert_eq!(queue[0].attempts, 0);
        assert_eq!(queue[0].task.snapshot.states()[0].gpr[0], 9);
    }
    assert_eq!(ring.clear(), 1);
    assert_eq!(replacement_budget.reserved_bytes(), 0);
}

#[test]
fn replay_moves_original_owners_in_order_and_refunds_only_after_pending_drop() {
    let budget = AllocationBudget::new(64 * 1024);
    let ring = Arc::new(RetryRing::new(8, 2));
    for marker in 1..=3 {
        assert!(ring.enqueue(task(&budget, marker)).is_ok());
    }
    let reserved = budget.reserved_bytes();
    let (probe, replacement_budget) = probe(&ring);
    let _wait = observe(&budget, &probe);
    let mut pending = Vec::new();
    let stats = ring.drain_into_pending(&mut pending, 2);
    assert_eq!(stats.replayed, 2);
    assert_eq!(stats.depth, 1);
    assert_eq!(budget.reserved_bytes(), reserved);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert_eq!(pending[0].snapshot.states()[0].pc, 1);
    assert_eq!(pending[1].snapshot.states()[0].pc, 2);
    drop(pending);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    let mut pending = Vec::new();
    assert_eq!(ring.drain_into_pending(&mut pending, 8).replayed, 2);
    assert_eq!(pending[0].snapshot.states()[0].pc, 3);
    assert_eq!(pending[1].snapshot.states()[0].pc, 9);
    drop(pending);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(replacement_budget.reserved_bytes(), 0);
}

#[test]
fn full_retry_ring_returns_owner_before_any_original_refund() {
    let budget = AllocationBudget::new(64 * 1024);
    let ring = Arc::new(RetryRing::new(1, 2));
    assert!(ring.enqueue(task(&budget, 1)).is_ok());
    let rejected = task(&budget, 2);
    let reserved = budget.reserved_bytes();
    let (probe, replacement_budget) = probe(&ring);
    // This control probes unlock only; a full queue deliberately has no replacement slot.
    drop(probe.replacement.lock().unwrap().take());
    assert_eq!(replacement_budget.reserved_bytes(), 0);
    let _wait = observe(&budget, &probe);
    let returned = match ring.enqueue(rejected) {
        Err(task) => task,
        Ok(_) => panic!("full ring must return owner"),
    };
    assert_eq!(budget.reserved_bytes(), reserved);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert_eq!(returned.snapshot.states()[0].pc, 2);
    drop(returned);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(ring.clear(), 1);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn optional_capture_refusal_preserves_vm_effects_gas_and_source_trace() {
    let mut vm = ivm::IVM::new(1_000_000);
    let code = [
        ivm::encoding::wide::encode_ri(ivm::instruction::wide::arithmetic::ADDI, 1, 0, 7),
        ivm::encoding::wide::encode_halt(),
    ];
    let bytes: Vec<u8> = code.into_iter().flat_map(u32::to_le_bytes).collect();
    vm.load_code(&bytes).unwrap();
    vm.set_zk_mode(true).unwrap();
    vm.set_zk_trace_enabled(true);
    vm.set_max_cycles(8);
    vm.run().unwrap();
    let before = vm.execution_summary();
    let gas = vm.remaining_gas();
    let budget = AllocationBudget::new(0);
    assert!(!capture_and_submit(
        &vm,
        &budget,
        None,
        bytes.into(),
        None,
        None,
        None
    ));
    assert_eq!(vm.remaining_gas(), gas);
    assert_eq!(vm.execution_summary(), before);
    assert_eq!(vm.register(1), 7);
    let funded = AllocationBudget::new(1024 * 1024);
    let snapshot = vm.try_diagnostic_snapshot(&funded).unwrap();
    assert!(!snapshot.states().is_empty());
    assert!(ivm::zk::check_diagnostic_trace(&snapshot).is_ok());
    drop(snapshot);
    assert_eq!(funded.reserved_bytes(), 0);
}
