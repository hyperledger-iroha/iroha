//! Growth overlap, refusal and final custody for staged canonical rows.

use super::*;
use mv::allocation::AllocationRefusal;
use std::{
    alloc::Layout,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn row(index: u8, budget: &AllocationBudget) -> PairedDigestRow {
    let mut key = ChargedBuffer::new(1, budget).unwrap();
    key.append(&[index]).unwrap();
    PairedDigestRow {
        key,
        ordered_value_digest: Hash::new([index]),
        lookup_value_digest: Hash::new([index]),
    }
}
fn slots(count: usize) -> usize {
    Layout::array::<PairedDigestRow>(count).unwrap().size()
}

#[test]
fn staged_growth_funds_simultaneous_backings_and_moves_keys_without_copying() {
    let budget = AllocationBudget::new(16 * 1024);
    let mut rows = StagedRows::new(3, &budget).unwrap();
    let first = row(2, &budget);
    let original_key = first.key.as_slice().as_ptr();
    rows.push(first).unwrap();
    rows.push(row(1, &budget)).unwrap();
    assert_eq!(budget.peak_reserved_bytes(), slots(1) + slots(2) + 2);
    rows.push(row(3, &budget)).unwrap();
    assert_eq!(rows.rows.capacity(), 3);
    assert_eq!(budget.peak_reserved_bytes(), slots(2) + slots(3) + 3);
    assert_eq!(budget.reserved_bytes(), slots(3) + 3);
    assert_eq!(rows.as_slice()[0].key.as_slice().as_ptr(), original_key);
    rows.as_mut_slice()
        .sort_unstable_by_key(|row| row.key.as_slice()[0]);
    assert_eq!(rows.as_slice()[1].key.as_slice().as_ptr(), original_key);
    budget.set_limit_bytes(0);
    assert_eq!(rows.len(), 3);
    drop(rows);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn refused_growth_preserves_prior_rows_and_original_charge_before_retry() {
    let budget = AllocationBudget::new(slots(1) + slots(2) + 1);
    let mut rows = StagedRows::new(2, &budget).unwrap();
    rows.push(row(1, &budget)).unwrap();
    assert!(matches!(
        rows.push(row(2, &budget)),
        Err(LeafError::Admission(AllocationRefusal::Capacity { .. }))
    ));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.as_slice()[0].key.as_slice(), &[1]);
    assert_eq!(budget.reserved_bytes(), slots(1) + 1);
    budget.set_limit_bytes(slots(1) + slots(2) + 2);
    rows.push(row(2, &budget)).unwrap();
    assert_eq!(rows.len(), 2);
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _rows = rows;
        panic!("staged row owner unwind");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn zero_rows_and_exhausted_declared_count_do_not_allocate_slots() {
    let budget = AllocationBudget::new(1);
    let mut rows = StagedRows::new(0, &budget).unwrap();
    assert!(rows.is_empty());
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(matches!(
        rows.push(row(0, &budget)),
        Err(LeafError::RowLimit)
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}
