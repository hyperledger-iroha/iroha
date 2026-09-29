//! Physical writer backing, original pool refusal and retained publication custody.

use mv::{
    BlockAcquisition, BlockMode, PublicationPreparationError, Value,
    allocation::{AllocationBudget, AllocationCharge},
    cell::{
        Cell as MvCell, CellAllocationCharges, CellGenerationBacking, CellGenerationBackingError,
        CellInitialization, CellPublicationSuccessor,
    },
};
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

fn charges<V: Value>(budget: &AllocationBudget) -> CellAllocationCharges<AllocationCharge> {
    let [current, undo] = MvCell::<V, AllocationCharge>::allocation_layouts();
    let mut original = budget.try_reserve_layouts([current, undo]).unwrap();
    CellAllocationCharges::new(
        original.try_split(current).unwrap(),
        original.try_split(undo).unwrap(),
    )
}
fn no_allocations<T>(operation: impl FnOnce() -> T) -> T {
    COUNT.with(|count| count.set(Some(0)));
    let result = operation();
    let allocations = COUNT.with(|count| count.replace(None).unwrap());
    assert_eq!(
        allocations, 0,
        "original backing transfer allocated a replacement"
    );
    result
}
fn collect_until_empty(budget: &AllocationBudget) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while budget.reserved_bytes() != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "retained original EBR charge"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
}

#[test]
fn either_physical_refusal_returns_both_original_charges_for_retry_after_limit_shrink() {
    let _serial = SERIAL.lock().unwrap();
    let [current, undo] = MvCell::<[u8; 257], AllocationCharge>::allocation_layouts();
    for (layout, previous) in [(current, 0), (undo, usize::from(current == undo))] {
        let source = AllocationBudget::new(current.size() + undo.size());
        let original = charges::<[u8; 257]>(&source);
        FAIL.with(|fail| fail.set(Some((layout, previous))));
        let (original, error) =
            match CellGenerationBacking::<[u8; 257]>::try_from_charges(&source, original) {
                Err(refusal) => refusal,
                Ok(_) => panic!("exact allocator refusal must propagate"),
            };
        let unused_failure = FAIL.with(Cell::take);
        assert!(unused_failure.is_none());
        assert_eq!(error, CellGenerationBackingError::Allocator { layout });
        assert_eq!(
            source.reserved_bytes(),
            current.size() + undo.size(),
            "neither charge may be refunded or replaced"
        );
        source.set_limit_bytes(0);
        let backing = CellGenerationBacking::<[u8; 257]>::try_from_charges(&source, original)
            .unwrap_or_else(|_| panic!("original admitted charges retry without a new grant"));
        assert!(backing.belongs_to(&source.clone()));
        assert_eq!(source.reserved_bytes(), current.size() + undo.size());
        drop(backing);
        assert_eq!(source.reserved_bytes(), 0);
    }
}

#[test]
fn wrong_pool_or_exact_layout_returns_unchanged_admission_before_any_allocation() {
    let _serial = SERIAL.lock().unwrap();
    let [current, undo] = MvCell::<u64, AllocationCharge>::allocation_layouts();
    let source = AllocationBudget::new(current.size() + undo.size());
    let foreign = AllocationBudget::new(source.limit_bytes());
    let original = charges::<u64>(&source);
    let (original, error) = no_allocations(|| {
        CellGenerationBacking::<u64>::try_from_charges(&foreign, original)
            .err()
            .unwrap()
    });
    assert_eq!(error, CellGenerationBackingError::ForeignPool);
    assert_eq!(source.reserved_bytes(), current.size() + undo.size());
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(original);
    assert_eq!(source.reserved_bytes(), 0);
    let wrong = Layout::from_size_align(current.size(), current.align() * 2).unwrap();
    let mut original = source.try_reserve_layouts([wrong, undo]).unwrap();
    let charges = CellAllocationCharges::new(
        original.try_split(wrong).unwrap(),
        original.try_split(undo).unwrap(),
    );
    let (charges, error) = no_allocations(|| {
        CellGenerationBacking::<u64>::try_from_charges(&source, charges)
            .err()
            .unwrap()
    });
    assert_eq!(
        error,
        CellGenerationBackingError::LayoutMismatch {
            expected: current,
            actual: wrong
        }
    );
    assert_eq!(source.reserved_bytes(), current.size() + undo.size());
    drop(charges);
    assert_eq!(source.reserved_bytes(), 0);
}

