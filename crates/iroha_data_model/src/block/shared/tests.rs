//! Original graph, source-preserving refusal and final physical-control custody.

use super::*;
use crate::block::output_test_support as fixture;

#[test]
fn shared_block_moves_original_graph_and_retains_credit_through_final_clone() {
    let block = fixture::proposal(2);
    let original = block.external_entrypoints_slice().as_ptr();
    let wire = block.encode_wire().unwrap();
    let bytes = SharedSignedBlock::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let foreign = AllocationBudget::new(bytes);
    let shared = SharedSignedBlock::try_new(block, &budget).unwrap();
    assert_eq!(shared.external_entrypoints_slice().as_ptr(), original);
    assert_eq!(shared.encode_wire().unwrap(), wire);
    assert!(shared.belongs_to(&budget));
    assert!(!shared.belongs_to(&foreign));
    let final_owner = shared.clone();
    assert!(SharedSignedBlock::ptr_eq(&shared, &final_owner));
    assert_eq!(shared, final_owner);
    drop(shared);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(matches!(
        budget.try_reserve_bytes(1),
        Err(AllocationRefusal::Capacity { .. })
    ));
    assert_eq!(final_owner.external_entrypoints_slice().as_ptr(), original);
    drop(final_owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn shared_block_refusal_returns_same_graph_and_prepaid_shortage_preserves_parent() {
    let block = fixture::proposal(1);
    let original = block.external_entrypoints_slice().as_ptr();
    let bytes = SharedSignedBlock::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let held = budget.try_reserve_bytes(bytes).unwrap();
    let (block, error) = SharedSignedBlock::try_new(block, &budget).unwrap_err();
    assert!(matches!(
        error,
        SharedBlockAdmissionError::Admission(AllocationRefusal::Capacity { .. })
    ));
    assert_eq!(block.external_entrypoints_slice().as_ptr(), original);
    drop(held);
    let mut parent = budget.try_reserve_bytes(bytes - 1).unwrap();
    let (block, error) = SharedSignedBlock::from_reservation(block, &mut parent).unwrap_err();
    assert!(matches!(
        error,
        SharedBlockAdmissionError::Allocation(PrepaidSharedError::Reservation(_))
    ));
    assert_eq!(parent.remaining_bytes(), bytes - 1);
    assert_eq!(block.external_entrypoints_slice().as_ptr(), original);
    drop(parent);
    let mut parent = budget.try_reserve_bytes(bytes).unwrap();
    let shared = SharedSignedBlock::from_reservation(block, &mut parent).unwrap();
    assert_eq!(parent.remaining_bytes(), 0);
    assert_eq!(shared.external_entrypoints_slice().as_ptr(), original);
    drop(shared);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepaid_shared_block_shell_can_be_cancelled_or_initialized_without_new_admission() {
    let bytes = SharedSignedBlock::allocation_layout().size();
    let budget = AllocationBudget::new(bytes);
    let shell = SharedSignedBlock::reserve(&budget).unwrap();
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(shell);
    assert_eq!(budget.reserved_bytes(), 0);
    let block = fixture::proposal(1);
    let original = block.external_entrypoints_slice().as_ptr();
    let shell = SharedSignedBlock::reserve(&budget).unwrap();
    budget.set_limit_bytes(0);
    let shared = shell.initialize(block);
    assert_eq!(shared.external_entrypoints_slice().as_ptr(), original);
    assert!(shared.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(shared);
    assert_eq!(budget.reserved_bytes(), 0);
}
