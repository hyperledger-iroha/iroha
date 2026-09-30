//! Actual allocator refusal and allocation-free movement of initial Cell backing.

use iroha_allocation::AllocationBudget;
use mv::cell::{CellInitialization, CellInitializationError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Mutex,
};

static SERIAL: Mutex<()> = Mutex::new(());
thread_local! {
    static FAIL: Cell<Option<(Layout, usize)>> = const { Cell::new(None) };
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}
struct ObservedAllocator;
// This isolated test forwards each exact System layout and uses no allocating
// observations inside allocator callbacks.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNT.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        let refuse = FAIL
            .try_with(|fail| match fail.get() {
                Some((expected, remaining)) if expected == layout => {
                    if remaining == 0 {
                        fail.set(None);
                        true
                    } else {
                        fail.set(Some((expected, remaining - 1)));
                        false
                    }
                }
                _ => false,
            })
            .unwrap_or(false);
        if refuse {
            return std::ptr::null_mut();
        }
        // SAFETY: preserve the original requested layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: return the same allocation using its original layout.
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

#[test]
fn refused_initial_owner_and_either_ebr_shell_refund_all_original_backing() {
    let _serial = SERIAL.lock().unwrap();
    // These large inline EBR layouts are distinct from notification/native mutex
    // controls, so refusal targets actual fallible backing rather than mutex setup.
    let layouts = CellInitialization::<[u8; 257]>::allocation_layouts();
    let demand = layouts
        .into_iter()
        .map(|layout| layout.size())
        .sum::<usize>();
    for (layout, prior_matches) in [
        (layouts[2], 0),
        (layouts[0], 0),
        (layouts[1], usize::from(layouts[0] == layouts[1])),
    ] {
        let original = AllocationBudget::new(demand);
        FAIL.with(|fail| fail.set(Some((layout, prior_matches))));
        let result = CellInitialization::<[u8; 257]>::try_reserve(&original);
        let unused_failure = FAIL.with(Cell::take);
        assert!(
            unused_failure.is_none(),
            "actual requested backing must reach the allocator"
        );
        match result {
            Err(CellInitializationError::Allocator { layout: actual }) => {
                assert_eq!(actual, layout)
            }
            _ => panic!("physical refusal must return as a local allocation error"),
        }
        assert_eq!(
            original.reserved_bytes(),
            0,
            "all partial controls and uninitialized EBR shells free before refund"
        );
        // Retry uses the same original pool with the same finite capacity.
        let retained = CellInitialization::<[u8; 257]>::try_reserve(&original).unwrap();
        assert_eq!(original.reserved_bytes(), demand);
        drop(retained);
        assert_eq!(original.reserved_bytes(), 0);
    }
}

#[test]
fn initialized_payloads_use_original_reserved_backing_without_allocating() {
    let _serial = SERIAL.lock().unwrap();
    let demand = CellInitialization::<u64>::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum();
    let original = AllocationBudget::new(demand);
    let retained = CellInitialization::try_reserve(&original).unwrap();
    original.set_limit_bytes(0);
    COUNT.with(|count| count.set(Some(0)));
    let cell = retained.initialize(9_u64, Some(6));
    let allocations = COUNT.with(|count| count.replace(None).unwrap());
    assert_eq!(
        allocations, 0,
        "initialization cannot replace an original physical allocation"
    );
    assert_eq!(*cell.view().get(), 9);
    assert_eq!(*cell.predecessor_view().get(), Some(6));
    assert_eq!(original.reserved_bytes(), demand);
}
