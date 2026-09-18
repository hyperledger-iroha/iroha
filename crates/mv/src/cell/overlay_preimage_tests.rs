//! Actual cell preimages across child application, rollback and replacement.

use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn pair<V: crate::Value + Copy>(value: Option<TouchedValue<'_, V>>) -> Option<(V, V)> {
    value.map(|value| (*value.before, *value.after))
}

#[test]
fn child_and_block_preimages_remain_distinct_across_apply_and_rollback() {
    let cell = Cell::new(10_u64);
    let mut block = cell.block();
    assert_eq!(block.get_before_block(), &10);
    assert!(block.touched_value().is_none());
    {
        let mut tx = block.transaction();
        assert!(tx.touched_value().is_none());
        assert_eq!(tx.get_before_block(), &10);
        assert_eq!(tx.get_before_transaction(), &10);
        *tx.get_mut() = 11;
        assert_eq!(pair(tx.touched_value()), Some((10, 11)));
        assert_eq!(tx.get_before_block(), &10);
        tx.apply();
    }
    assert_eq!(pair(block.touched_value()), Some((10, 11)));
    {
        let mut tx = block.transaction();
        assert!(tx.touched_value().is_none());
        assert_eq!(tx.get_before_block(), &10);
        assert_eq!(tx.get_before_transaction(), &11);
        *tx.get_mut() = 12;
        assert_eq!(pair(tx.touched_value()), Some((11, 12)));
        assert_eq!(tx.get_before_block(), &10);
        assert_eq!(tx.get_before_transaction(), &11);
    }
    assert_eq!(pair(block.touched_value()), Some((10, 11)));
    {
        let mut tx = block.transaction();
        *tx.get_mut() = 13;
        tx.apply();
    }
    assert_eq!(pair(block.touched_value()), Some((10, 13)));
    assert_eq!(block.get_before_block(), &10);
    block.commit();
    assert_eq!(cell.view().get(), &13);
    let next = cell.block();
    assert_eq!(next.get_before_block(), &13);
    assert!(next.touched_value().is_none());
}

#[test]
fn no_op_borrows_are_explicit_but_aborted_only_work_is_absent() {
    let cell = Cell::new(10_u64);
    let mut block = cell.block();
    {
        let mut tx = block.transaction();
        *tx.get_mut() = 11;
        assert_eq!(pair(tx.touched_value()), Some((10, 11)));
    }
    assert!(!block.is_dirty());
    assert!(block.touched_value().is_none());
    assert_eq!(block.get_before_block(), &10);
    {
        let mut tx = block.transaction();
        assert_eq!(tx.get_mut(), &mut 10);
        assert_eq!(pair(tx.touched_value()), Some((10, 10)));
        tx.apply();
    }
    assert_eq!(pair(block.touched_value()), Some((10, 10)));
    *block.get_mut() = 11;
    *block.get_mut() = 10;
    assert_eq!(pair(block.touched_value()), Some((10, 10)));
}

#[test]
fn replacement_preimage_is_reverted_parent_not_discarded_tip() {
    let cell = Cell::new(10_u64);
    {
        let mut block = cell.block();
        *block.get_mut() = 11;
        block.commit();
    }
    let previous = cell.view();
    let mut block = cell.block_and_revert();
    assert!(block.is_dirty());
    assert_eq!(block.get_before_block(), &10);
    assert!(block.touched_value().is_none());
    {
        let mut tx = block.transaction();
        assert_eq!(tx.get_before_block(), &10);
        assert_eq!(tx.get_before_transaction(), &10);
        *tx.get_mut() = 12;
        assert_eq!(pair(tx.touched_value()), Some((10, 12)));
        tx.apply();
    }
    assert_eq!(pair(block.touched_value()), Some((10, 12)));
    block.commit();
    assert_eq!(previous.get(), &11);
    assert_eq!(cell.view().get(), &12);
}

#[test]
fn unwind_and_other_cells_cannot_replace_owned_preimages() {
    let first = Cell::new(10_u64);
    let second = Cell::new(20_u64);
    let mut first_block = first.block();
    let mut second_block = second.block();
    *first_block.get_mut() = 11;
    *second_block.get_mut() = 21;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut tx = first_block.transaction();
        *tx.get_mut() = 99;
        assert_eq!(tx.get_before_block(), &10);
        assert_eq!(tx.get_before_transaction(), &11);
        assert_eq!(pair(second_block.touched_value()), Some((20, 21)));
        panic!("interrupt actual cell child");
    }));
    assert!(result.is_err());
    assert_eq!(pair(first_block.touched_value()), Some((10, 11)));
    assert_eq!(pair(second_block.touched_value()), Some((20, 21)));
}

#[test]
fn cell_preimage_observation_never_clones_the_value() {
    struct Counted(u64, Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.1.fetch_add(1, Ordering::Relaxed);
            Self(self.0, self.1.clone())
        }
    }
    let clones = Arc::new(AtomicUsize::new(0));
    let cell = Cell::new(Counted(10, clones.clone()));
    let mut block = cell.block();
    block.get_mut().0 = 11;
    clones.store(0, Ordering::Relaxed);
    assert_eq!(block.get_before_block().0, 10);
    let change = block.touched_value().unwrap();
    assert_eq!(change.before.0, 10);
    assert_eq!(change.after.0, 11);
    assert_eq!(clones.load(Ordering::Relaxed), 0);
    let mut tx = block.transaction();
    tx.get_mut().0 = 12;
    clones.store(0, Ordering::Relaxed);
    assert_eq!(tx.get_before_block().0, 10);
    assert_eq!(tx.get_before_transaction().0, 11);
    let change = tx.touched_value().unwrap();
    assert_eq!(change.before.0, 11);
    assert_eq!(change.after.0, 12);
    assert_eq!(clones.load(Ordering::Relaxed), 0);
}
