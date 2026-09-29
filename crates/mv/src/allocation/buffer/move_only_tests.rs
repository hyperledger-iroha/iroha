//! Move-only initialized values retain their original fixed backing charge.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct DropValue(Arc<AtomicUsize>);

impl Drop for DropValue {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn move_only_elements_keep_exact_backing_charge_through_drain_and_unwind() {
    let bytes = 2 * std::mem::size_of::<DropValue>();
    let budget = AllocationBudget::new(bytes);
    let dropped = Arc::new(AtomicUsize::new(0));
    let empty = ChargedBuffer::<DropValue>::new(0, &budget).unwrap();
    assert_eq!(empty.capacity(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
    drop(empty);

    let mut buffer = ChargedBuffer::new(2, &budget).unwrap();
    buffer.push_reserved(DropValue(Arc::clone(&dropped)));
    buffer.push_reserved(DropValue(Arc::clone(&dropped)));
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(buffer.pop().unwrap());
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    buffer.push_reserved(DropValue(Arc::clone(&dropped)));
    let moved = buffer.drain_all().next().unwrap();
    assert_eq!(dropped.load(Ordering::Relaxed), 2);
    assert!(buffer.as_slice().is_empty());
    assert_eq!(budget.reserved_bytes(), bytes);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _buffer = buffer;
        panic!("drop charged backing during unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    drop(moved);
    assert_eq!(dropped.load(Ordering::Relaxed), 3);
}
