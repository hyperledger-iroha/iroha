//! Real allocator controls for the ordered digest tree's private shared owner.

use iroha_allocation::release::ReleaseRegistration;
use iroha_allocation::{AllocationBudget, AllocationRefusal};
use iroha_crypto::{Hash, NoritoKeyDigestRangeTreeV1, NoritoKeyRangeError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Poll, Wake, Waker},
};

struct ObservedAllocator;
thread_local! {
    static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static FAIL_AT: Cell<Option<usize>> = const { Cell::new(None) };
    static ATTEMPTS: Cell<usize> = const { Cell::new(0) };
    static BUDGET: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
}
static SERIAL: Mutex<()> = Mutex::new(());
static COUNT: AtomicUsize = AtomicUsize::new(0);
static POINTER: AtomicUsize = AtomicUsize::new(0);
static TOTAL_SIZE: AtomicUsize = AtomicUsize::new(0);
static UNDERFUNDED: AtomicBool = AtomicBool::new(false);
static SIZE: AtomicUsize = AtomicUsize::new(0);
static RESERVED: AtomicUsize = AtomicUsize::new(0);
static FREED: AtomicBool = AtomicBool::new(false);

// Test-only instrumentation forwards each exact System allocation/deallocation.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let fail = FAIL_AT
            .try_with(|target| {
                target.get().is_some_and(|at| {
                    ATTEMPTS.with(|count| {
                        let ordinal = count.get() + 1;
                        count.set(ordinal);
                        if ordinal == at {
                            target.set(None);
                            true
                        } else {
                            false
                        }
                    })
                })
            })
            .unwrap_or(false);
        if fail {
            return std::ptr::null_mut();
        }
        // SAFETY: forward the allocator's original request unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if OBSERVE.try_with(Cell::get).unwrap_or(false) {
            let ordinal = COUNT.fetch_add(1, SeqCst);
            let total = TOTAL_SIZE.fetch_add(layout.size(), SeqCst) + layout.size();
            let credit = BUDGET
                .try_with(|budget| {
                    budget
                        .borrow()
                        .as_ref()
                        .map_or(0, AllocationBudget::reserved_bytes)
                })
                .unwrap_or(0);
            if credit < total {
                UNDERFUNDED.store(true, SeqCst);
            }
            if ordinal == 0 {
                POINTER.store(pointer as usize, SeqCst);
                SIZE.store(layout.size(), SeqCst);
                RESERVED.store(
                    BUDGET
                        .try_with(|budget| {
                            budget
                                .borrow()
                                .as_ref()
                                .map_or(0, AllocationBudget::reserved_bytes)
                        })
                        .unwrap_or(0),
                    SeqCst,
                );
            }
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let tracked = POINTER
            .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
            .is_ok();
        // SAFETY: forward the exact allocation's pointer and layout.
        unsafe { System.dealloc(pointer, layout) };
        if tracked {
            FREED.store(true, SeqCst);
        }
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn observe(budget: &AllocationBudget) {
    OBSERVE.set(false);
    BUDGET.with(|slot| *slot.borrow_mut() = Some(budget.clone()));
    COUNT.store(0, SeqCst);
    POINTER.store(0, SeqCst);
    SIZE.store(0, SeqCst);
    TOTAL_SIZE.store(0, SeqCst);
    UNDERFUNDED.store(false, SeqCst);
    RESERVED.store(0, SeqCst);
    FREED.store(false, SeqCst);
    OBSERVE.set(true);
}

fn empty_tree(
    budget: &AllocationBudget,
    schema: Hash,
) -> Result<NoritoKeyDigestRangeTreeV1, NoritoKeyRangeError> {
    NoritoKeyDigestRangeTreeV1::from_sorted_digests(
        schema,
        b"allocation control",
        std::iter::empty::<(&[u8], Hash)>(),
        0,
        budget,
    )
}

