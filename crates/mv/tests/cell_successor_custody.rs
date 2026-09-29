//! Exact prepaid Cell identity ownership through actual capture and publication.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell as LocalCell, RefCell},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
};

use concread::ebrcell::Untracked;
use mv::{
    BlockAcquisition, BlockMode, BlockPublication, PublicationPreparationError,
    allocation::{AllocationBudget, AllocationCharge},
    cell::{Cell, CellAllocationCharges, CellPublicationSuccessor, CellPublicationSuccessorError},
};

static SERIAL: Mutex<()> = Mutex::new(());
static POINTER: AtomicUsize = AtomicUsize::new(0);
static FREED: AtomicBool = AtomicBool::new(false);
static CREDIT_AT_FREE: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static NEXT: LocalCell<Option<bool>> = const { LocalCell::new(None) };
    static COUNT: LocalCell<Option<usize>> = const { LocalCell::new(None) };
    static BUDGET: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
}

struct ObservedAllocator;
// Test-only exact System forwarding; no assertions or allocations in callbacks.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNT.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        let observe = NEXT.try_with(LocalCell::take).unwrap_or(None);
        if observe == Some(true) {
            return std::ptr::null_mut();
        }
        // SAFETY: forward the caller's nonzero layout unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if observe == Some(false) {
            POINTER.store(pointer as usize, SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let observed = POINTER
            .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
            .is_ok();
        if observed {
            let reserved = BUDGET
                .try_with(|budget| {
                    budget
                        .borrow()
                        .as_ref()
                        .map_or(0, AllocationBudget::reserved_bytes)
                })
                .unwrap_or(0);
            CREDIT_AT_FREE.store(reserved, SeqCst);
        }
        // SAFETY: return the exact live allocation and layout to System.
        unsafe { System.dealloc(pointer, layout) };
        if observed {
            FREED.store(true, SeqCst);
        }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn observe(budget: &AllocationBudget, fail: bool) {
    NEXT.with(|next| next.set(None));
    assert_eq!(POINTER.load(SeqCst), 0);
    FREED.store(false, SeqCst);
    CREDIT_AT_FREE.store(0, SeqCst);
    BUDGET.with(|observed| *observed.borrow_mut() = Some(budget.clone()));
    NEXT.with(|next| next.set(Some(fail)));
}

fn no_allocations<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNT.with(|count| count.set(None));
        }
    }
    COUNT.with(|count| {
        assert_eq!(count.get(), None);
        count.set(Some(0));
    });
    let reset = Reset;
    let result = operation();
    let count = COUNT.with(|count| count.get().unwrap());
    drop(reset);
    assert_eq!(count, 0, "original owner transfer allocated a replacement");
    result
}

fn charge(budget: &AllocationBudget, layout: Layout) -> AllocationCharge {
    budget
        .try_reserve(layout)
        .unwrap()
        .try_split(layout)
        .unwrap()
}

fn untracked() -> CellAllocationCharges<Untracked> {
    CellAllocationCharges::new(Untracked, Untracked)
}

fn successor(budget: &AllocationBudget) -> CellPublicationSuccessor {
    let original = charge(budget, CellPublicationSuccessor::allocation_layout());
    observe(budget, false);
    let token = CellPublicationSuccessor::try_from_charge(budget, original).unwrap();
    NEXT.with(|next| assert_eq!(next.take(), None));
    assert_ne!(POINTER.load(SeqCst), 0);
    token
}

