//! Actual padded node storage, original prepaid charges and unwind reclamation.

use super::*;
use std::alloc::{GlobalAlloc, System};
use std::cell::Cell;
use std::cmp::Ordering;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Clone, Copy)]
struct Record {
    pointer: usize,
    layout: Layout,
    freed: bool,
    refunded: bool,
}

thread_local! {
    static RECORDS: Cell<[Option<Record>; 128]> = const { Cell::new([None; 128]) };
    static EXPECTED: Cell<Option<(usize, Layout)>> = const { Cell::new(None) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static CLONES: Cell<usize> = const { Cell::new(0) };
    static PANIC_CLONE: Cell<usize> = const { Cell::new(0) };
    static PANIC_COMPARE: Cell<bool> = const { Cell::new(false) };
    static PANIC_DROP: Cell<bool> = const { Cell::new(false) };
}

struct ObservedAllocator;

unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let _ = EXPECTED.try_with(|expected| {
                if let Some((id, exact)) = expected.get().filter(|(_, exact)| *exact == layout) {
                    expected.set(None);
                    let _ = RECORDS.try_with(|records| {
                        let mut all = records.get();
                        all[id] = Some(Record {
                            pointer: pointer as usize,
                            layout: exact,
                            freed: false,
                            refunded: false,
                        });
                        records.set(all);
                    });
                }
            });
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        let _ = RECORDS.try_with(|records| {
            let mut all = records.get();
            for record in all.iter_mut().flatten() {
                if record.pointer == pointer as usize && record.layout == layout && !record.freed {
                    record.freed = true;
                }
            }
            records.set(all);
        });
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

// Intentionally neither Clone nor Default. Its size also changes real padding.
struct Charge {
    id: usize,
    layout: Layout,
    _storage: [u8; 129],
}

impl Drop for Charge {
    fn drop(&mut self) {
        RECORDS.with(|records| {
            let mut all = records.get();
            let record = all[self.id]
                .as_mut()
                .expect("the charged node was allocated");
            assert_eq!(record.layout, self.layout);
            assert!(record.freed, "node credits returned before System.dealloc");
            assert!(!record.refunded, "original charge returned twice");
            record.refunded = true;
            records.set(all);
        });
    }
}

struct Prepaid {
    next: usize,
    // A finite admitted count suffices for this allocator-observation fixture;
    // production must prepay the complete exact layout sum before mutation.
    remaining: usize,
}

impl NodeFunding for Prepaid {
    type Charge = Charge;

    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        assert!(self.remaining > 0, "operation exceeded original admission");
        self.remaining -= 1;
        let id = self.next;
        self.next += 1;
        EXPECTED.with(|expected| assert!(expected.replace(Some((id, layout))).is_none()));
        Charge {
            id,
            layout,
            _storage: [0; 129],
        }
    }
}

fn prepaid() -> Prepaid {
    assert_eq!(LIVE.with(Cell::get), 0);
    RECORDS.with(|records| records.set([None; 128]));
    EXPECTED.with(|expected| expected.set(None));
    CLONES.with(|count| count.set(0));
    PANIC_CLONE.with(|at| at.set(0));
    PANIC_COMPARE.with(|value| value.set(false));
    PANIC_DROP.with(|value| value.set(false));
    Prepaid {
        next: 0,
        remaining: 128,
    }
}

fn record(id: usize) -> Record {
    RECORDS.with(|records| records.get()[id].expect("original allocation record"))
}

fn all_refunded(funding: &Prepaid) {
    assert!(EXPECTED.with(Cell::get).is_none());
    for id in 0..funding.next {
        assert!(record(id).refunded);
    }
    assert_eq!(LIVE.with(Cell::get), 0);
    assert_released();
}

#[derive(Debug)]
struct Probe(usize);

impl Probe {
    fn new(key: usize) -> Self {
        LIVE.with(|live| live.set(live.get() + 1));
        Self(key)
    }
}

impl Clone for Probe {
    fn clone(&self) -> Self {
        let attempt = CLONES.with(|count| {
            let n = count.get() + 1;
            count.set(n);
            n
        });
        assert_ne!(
            attempt,
            PANIC_CLONE.with(Cell::get),
            "injected clone failure"
        );
        Self::new(self.0)
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        LIVE.with(|live| live.set(live.get() - 1));
        assert!(
            !PANIC_DROP.with(|value| value.replace(false)),
            "injected destructor failure"
        );
    }
}

