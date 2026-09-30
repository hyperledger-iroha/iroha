//! Original-credit, geometry and allocation-free operation controls.

use super::*;
use iroha_allocation::AllocationBudget;
use std::sync::Arc;

#[test]
fn duplicate_indices_iterate_once_in_order_and_clear_keeps_backing() {
    for chunks in [0, 1, 63, 64, 65, 127, 128, 129] {
        let mut set = DirtyChunks::new(chunks, None).unwrap();
        let original = set.words().as_ptr();
        set.extend((0..chunks).rev());
        set.extend(0..chunks);
        assert_eq!(set.len(), chunks);
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            (0..chunks).collect::<Vec<_>>()
        );
        set.clear();
        assert!(set.is_empty());
        assert_eq!(set.iter().count(), 0);
        assert_eq!(set.words().as_ptr(), original);
    }
}

#[test]
fn funded_bitmap_keeps_original_credit_through_shrink_and_final_borrower() {
    let plan = DirtyChunks::memory_plan(65).unwrap();
    assert_eq!(plan.requested_bytes(), 16);
    let budget = AllocationBudget::new(16);
    let mut lease = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    let mut set = DirtyChunks::new(65, Some(&mut lease)).unwrap();
    assert_eq!(lease.remaining_bytes(), 0);
    budget.set_limit_bytes(0);
    set.extend([64, 0, 64]);
    assert_eq!(set.iter().collect::<Vec<_>>(), [0, 64]);
    set.clear();
    assert_eq!(budget.reserved_bytes(), 16);
    let owner = Arc::new(set);
    let borrower = Arc::clone(&owner);
    drop(lease);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 16);
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn copying_uses_prepaid_geometry_and_original_parent_refuses_growth() {
    let mut source = DirtyChunks::new(65, None).unwrap();
    source.extend([0, 64]);
    let budget = AllocationBudget::new(16);
    let mut lease =
        ExecutionMemoryLease::reserve(&budget, DirtyChunks::memory_plan(65).unwrap()).unwrap();
    let mut copy = source.try_copy(Some(&mut lease)).unwrap();
    assert_eq!(copy, source);
    assert_eq!(copy.len(), source.len());
    assert!(source.try_copy(Some(&mut lease)).is_err());
    assert_eq!(budget.reserved_bytes(), 16);
    budget.set_limit_bytes(0);
    source.clear();
    source.insert(1);
    copy.copy_from(&source);
    assert_eq!(copy, source);
    assert_eq!(copy.len(), source.len());
    drop(copy);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn bounds_are_rejected_before_changing_valid_bits() {
    let mut set = DirtyChunks::new(65, None).unwrap();
    set.insert(64);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| set.insert(65)));
    assert!(result.is_err());
    assert_eq!(set.iter().collect::<Vec<_>>(), [64]);
    assert_eq!(set.len(), 1);
    let wrong_shape = DirtyChunks::new(64, None).unwrap();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| set.copy_from(&wrong_shape)));
    assert!(result.is_err());
    assert_eq!(set.iter().collect::<Vec<_>>(), [64]);
}

#[test]
fn original_budget_refuses_bitmap_before_any_backing_is_admitted() {
    let budget = AllocationBudget::new(15);
    let plan = DirtyChunks::memory_plan(65).unwrap();
    assert!(ExecutionMemoryLease::reserve(&budget, plan).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(16);
    let mut lease = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
    let set = DirtyChunks::new(65, Some(&mut lease)).unwrap();
    assert_eq!(lease.remaining_bytes(), 0);
    assert_eq!(set.words().len(), 2);
    assert_eq!(budget.reserved_bytes(), 16);
    drop(set);
    assert_eq!(budget.reserved_bytes(), 0);
}