fn assert_released(budget: &AllocationBudget, layout: Layout) {
    assert!(FREED.load(SeqCst));
    assert_eq!(CREDIT_AT_FREE.load(SeqCst), layout.size());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn successor_refusal_returns_original_charge_and_retries_without_new_pool_capacity() {
    let _serial = SERIAL.lock().unwrap();
    let layout = CellPublicationSuccessor::allocation_layout();
    let budget = AllocationBudget::new(layout.size());
    let foreign = AllocationBudget::new(layout.size());
    let original = charge(&budget, layout);
    let (original, error) =
        no_allocations(
            || match CellPublicationSuccessor::try_from_charge(&foreign, original) {
                Err(error) => error,
                Ok(_) => panic!("foreign source was accepted"),
            },
        );
    assert_eq!(error, CellPublicationSuccessorError::ForeignPool);
    assert!(original.belongs_to(&budget));
    assert_eq!(original.layout(), layout);
    assert_eq!(budget.reserved_bytes(), layout.size());
    assert_eq!(foreign.reserved_bytes(), 0);
    observe(&budget, true);
    let (original, error) = match CellPublicationSuccessor::try_from_charge(&budget, original) {
        Err(error) => error,
        Ok(_) => panic!("physical allocator refusal was ignored"),
    };
    NEXT.with(|next| assert_eq!(next.take(), None));
    assert_eq!(error, CellPublicationSuccessorError::Allocator { layout });
    assert!(original.belongs_to(&budget));
    assert_eq!(budget.reserved_bytes(), layout.size());
    budget.set_limit_bytes(0);
    observe(&budget, false);
    let token = CellPublicationSuccessor::try_from_charge(&budget, original).unwrap();
    NEXT.with(|next| assert_eq!(next.take(), None));
    assert!(token.belongs_to(&budget.clone()));
    assert!(!token.belongs_to(&foreign));
    assert_eq!(budget.reserved_bytes(), layout.size());
    drop(token);
    assert_released(&budget, layout);
}

#[test]
fn successor_rejects_wrong_exact_layout_before_allocating_or_refunding() {
    let _serial = SERIAL.lock().unwrap();
    let expected = CellPublicationSuccessor::allocation_layout();
    let actual = Layout::from_size_align(expected.size() - 1, expected.align()).unwrap();
    let budget = AllocationBudget::new(expected.size());
    let original = charge(&budget, actual);
    let (original, error) =
        no_allocations(
            || match CellPublicationSuccessor::try_from_charge(&budget, original) {
                Err(error) => error,
                Ok(_) => panic!("inexact layout was accepted"),
            },
        );
    assert_eq!(
        error,
        CellPublicationSuccessorError::LayoutMismatch { expected, actual }
    );
    assert_eq!(original.layout(), actual);
    assert_eq!(budget.reserved_bytes(), actual.size());
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepaid_cell_capture_and_busy_retry_keep_the_original_identity_at_full_capacity() {
    let _serial = SERIAL.lock().unwrap();
    let layout = CellPublicationSuccessor::allocation_layout();
    let budget = AllocationBudget::new(layout.size());
    let foreign = AllocationBudget::new(layout.size());
    // This test funds exactly the identity. Outer EBR/payload admission is a
    // separate obligation; it deliberately uses the explicit Untracked mode.
    let target = Cell::new(7_u64);
    let token = successor(&budget);
    let pointer = POINTER.load(SeqCst);
    let (charges, token) = no_allocations(|| {
        target
            .try_block_acquisition_with_successor(untracked(), token, &foreign)
            .err()
            .expect("a foreign aggregate cannot adopt the original token")
    });
    budget.set_limit_bytes(0);
    let mut acquisition = no_allocations(|| {
        target
            .try_block_acquisition_with_successor(charges, token, &budget)
            .unwrap_or_else(|_| panic!("original source must transfer"))
    });
    acquisition.initialize(BlockMode::Ordinary);
    let mut block = acquisition.into_block();
    let identity = block.publication_identity();
    *block.get_mut() = 9;
    let original = no_allocations(|| {
        block
            .try_detach(|_| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!("original capture must succeed"))
    });
    assert_eq!(original.publication_identity(), identity);
    assert_eq!(*original.get(), 9);
    assert_eq!(*original.get_before_block(), 7);
    assert_eq!(POINTER.load(SeqCst), pointer);
    let blocker = target.block();
    let (original, error, cleanup) = no_allocations(|| {
        original
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("the actual original target writer is busy")
    });
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    assert_eq!(original.publication_identity(), identity);
    assert_eq!(*original.get(), 9);
    assert_eq!(POINTER.load(SeqCst), pointer);
    assert_eq!(budget.reserved_bytes(), layout.size());
    drop(blocker);
    drop(cleanup);
    let prepared = no_allocations(|| {
        original
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|(_, error, _)| panic!("original retry failed: {error:?}"))
    });
    let retired = no_allocations(|| prepared.publish());
    drop(retired);
    assert_eq!(*target.view(), 9);
    assert_eq!(POINTER.load(SeqCst), pointer);
    assert!(!FREED.load(SeqCst));
    assert_eq!(budget.reserved_bytes(), layout.size());
    drop(identity);
    drop(target);
    assert_released(&budget, layout);
}

#[test]
fn prepaid_cell_attached_publication_consumes_the_same_successor_in_both_modes() {
    let _serial = SERIAL.lock().unwrap();
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        let layout = CellPublicationSuccessor::allocation_layout();
        let budget = AllocationBudget::new(layout.size());
        let target = Cell::new(7_u64);
        let mut tip = target.block();
        *tip.get_mut() = 8;
        tip.commit();
        let token = successor(&budget);
        let pointer = POINTER.load(SeqCst);
        let mut acquisition = target
            .try_block_acquisition_with_successor(untracked(), token, &budget)
            .unwrap_or_else(|_| panic!("original source must transfer"));
        acquisition.initialize(mode);
        let mut block = acquisition.into_block();
        assert_eq!(
            *block.get_before_block(),
            if mode == BlockMode::Ordinary { 8 } else { 7 }
        );
        *block.get_mut() = 9;
        let mut publication = block.publication_slot();
        budget.set_limit_bytes(0);
        no_allocations(|| publication.prepare_publication());
        no_allocations(|| publication.publish_prepared());
        assert_eq!(*target.view(), 9);
        assert_eq!(POINTER.load(SeqCst), pointer);
        drop(publication);
        assert_eq!(budget.reserved_bytes(), layout.size());
        assert!(!FREED.load(SeqCst));
        drop(target);
        assert_released(&budget, layout);
    }
}
