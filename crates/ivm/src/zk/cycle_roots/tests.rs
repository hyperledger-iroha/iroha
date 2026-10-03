//! Exact row growth, original-pool refusal and independent copy reclamation.

use super::*;

fn row(value: u64) -> StepEntry {
    let root = HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(value.to_le_bytes()));
    StepEntry {
        pc: value,
        reg_root: root,
        mem_root: root,
    }
}
fn append(log: &mut StepLog, value: u64) {
    log.prepare_cycles(1).unwrap();
    let row = row(value);
    log.record_reserved(row.pc, row.reg_root, row.mem_root);
}

#[test]
fn zero_and_one_short_original_admission_preserve_rows_until_exact_retry() {
    let size = std::mem::size_of::<StepEntry>();
    let original = AllocationBudget::new(0);
    let mut log = StepLog::new(Some(&original));
    assert!(
        matches!(log.prepare_cycles(1), Err(VMError::AllocationDeferred(AllocationRefusal::ExceedsLimit { requested_bytes, .. })) if requested_bytes == 4 * size)
    );
    assert_eq!(original.peak_reserved_bytes(), 0);
    original.set_limit_bytes(4 * size);
    for value in 0..4 {
        append(&mut log, value);
    }
    let pointer = log.as_slice().as_ptr();
    original.set_limit_bytes(12 * size - 1);
    assert!(
        matches!(log.prepare_cycles(1), Err(VMError::AllocationDeferred(AllocationRefusal::Capacity { requested_bytes, .. })) if requested_bytes == 8 * size)
    );
    assert_eq!(log.as_slice().as_ptr(), pointer);
    assert_eq!(log.as_slice(), &[row(0), row(1), row(2), row(3)]);
    assert_eq!(original.reserved_bytes(), 4 * size);
    original.set_limit_bytes(12 * size);
    append(&mut log, 4);
    assert_ne!(log.as_slice().as_ptr(), pointer);
    assert_eq!(original.peak_reserved_bytes(), 12 * size);
    assert_eq!(original.reserved_bytes(), 8 * size);
    original.set_limit_bytes(0);
    log.prepare_cycles(3).unwrap();
    for value in 5..8 {
        let row = row(value);
        log.record_reserved(row.pc, row.reg_root, row.mem_root);
    }
    assert_eq!(log.as_slice().len(), 8);
    drop(log);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn admitted_allocator_refusal_and_overflow_leave_the_previous_owner_unchanged() {
    let size = std::mem::size_of::<StepEntry>();
    let original = AllocationBudget::new(64 * size);
    let mut log = StepLog::new(Some(&original));
    append(&mut log, 17);
    let pointer = log.as_slice().as_ptr();
    REFUSE_NEXT_ALLOCATION.set(true);
    assert!(matches!(
        log.prepare_cycles(12),
        Err(VMError::ExecutionDeferred(
            ExecutionDeferral::AllocationUnavailable
        ))
    ));
    assert_eq!(log.as_slice(), &[row(17)]);
    assert_eq!(log.as_slice().as_ptr(), pointer);
    assert_eq!(original.reserved_bytes(), 4 * size);
    assert!(matches!(
        log.prepare_cycles(u64::MAX),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::DemandOverflow
        ))
    ));
    assert_eq!(original.reserved_bytes(), 4 * size);
    log.prepare_cycles(12).unwrap();
    assert_eq!(log.rows.capacity(), 13);
    assert_eq!(original.reserved_bytes(), 13 * size);
}

#[test]
fn clear_retains_credit_and_reset_keeps_the_original_pool_for_future_rows() {
    let size = std::mem::size_of::<StepEntry>();
    let original = AllocationBudget::new(4 * size);
    let mut log = StepLog::new(Some(&original));
    append(&mut log, 13);
    original.set_limit_bytes(0);
    log.clear();
    assert!(log.as_slice().is_empty());
    assert_eq!(original.reserved_bytes(), 4 * size);
    append(&mut log, 29);
    assert_eq!(log.as_slice(), &[row(29)]);
    log.reset();
    assert_eq!(original.reserved_bytes(), 0);
    assert!(matches!(
        log.prepare_cycles(1),
        Err(VMError::AllocationDeferred(_))
    ));
    original.set_limit_bytes(4 * size);
    append(&mut log, 31);
    assert_eq!(original.reserved_bytes(), 4 * size);
}

#[test]
fn test_only_copy_keeps_its_independent_original_charge_after_source_drop() {
    let size = std::mem::size_of::<StepEntry>();
    let original = AllocationBudget::new(4 * size);
    let mut log = StepLog::new(Some(&original));
    append(&mut log, 3);
    assert!(matches!(
        log.try_clone_allocation(),
        Err(VMError::AllocationDeferred(_))
    ));
    original.set_limit_bytes(8 * size);
    let copy = log.try_clone_allocation().unwrap();
    assert_eq!(log.as_slice(), copy.as_slice());
    assert_ne!(log.as_slice().as_ptr(), copy.as_slice().as_ptr());
    log.reset();
    assert_eq!(original.reserved_bytes(), 4 * size);
    assert_eq!(copy.as_slice(), &[row(3)]);
    drop(copy);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn explicit_standalone_buffer_is_fallible_but_never_adopts_a_new_pool() {
    let mut log = StepLog::new(None);
    crate::cache_memory::refuse_next_owned_vec_growth_for_test();
    assert!(matches!(
        log.prepare_cycles(1),
        Err(VMError::ExecutionDeferred(
            ExecutionDeferral::AllocationUnavailable
        ))
    ));
    assert!(log.as_slice().is_empty());
    for value in 0..12 {
        append(&mut log, value);
    }
    assert!(log.original.is_none());
    assert_eq!(log.as_slice().len(), 12);
    let copy = log.try_clone_allocation().unwrap();
    assert!(copy.original.is_none());
    assert_eq!(log.as_slice(), copy.as_slice());
    log.clear();
    log.reset();
    assert_eq!(log.allocated_bytes().unwrap(), 0);
}

#[test]
fn unwind_reclaims_initialized_and_spare_rows_from_the_original_pool() {
    let original = AllocationBudget::new(16 * std::mem::size_of::<StepEntry>());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut log = StepLog::new(Some(&original));
        append(&mut log, 5);
        log.prepare_cycles(7).unwrap();
        assert_eq!(
            original.reserved_bytes(),
            8 * std::mem::size_of::<StepEntry>()
        );
        panic!("cycle owner unwind fixture");
    }));
    assert!(outcome.is_err());
    assert_eq!(original.reserved_bytes(), 0);
}
