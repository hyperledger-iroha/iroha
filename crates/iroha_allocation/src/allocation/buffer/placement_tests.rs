//! Exact admitted backing can initialize in place without a whole-value move.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::SeqCst},
};

struct Part {
    dropped: Arc<AtomicUsize>,
    budget: AllocationBudget,
    expected: usize,
}
impl Drop for Part {
    fn drop(&mut self) {
        assert_eq!(
            self.budget.reserved_bytes(),
            self.expected,
            "real backing remains charged during payload destruction"
        );
        self.dropped.fetch_add(1, SeqCst);
    }
}
struct Pair {
    first: Part,
    second: Part,
}

#[test]
fn exact_placement_retains_allocation_identity_and_charge_through_initialized_drop() {
    let bytes = Layout::new::<Pair>().size();
    let budget = AllocationBudget::new(bytes);
    let foreign = AllocationBudget::new(bytes);
    let mut original = ChargedBuffer::<Pair>::new(1, &budget).unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    let pointer = original.spare_capacity_mut()[0].as_mut_ptr();
    // SAFETY: two distinct aligned field projections initialize the complete
    // sole Pair in this original backing. No incomplete Pair reference is made.
    #[allow(unsafe_code)]
    unsafe {
        std::ptr::addr_of_mut!((*pointer).first).write(Part {
            dropped: dropped.clone(),
            budget: budget.clone(),
            expected: bytes,
        });
        std::ptr::addr_of_mut!((*pointer).second).write(Part {
            dropped: dropped.clone(),
            budget: budget.clone(),
            expected: bytes,
        });
        original.set_initialized_len(1);
    }
    assert_eq!(std::ptr::from_ref(&original.as_slice()[0]), pointer);
    assert!(original.spare_capacity_mut().is_empty());
    assert!(original.belongs_to(&budget));
    assert!(!original.belongs_to(&foreign));
    assert_eq!(foreign.reserved_bytes(), 0);
    budget.set_limit_bytes(0);
    drop(original);
    assert_eq!(dropped.load(SeqCst), 2);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn partial_placement_never_exposes_or_drops_an_uninitialized_element() {
    let bytes = Layout::new::<Pair>().size();
    let budget = AllocationBudget::new(bytes);
    let mut original = ChargedBuffer::<Pair>::new(1, &budget).unwrap();
    let dropped = Arc::new(AtomicUsize::new(0));
    let pointer = original.spare_capacity_mut()[0].as_mut_ptr();
    // SAFETY: only first is initialized and then destroyed in place. Length
    // stays zero throughout, so no Pair or uninitialized second is ever dropped.
    #[allow(unsafe_code)]
    unsafe {
        std::ptr::addr_of_mut!((*pointer).first).write(Part {
            dropped: dropped.clone(),
            budget: budget.clone(),
            expected: bytes,
        });
        assert!(original.as_slice().is_empty());
        std::ptr::drop_in_place(std::ptr::addr_of_mut!((*pointer).first));
    }
    assert_eq!(dropped.load(SeqCst), 1);
    drop(original);
    assert_eq!(dropped.load(SeqCst), 1);
    assert_eq!(budget.reserved_bytes(), 0);
}
