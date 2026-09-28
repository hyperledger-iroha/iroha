//! Exact initial backing, physical reclamation and canonical value movement.

use super::*;
use std::time::{Duration, Instant};

fn bytes<V: Value>() -> usize {
    CellInitialization::<V>::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum()
}
fn collect_until(budget: &AllocationBudget, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while budget.reserved_bytes() != expected {
        assert!(
            Instant::now() < deadline,
            "original EBR custody {} != {expected}",
            budget.reserved_bytes()
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}

#[test]
fn initial_admission_is_atomic_and_unused_backing_refunds_without_epoch_collection() {
    let demand = bytes::<u64>();
    let original = AllocationBudget::new(demand - 1);
    assert!(matches!(
        CellInitialization::<u64>::try_reserve(&original),
        Err(CellInitializationError::Admission(_))
    ));
    assert_eq!(original.reserved_bytes(), 0);
    original.set_limit_bytes(demand);
    let unrelated_reader = crossbeam_epoch::pin();
    let initial = CellInitialization::<u64>::try_reserve(&original).unwrap();
    assert_eq!(original.reserved_bytes(), demand);
    assert!(matches!(
        original.try_reserve_bytes(1),
        Err(AllocationRefusal::Capacity { .. })
    ));
    original.set_limit_bytes(0);
    drop(initial);
    assert_eq!(
        original.reserved_bytes(),
        0,
        "unused physical shells free before refund, without a collector grace period"
    );
    drop(unrelated_reader);
}

#[test]
fn initial_values_move_once_and_observers_retain_original_control_charges() {
    // Nested Vec backing is original caller-owned payload, outside outer demand.
    let current = vec![4_u8; 23];
    let undo = vec![7_u8; 17];
    let current_pointer = current.as_ptr();
    let undo_pointer = undo.as_ptr();
    let source = AllocationBudget::new(bytes::<Vec<u8>>());
    let foreign = AllocationBudget::new(bytes::<Vec<u8>>());
    let initial = CellInitialization::try_reserve(&source).unwrap();
    source.set_limit_bytes(0);
    let cell = initial.initialize(current, Some(undo));
    assert_eq!(cell.view().get().as_ptr(), current_pointer);
    assert_eq!(
        cell.predecessor_view().get().as_ref().unwrap().as_ptr(),
        undo_pointer
    );
    assert_eq!(source.reserved_bytes(), bytes::<Vec<u8>>());
    assert_eq!(foreign.reserved_bytes(), 0);
    let current = cell.blocks.read();
    let undo = cell.revert.read();
    let release = cell.blocks_released.observe();
    let identity = cell.publication.capture();
    drop(cell);
    let generation_bytes = Cell::<Vec<u8>, AllocationCharge>::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum::<usize>();
    assert!(source.reserved_bytes() >= generation_bytes);
    assert_eq!(current.as_ptr(), current_pointer);
    assert_eq!(undo.as_ref().unwrap().as_ptr(), undo_pointer);
    drop((current, undo, identity));
    collect_until(
        &source,
        ReleaseNotification::allocation_layout::<AllocationCharge>().size(),
    );
    drop(release);
    assert_eq!(source.reserved_bytes(), 0);
}