#[test]
fn original_backing_survives_zero_capacity_acquisition_capture_busy_retry_and_publication() {
    let _serial = SERIAL.lock().unwrap();
    let initial_pool = AllocationBudget::new(1024 * 1024);
    let target = CellInitialization::try_reserve(&initial_pool)
        .unwrap()
        .initialize(7_u64, None);
    // Warm native locks and collector infrastructure, which have separate policy.
    drop(target.view());
    drop(target.block_charged(charges::<u64>(&initial_pool)));
    let [current, undo] = MvCell::<u64, AllocationCharge>::allocation_layouts();
    let successor_layout = CellPublicationSuccessor::allocation_layout();
    let demand = current.size() + undo.size() + successor_layout.size();
    let source = AllocationBudget::new(demand);
    let foreign = AllocationBudget::new(demand);
    let backing = CellGenerationBacking::try_from_charges(&source, charges::<u64>(&source))
        .unwrap_or_else(|_| panic!("exact original generation backing"));
    let successor_charge = source
        .try_reserve(successor_layout)
        .unwrap()
        .try_split(successor_layout)
        .unwrap();
    let successor = CellPublicationSuccessor::try_from_charge(&source, successor_charge).unwrap();
    let (backing, successor) = no_allocations(|| {
        target
            .try_block_acquisition_with_backing(backing, successor, &foreign)
            .err()
            .unwrap()
    });
    assert!(backing.belongs_to(&source));
    assert!(successor.belongs_to(&source));
    assert_eq!(source.reserved_bytes(), demand);
    source.set_limit_bytes(0);
    let mut slot = no_allocations(|| {
        target
            .try_block_acquisition_with_backing(backing, successor, &source)
            .unwrap_or_else(|_| panic!("same original pair and successor"))
    });
    no_allocations(|| slot.initialize(BlockMode::Ordinary));
    let mut block = slot.into_block();
    *block.get_mut() = 9;
    let original = no_allocations(|| {
        block
            .try_detach(|_| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!("same original capture"))
    });
    let pointer = std::ptr::from_ref(original.get());
    assert_eq!(source.reserved_bytes(), demand);
    let held = target.block_charged(charges::<u64>(&initial_pool));
    let (original, error, cleanup) = original
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .err()
        .unwrap();
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    assert_eq!(std::ptr::from_ref(original.get()), pointer);
    assert_eq!(source.reserved_bytes(), demand);
    drop((cleanup, held));
    let prepared = original
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("same original publication retry"));
    drop(prepared.publish());
    assert_eq!(*target.view().get(), 9);
    assert_eq!(*target.predecessor_view().get(), Some(7));
    assert_eq!(source.reserved_bytes(), demand);
    drop(target);
    collect_until_empty(&source);
}

#[test]
fn poisoned_original_writer_returns_unchanged_reserved_backing_and_charge() {
    use concread::ebrcell::{EbrCell, EbrCellWriterAdmissionError, ReservedEbrCell};
    let _serial = SERIAL.lock().unwrap();
    let layout = EbrCell::<u64, AllocationCharge>::allocation_layout();
    let source = AllocationBudget::new(2 * layout.size());
    let mut original = source.try_reserve_layouts([layout, layout]).unwrap();
    let target = EbrCell::new_charged(7_u64, original.try_split(layout).unwrap());
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _held = target.acquire_writer();
        panic!("poison only this original physical writer");
    }));
    assert!(panic.is_err());
    let backing = ReservedEbrCell::try_new(original.try_split(layout).unwrap()).unwrap();
    source.set_limit_bytes(0);
    let (held, backing, error) =
        no_allocations(
            || match target.acquire_writer().try_clone_reserved(backing) {
                Err(original) => original,
                Ok(_) => panic!("poisoned source cannot clone a generation"),
            },
        );
    assert!(matches!(error, EbrCellWriterAdmissionError::Poisoned));
    assert!(
        target.try_acquire_writer().is_none(),
        "original refusal still owns its writer"
    );
    assert_eq!(source.reserved_bytes(), 2 * layout.size());
    drop(held);
    let charge = no_allocations(|| backing.into_charge());
    assert!(charge.belongs_to(&source));
    assert_eq!(charge.layout(), layout);
    assert_eq!(source.reserved_bytes(), 2 * layout.size());
    drop(charge);
    assert_eq!(source.reserved_bytes(), layout.size());
    drop(target);
    collect_until_empty(&source);
}

#[test]
fn clone_unwind_releases_writers_and_conservatively_retains_the_actual_failed_backing() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    #[derive(Debug)]
    struct FailsClone(Arc<AtomicBool>);
    impl Clone for FailsClone {
        fn clone(&self) -> Self {
            assert!(
                !self.0.swap(false, Ordering::SeqCst),
                "deliberate payload clone panic"
            );
            Self(Arc::clone(&self.0))
        }
    }
    let _serial = SERIAL.lock().unwrap();
    let initial = AllocationBudget::new(1024 * 1024);
    let fail = Arc::new(AtomicBool::new(true));
    let target = CellInitialization::try_reserve(&initial)
        .unwrap()
        .initialize(FailsClone(fail), None);
    let [current, undo] = MvCell::<FailsClone, AllocationCharge>::allocation_layouts();
    let next = CellPublicationSuccessor::allocation_layout();
    let source = AllocationBudget::new(current.size() + undo.size() + next.size());
    let backing = CellGenerationBacking::try_from_charges(&source, charges::<FailsClone>(&source))
        .unwrap_or_else(|_| panic!("original backing"));
    let successor = CellPublicationSuccessor::try_from_charge(
        &source,
        source.try_reserve(next).unwrap().try_split(next).unwrap(),
    )
    .unwrap();
    let mut slot = target
        .try_block_acquisition_with_backing(backing, successor, &source)
        .unwrap_or_else(|_| panic!("original source"));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        slot.initialize(BlockMode::Ordinary)
    }));
    assert!(panic.is_err());
    drop(slot);
    assert_eq!(
        source.reserved_bytes(),
        current.size(),
        "a possibly leaking Clone cannot refund its actual original physical shell"
    );
    assert!(
        !target.view().get().0.load(Ordering::SeqCst),
        "original current source remains readable"
    );
}