impl PartialEq for Probe {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Probe {}
impl PartialOrd for Probe {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Probe {
    fn cmp(&self, other: &Self) -> Ordering {
        assert!(
            !PANIC_COMPARE.with(Cell::get),
            "injected comparison failure"
        );
        self.0.cmp(&other.0)
    }
}

type ChargedNode = Node<Probe, Probe, Charge>;
type ChargedLeaf = Leaf<Probe, Probe, Charge>;
type ChargedBranch = Branch<Probe, Probe, Charge>;

struct Owner(*mut ChargedNode);
impl Drop for Owner {
    fn drop(&mut self) {
        ChargedNode::free(self.0);
    }
}

fn leaf(funding: &mut Prepaid, start: usize, count: usize) -> Owner {
    let owner = Owner(ChargedNode::new_leaf(1, funding).cast());
    let leaf = unsafe { &mut *owner.0.cast::<ChargedLeaf>() };
    for key in start..start + count {
        assert!(matches!(
            leaf.insert_or_update(Probe::new(key), Probe::new(key), funding),
            LeafInsertState::Ok(None)
        ));
    }
    owner
}

#[test]
fn charged_leaf_branch_and_final_tree_refund_only_after_exact_padded_free() {
    let mut funding = prepaid();
    let left = leaf(&mut funding, 0, 1);
    let right = leaf(&mut funding, 10, 1);
    let root = ChargedNode::new_branch(1, left.0, right.0, &mut funding).cast();
    assert_eq!(record(0).layout, Layout::new::<CachePadded<ChargedLeaf>>());
    assert_eq!(
        record(2).layout,
        Layout::new::<CachePadded<ChargedBranch>>()
    );
    assert!(record(0).layout.size() > Layout::new::<CachePadded<Leaf<Probe, Probe>>>().size());
    assert!(record(2).layout.size() > Layout::new::<CachePadded<Branch<Probe, Probe>>>().size());
    assert_eq!(record(0).pointer, left.0 as usize);
    assert_eq!(record(2).pointer, root as usize);
    std::mem::forget(left);
    std::mem::forget(right);
    unsafe { ChargedNode::free_tree(root) };
    all_refunded(&funding);
}

#[test]
fn every_charged_leaf_clone_panic_refunds_only_its_actual_new_allocation() {
    let mut funding = prepaid();
    let source = leaf(&mut funding, 0, L_CAPACITY);
    let original = unsafe { &*source.0.cast::<ChargedLeaf>() };
    for at in 1..=2 * L_CAPACITY {
        CLONES.with(|count| count.set(0));
        PANIC_CLONE.with(|value| value.set(at));
        let id = funding.next;
        assert!(catch_unwind(AssertUnwindSafe(|| original.req_clone(2, &mut funding))).is_err());
        assert!(record(id).refunded);
        assert!(!record(0).freed);
        assert_eq!(LIVE.with(Cell::get), 2 * L_CAPACITY);
    }
    PANIC_CLONE.with(|value| value.set(0));
    drop(Owner(original.req_clone(2, &mut funding).unwrap()));
    assert!(original.req_clone(1, &mut funding).is_none());
    drop(source);
    all_refunded(&funding);
}

#[test]
fn every_charged_branch_clone_panic_preserves_children_and_original_charge() {
    let mut funding = prepaid();
    let children: Vec<_> = (0..BV_CAPACITY)
        .map(|i| leaf(&mut funding, i * 10, 1))
        .collect();
    let root = Owner(ChargedNode::new_branch(1, children[0].0, children[1].0, &mut funding).cast());
    let original = unsafe { &mut *root.0.cast::<ChargedBranch>() };
    for child in &children[2..] {
        assert!(matches!(original.add_node(child.0), BranchInsertState::Ok));
    }
    for at in 1..=L_CAPACITY {
        CLONES.with(|count| count.set(0));
        PANIC_CLONE.with(|value| value.set(at));
        let id = funding.next;
        assert!(catch_unwind(AssertUnwindSafe(|| original.req_clone(2, &mut funding))).is_err());
        assert!(record(id).refunded);
        assert_eq!(LIVE.with(Cell::get), 2 * BV_CAPACITY + L_CAPACITY);
    }
    PANIC_CLONE.with(|value| value.set(0));
    drop(Owner(original.req_clone(2, &mut funding).unwrap()));
    let count = funding.next;
    assert!(original.req_clone(1, &mut funding).is_none());
    assert_eq!(funding.next, count);
    drop(root);
    drop(children);
    all_refunded(&funding);
}

#[test]
fn charged_leaf_split_directions_keep_both_original_allocation_owners() {
    for key in [0, 11, 100] {
        let mut funding = prepaid();
        let source = leaf(&mut funding, 10, L_CAPACITY);
        let current = unsafe { &mut *source.0.cast::<ChargedLeaf>() };
        // The middle case inserts a new odd key into an even-key full leaf.
        for index in 0..L_CAPACITY {
            unsafe {
                current.key[index].assume_init_mut().0 = 10 + index * 2;
            }
        }
        let split = match current.insert_or_update(Probe::new(key), Probe::new(key), &mut funding) {
            LeafInsertState::Split(node) | LeafInsertState::RevSplit(node) => Owner(node.cast()),
            LeafInsertState::Ok(_) => panic!("full leaf did not split"),
        };
        assert_eq!(funding.next, 2);
        assert_eq!(LIVE.with(Cell::get), 2 * (L_CAPACITY + 1));
        drop(source);
        assert!(record(0).refunded);
        assert!(!record(1).freed);
        drop(split);
        all_refunded(&funding);
    }
}

#[test]
fn charged_branch_constructor_clone_failure_reclaims_its_empty_owner() {
    let mut funding = prepaid();
    let left = leaf(&mut funding, 0, 1);
    let right = leaf(&mut funding, 10, 1);
    CLONES.with(|count| count.set(0));
    PANIC_CLONE.with(|value| value.set(1));
    assert!(catch_unwind(AssertUnwindSafe(|| ChargedNode::new_branch(
        1,
        left.0,
        right.0,
        &mut funding
    )))
    .is_err());
    assert!(record(2).refunded);
    assert_eq!(LIVE.with(Cell::get), 4);
    drop(left);
    drop(right);
    all_refunded(&funding);
}

#[test]
fn charged_payload_destructor_failure_does_not_prematurely_return_credit() {
    let mut funding = prepaid();
    let left = leaf(&mut funding, 0, 1);
    let right = leaf(&mut funding, 10, 1);
    let root = Owner(ChargedNode::new_branch(1, left.0, right.0, &mut funding).cast());
    PANIC_DROP.with(|value| value.set(true));
    assert!(catch_unwind(AssertUnwindSafe(|| drop(root))).is_err());
    assert!(record(2).freed);
    assert!(!record(2).refunded);
    drop(left);
    drop(right);
    assert!(record(0).refunded && record(1).refunded);
    assert_eq!(LIVE.with(Cell::get), 0);
    assert_released();
}

#[test]
#[cfg(debug_assertions)]
fn charged_branch_verification_panic_frees_original_box_before_refund() {
    let mut funding = prepaid();
    let left = leaf(&mut funding, 0, 1);
    let right = leaf(&mut funding, 10, 1);
    PANIC_COMPARE.with(|value| value.set(true));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        ChargedNode::new_branch(1, left.0, right.0, &mut funding)
    }))
    .is_err());
    PANIC_COMPARE.with(|value| value.set(false));
    assert!(record(2).refunded);
    assert_eq!(LIVE.with(Cell::get), 4);
    drop(left);
    drop(right);
    all_refunded(&funding);
}

#[test]
fn raw_node_thread_traits_require_original_key_value_and_charge_safety() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Node<u64, u64, Untracked>>();

    // Ambiguous inference would fail compilation if a second Sync/Send impl
    // applies. Cell keys are Send and Ord, but cannot be shared across readers.
    trait AmbiguousIfSync<A> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfSync<()> for T {}
    impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
    trait AmbiguousIfSend<A> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
    fn key_bounds<T: Clone + Ord + Debug + Send + 'static>() {}
    key_bounds::<Cell<u64>>();
    let _ = <Node<Cell<u64>, u64> as AmbiguousIfSync<_>>::probe;
    let _ = <Node<u64, u64, std::rc::Rc<()>> as AmbiguousIfSend<_>>::probe;
    let _ = <Node<u64, u64, Cell<u64>> as AmbiguousIfSync<_>>::probe;
}