struct ReentrantAfterFree {
    budget: AllocationBudget,
    observer_bytes: usize,
    bytes: usize,
    wakes: AtomicUsize,
}
impl Wake for ReentrantAfterFree {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        assert!(
            FREED.load(SeqCst),
            "owner credit refunded before original System.dealloc returned"
        );
        assert_eq!(self.budget.reserved_bytes(), self.observer_bytes);
        self.wakes.fetch_add(1, SeqCst);
        // A reentrant consumer may reuse all released payload credit immediately. Its
        // original owner allocation must already be gone before this admission.
        let reused = self.budget.try_reserve_bytes(self.bytes).unwrap();
        assert!(FREED.load(SeqCst));
        drop(reused);
    }
}

#[test]
fn last_private_owner_deallocates_before_reentrant_refund_in_single_concurrent_and_unwind_paths() {
    let _serial = SERIAL.lock().unwrap();
    let schema = Hash::new(b"owner deallocation");
    for owners in [1, 8, 0] {
        let budget = AllocationBudget::new(4096);
        let observer_bytes = ReleaseRegistration::allocation_layout().size();
        let mut registration = ReleaseRegistration::from_reservation(
            &mut budget
                .try_reserve(ReleaseRegistration::allocation_layout())
                .unwrap(),
        )
        .unwrap();
        assert!(registration.belongs_to(&budget));
        // Warm source-independent hashing before observing the shared-owner-only empty tree.
        drop(empty_tree(&budget, schema).unwrap());
        observe(&budget);
        let tree = empty_tree(&budget, schema).unwrap();
        OBSERVE.set(false);
        assert_eq!(
            COUNT.load(SeqCst),
            1,
            "empty tree should allocate only its private owner"
        );
        let bytes = budget.reserved_bytes() - observer_bytes;
        assert!(bytes > 0);
        assert_eq!(
            SIZE.load(SeqCst),
            bytes,
            "owner layout calculator matches allocator request"
        );
        assert_eq!(
            RESERVED.load(SeqCst),
            bytes + observer_bytes,
            "credit precedes physical allocation"
        );
        assert!(!FREED.load(SeqCst));
        budget.set_limit_bytes(bytes + observer_bytes);
        let Err(AllocationRefusal::Capacity { release, .. }) = budget.try_reserve_bytes(1) else {
            panic!("empty tree's exact original owner must retain the entire pool")
        };
        let observer = Arc::new(ReentrantAfterFree {
            budget: budget.clone(),
            observer_bytes,
            bytes,
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&observer));
        assert_eq!(
            registration.poll_wait(&release, &mut Context::from_waker(&waker)),
            Poll::Pending
        );
        match owners {
            0 => {
                assert!(
                    catch_unwind(AssertUnwindSafe(move || {
                        let _tree = tree;
                        panic!("exercise final owner cleanup during unwind");
                    }))
                    .is_err()
                );
            }
            1 => drop(tree),
            count => {
                let copies: Vec<_> = (0..count).map(|_| tree.clone()).collect();
                drop(tree);
                assert_eq!(budget.reserved_bytes(), bytes + observer_bytes);
                assert!(!FREED.load(SeqCst));
                let barrier = Barrier::new(count);
                std::thread::scope(|scope| {
                    for copy in copies {
                        let barrier = &barrier;
                        scope.spawn(move || {
                            barrier.wait();
                            drop(copy);
                        });
                    }
                });
            }
        }
        assert!(FREED.load(SeqCst));
        assert_eq!(observer.wakes.load(SeqCst), 1);
        assert_eq!(budget.reserved_bytes(), observer_bytes);
        assert_eq!(
            registration.poll_wait(&release, &mut Context::from_waker(&waker)),
            Poll::Ready(())
        );
        drop(registration);
        assert_eq!(budget.reserved_bytes(), 0);
        BUDGET.with(|slot| *slot.borrow_mut() = None);
    }
}

