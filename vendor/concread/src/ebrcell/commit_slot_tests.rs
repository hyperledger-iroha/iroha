//! Caller-owned EBR preparation preserves the original allocation and charge.

use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn commit_slot_rejects_reprepare_without_dropping_or_copying_original_allocation() {
    let cell = EbrCell::new(7_u64);
    let original = cell.read();
    let mut writer = cell.write();
    *writer = 9;
    let address = std::ptr::from_ref(&*writer);
    let mut slot = writer.commit_slot();
    slot.prepare();
    assert!(catch_unwind(AssertUnwindSafe(|| slot.prepare())).is_err());
    assert!(slot.is_prepared());
    assert!(cell.try_acquire_writer().is_none());
    let writer = slot.abort();
    assert_eq!(std::ptr::from_ref(&*writer), address);
    assert!(
        !cell.is_poisoned(),
        "caught validation retained original mutex"
    );
    let mut slot = writer.commit_slot();
    slot.prepare();
    let retirement = slot.into_prepared().publish().release();
    assert!(cell.try_acquire_writer().is_some());
    assert_eq!(*original, 7);
    assert_eq!(*cell.read(), 9);
    drop(retirement);
    drop(original);
}