#[test]
fn empty_tree_refusal_occurs_before_any_physical_owner_allocation() {
    let _serial = SERIAL.lock().unwrap();
    let schema = Hash::new(b"refusal before allocation");
    let budget = AllocationBudget::new(0);
    observe(&budget);
    let result = empty_tree(&budget, schema);
    OBSERVE.set(false);
    assert!(matches!(
        result,
        Err(NoritoKeyRangeError::Admission(
            AllocationRefusal::ExceedsLimit { .. }
        ))
    ));
    assert_eq!(COUNT.load(SeqCst), 0);
    assert_eq!(budget.reserved_bytes(), 0);
    BUDGET.with(|slot| *slot.borrow_mut() = None);
}

fn three_row_tree(
    budget: &AllocationBudget,
    schema: Hash,
) -> Result<NoritoKeyDigestRangeTreeV1, NoritoKeyRangeError> {
    NoritoKeyDigestRangeTreeV1::from_sorted_digests(
        schema,
        b"allocation control",
        [
            (b"a".as_slice(), schema),
            (b"bb".as_slice(), schema),
            (b"ccc".as_slice(), schema),
        ],
        4096,
        budget,
    )
}

#[test]
fn all_nonempty_backings_are_precharged_exactly_and_remain_with_last_borrower() {
    let _serial = SERIAL.lock().unwrap();
    let schema = Hash::new(b"funded padded tree");
    let budget = AllocationBudget::new(65_536);
    drop(three_row_tree(&budget, schema).unwrap());
    observe(&budget);
    let tree = three_row_tree(&budget, schema).unwrap();
    OBSERVE.set(false);
    assert!(!UNDERFUNDED.load(SeqCst), "each request must be precharged");
    assert_eq!(
        COUNT.load(SeqCst),
        9,
        "rows, three keys, outer levels, three levels, owner"
    );
    let bytes = TOTAL_SIZE.load(SeqCst);
    assert_eq!(
        budget.reserved_bytes(),
        bytes,
        "all requested tree allocations are exact"
    );
    let borrower = tree.clone();
    budget.set_limit_bytes(0);
    drop(tree);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(borrower.digest_rows().count(), 3);
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
    BUDGET.with(|slot| *slot.borrow_mut() = None);
}

#[test]
fn every_fallible_backing_allocation_failure_reclaims_original_credit() {
    let _serial = SERIAL.lock().unwrap();
    let schema = Hash::new(b"fallible padded tree");
    let budget = AllocationBudget::new(65_536);
    drop(three_row_tree(&budget, schema).unwrap());
    // Every allocation is fallible, including the ninth, final shared owner.
    // Refusing that owner must reclaim all already prepared keys and levels.
    for ordinal in 1..=9 {
        ATTEMPTS.set(0);
        FAIL_AT.set(Some(ordinal));
        let result = three_row_tree(&budget, schema);
        FAIL_AT.set(None);
        assert_eq!(ATTEMPTS.get(), ordinal);
        assert!(
            matches!(result, Err(NoritoKeyRangeError::Allocation)),
            "charged backing {ordinal} must return its typed allocator failure"
        );
        assert_eq!(budget.reserved_bytes(), 0);
        drop(three_row_tree(&budget, schema).unwrap());
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn empty_shared_owner_allocation_refusal_is_local_and_retries_without_leaking_credit() {
    let _serial = SERIAL.lock().unwrap();
    let schema = Hash::new(b"empty owner physical refusal");
    let budget = AllocationBudget::new(4096);
    ATTEMPTS.set(0);
    FAIL_AT.set(Some(1));
    let result = empty_tree(&budget, schema);
    FAIL_AT.set(None);
    assert_eq!(ATTEMPTS.get(), 1);
    assert!(matches!(result, Err(NoritoKeyRangeError::Allocation)));
    assert_eq!(budget.reserved_bytes(), 0);
    let retry = empty_tree(&budget, schema).unwrap();
    assert!(retry.is_empty());
    assert!(budget.reserved_bytes() > 0);
    drop(retry);
    assert_eq!(budget.reserved_bytes(), 0);
}
