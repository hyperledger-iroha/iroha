//! Explicit payload-cloning admission in the original charged node engine.

use super::allocation_tests::{all_refunded, prepaid, record, Charge, Prepaid};
use super::*;
use crate::internals::bptree::allocation::{NodeCloning, NodeFunding};
use crate::internals::bptree::cursor::{
    CursorMode, CursorRead, CursorReadOps, CursorWrite, SuperBlock,
};
use crate::internals::bptree::tracking::FixedTrackingBuffer;
use crate::internals::lincowcell::LinCowCell;
use std::cell::Cell;
use std::cmp::Ordering;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

struct Payload {
    bytes: ManuallyDrop<Box<[usize; 4]>>,
    charge: ManuallyDrop<Charge>,
    id: usize,
}

impl Payload {
    fn new(number: usize, policy: &mut Policy) -> Self {
        let id = policy.funding.next;
        let charge = policy.take_node_charge(Layout::new::<[usize; 4]>());
        let bytes = Box::new([number; 4]);
        assert_eq!(record(id).pointer, bytes.as_ptr() as usize);
        Self {
            bytes: ManuallyDrop::new(bytes),
            charge: ManuallyDrop::new(charge),
            id,
        }
    }

    fn number(&self) -> usize {
        self.bytes[0]
    }
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        panic!("ordinary payload Clone bypassed original admission")
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        // The allocator observer checks actual System.dealloc, not an owner counter.
        unsafe {
            ManuallyDrop::drop(&mut self.bytes);
            ManuallyDrop::drop(&mut self.charge);
        }
    }
}

impl Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.number().fmt(f)
    }
}
impl Borrow<usize> for Payload {
    fn borrow(&self) -> &usize {
        &self.bytes[0]
    }
}
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.number() == other.number()
    }
}
impl Eq for Payload {}
impl PartialOrd for Payload {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Payload {
    fn cmp(&self, other: &Self) -> Ordering {
        self.number().cmp(&other.number())
    }
}

// These shared cells only observe the test producer after it moves into a cursor.
// They do not supply allocation authority or replace the original prepaid owner.
#[derive(Default)]
struct Observations {
    next: Cell<usize>,
    calls: Cell<usize>,
    fail_at: Cell<usize>,
}
impl Observations {
    fn arm(&self, fail_at: usize) {
        self.calls.set(0);
        self.fail_at.set(fail_at);
    }
}
struct Policy {
    funding: Prepaid,
    observations: Rc<Observations>,
}
impl Policy {
    fn new() -> Self {
        Self {
            funding: prepaid(),
            observations: Rc::default(),
        }
    }
    fn next_operation(observations: &Rc<Observations>) -> Self {
        let next = observations.next.get();
        Self {
            funding: Prepaid {
                next,
                remaining: 128 - next,
            },
            observations: Rc::clone(observations),
        }
    }
    fn copy_payload(&mut self, original: &Payload) -> Payload {
        let call = self.observations.calls.get() + 1;
        self.observations.calls.set(call);
        let cloned = Payload::new(original.number(), self);
        // Inject after the nested allocation so unwind must reclaim that exact
        // allocation as well as any completed node prefix and intermediate key.
        assert_ne!(
            call,
            self.observations.fail_at.get(),
            "admitted payload clone refused"
        );
        cloned
    }
}
impl NodeFunding for Policy {
    type Charge = Charge;
    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        let charge = self.funding.take_allocation_charge(layout);
        self.observations.next.set(self.funding.next);
        charge
    }
}
impl NodeCloning<Payload, Payload> for Policy {
    fn clone_key(&mut self, key: &Payload) -> Payload {
        self.copy_payload(key)
    }
    fn clone_value(&mut self, value: &Payload) -> Payload {
        self.copy_payload(value)
    }
}

type N = Node<Payload, Payload, Charge>;
type L = Leaf<Payload, Payload, Charge>;
type B = Branch<Payload, Payload, Charge>;
struct OwnedNode(*mut N);
impl Drop for OwnedNode {
    fn drop(&mut self) {
        Node::free(self.0);
    }
}
fn leaf(policy: &mut Policy, first: usize, count: usize) -> OwnedNode {
    let owned = OwnedNode(N::new_leaf(1, policy).cast());
    for index in 0..count {
        let number = first + index * 10;
        let key = Payload::new(number, policy);
        let value = Payload::new(number * 3, policy);
        assert!(matches!(
            unsafe { &mut *owned.0.cast::<L>() }.insert_or_update(key, value, policy),
            LeafInsertState::Ok(None)
        ));
    }
    owned
}
fn refunded_since(start: usize, next: usize) {
    for id in start..next {
        assert!(
            record(id).freed && record(id).refunded,
            "allocation {id} not reclaimed"
        );
    }
}
fn finish(observations: &Observations) {
    all_refunded(&Prepaid {
        next: observations.next.get(),
        remaining: 0,
    });
}
fn leaf_snapshot(source: &L) -> Vec<(usize, usize, usize)> {
    (0..source.count())
        .map(|index| unsafe {
            let key = source.key[index].assume_init_ref();
            let value = source.values[index].assume_init_ref();
            (key.number(), key.id, value.id)
        })
        .collect()
}
struct OwnedBranch {
    node: OwnedNode,
    _children: Vec<OwnedNode>,
}
impl OwnedBranch {
    fn new(policy: &mut Policy, first: usize, keys: usize) -> Self {
        let children: Vec<_> = (0..(keys + 1).max(2))
            .map(|index| leaf(policy, first + index * 10, 1))
            .collect();
        let node = OwnedNode(N::new_branch(1, children[0].0, children[1].0, policy).cast());
        let branch = unsafe { &mut *node.0.cast::<B>() };
        if keys == 0 {
            assert_eq!(branch.remove_by_idx(1), children[1].0);
        } else {
            for child in &children[2..] {
                assert!(matches!(
                    branch.add_node(child.0, policy),
                    BranchInsertState::Ok
                ));
            }
        }
        Self {
            node,
            _children: children,
        }
    }
    fn branch(&self) -> &B {
        unsafe { &*self.node.0.cast::<B>() }
    }
    fn branch_mut(&mut self) -> &mut B {
        unsafe { &mut *self.node.0.cast::<B>() }
    }
    fn snapshot(&self) -> (Vec<usize>, Vec<usize>) {
        let branch = self.branch();
        (
            branch.key[..branch.count()]
                .iter()
                .map(|key| unsafe { key.assume_init_ref() }.id)
                .collect(),
            branch.nodes[..=branch.count()]
                .iter()
                .map(|node| *node as usize)
                .collect(),
        )
    }
}

#[test]
fn explicit_leaf_policy_reclaims_every_nested_clone_failure_and_retries_original_source() {
    for fail_at in 1..=2 * L_CAPACITY {
        let mut policy = Policy::new();
        let original = leaf(&mut policy, 0, L_CAPACITY);
        let source = unsafe { &*original.0.cast::<L>() };
        let before = leaf_snapshot(source);
        let baseline = policy.funding.next;
        policy.observations.arm(fail_at);
        assert!(catch_unwind(AssertUnwindSafe(|| source.req_clone(2, &mut policy))).is_err());
        assert_eq!(policy.observations.calls.get(), fail_at);
        refunded_since(baseline, policy.funding.next);
        assert_eq!(leaf_snapshot(source), before);
        assert!(source.verify());
        for (_, key, value) in &before {
            assert!(!record(*key).freed && !record(*value).freed);
        }
        policy.observations.arm(0);
        let cloned = OwnedNode(source.req_clone(2, &mut policy).unwrap());
        let copy = unsafe { &*cloned.0.cast::<L>() };
        for ((number, key, value), (copy_number, copy_key, copy_value)) in
            before.iter().zip(leaf_snapshot(copy))
        {
            assert_eq!(*number, copy_number);
            assert_ne!(*key, copy_key);
            assert_ne!(*value, copy_value);
        }
        assert_eq!(policy.observations.calls.get(), 2 * L_CAPACITY);
        drop(cloned);
        refunded_since(baseline, policy.funding.next);
        drop(original);
        finish(&policy.observations);
    }
}

#[test]
fn explicit_branch_policy_reclaims_partial_separator_clones_and_preserves_children() {
    for fail_at in 1..=L_CAPACITY {
        let mut policy = Policy::new();
        let original = OwnedBranch::new(&mut policy, 0, L_CAPACITY);
        let before = original.snapshot();
        let baseline = policy.funding.next;
        policy.observations.arm(fail_at);
        assert!(catch_unwind(AssertUnwindSafe(|| original
            .branch()
            .req_clone(2, &mut policy)))
        .is_err());
        refunded_since(baseline, policy.funding.next);
        assert_eq!(original.snapshot(), before);
        assert!(N::verify_raw(original.node.0));
        policy.observations.arm(0);
        let cloned = OwnedNode(original.branch().req_clone(2, &mut policy).unwrap());
        let copy = unsafe { &*cloned.0.cast::<B>() };
        assert_eq!(
            &copy.nodes[..=copy.count()],
            &original.branch().nodes[..=copy.count()]
        );
        for (index, original_id) in before.0.iter().enumerate() {
            assert_ne!(
                unsafe { copy.key[index].assume_init_ref() }.id,
                *original_id
            );
        }
        assert_eq!(policy.observations.calls.get(), L_CAPACITY);
        drop(cloned);
        drop(original);
        finish(&policy.observations);
    }
}

#[test]
fn explicit_policy_covers_leaf_splits_and_new_root_separators_in_both_directions() {
    for number in [15, 1000, 0] {
        let mut policy = Policy::new();
        let original = leaf(&mut policy, 10, L_CAPACITY);
        let source = unsafe { &*original.0.cast::<L>() };
        let before = leaf_snapshot(source);
        let cloned = OwnedNode(source.req_clone(2, &mut policy).unwrap());
        let key = Payload::new(number, &mut policy);
        let value = Payload::new(number * 3, &mut policy);
        let result =
            unsafe { &mut *cloned.0.cast::<L>() }.insert_or_update(key, value, &mut policy);
        let (sibling, left, right) = match result {
            LeafInsertState::Split(pointer) => {
                (OwnedNode(pointer.cast()), cloned.0, pointer.cast())
            }
            LeafInsertState::RevSplit(pointer) => {
                (OwnedNode(pointer.cast()), pointer.cast(), cloned.0)
            }
            _ => panic!("full leaf must split"),
        };
        policy.observations.arm(0);
        let root = OwnedNode(N::new_branch(2, left, right, &mut policy).cast());
        assert_eq!(policy.observations.calls.get(), 1);
        assert!(N::verify_raw(root.0));
        assert_eq!(leaf_snapshot(source), before);
        drop(root);
        drop(sibling);
        drop(cloned);
        drop(original);
        finish(&policy.observations);
    }
}

#[test]
fn explicit_policy_split_and_rekey_refusals_preserve_original_separator_allocations() {
    for direction in 0..3 {
        let mut policy = Policy::new();
        let mut original = OwnedBranch::new(&mut policy, 10, L_CAPACITY);
        let sibidx = if direction == 1 { L_CAPACITY - 1 } else { 0 };
        let minimum = if direction == 0 { 15 } else { sibidx * 10 + 5 };
        let inserted = leaf(&mut policy, minimum, 1);
        let before = original.snapshot();
        let baseline = policy.funding.next;
        policy.observations.arm(1);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if direction == 0 {
                original.branch_mut().add_node(inserted.0, &mut policy)
            } else {
                original
                    .branch_mut()
                    .add_node_left(inserted.0, sibidx, &mut policy)
            }
        }))
        .is_err());
        assert_eq!(original.snapshot(), before);
        refunded_since(baseline, policy.funding.next);
        policy.observations.arm(0);
        let split = if direction == 0 {
            original.branch_mut().add_node(inserted.0, &mut policy)
        } else {
            original
                .branch_mut()
                .add_node_left(inserted.0, sibidx, &mut policy)
        };
        let BranchInsertState::Split(left, right) = split else {
            panic!("expected original split children")
        };
        assert_eq!(policy.observations.calls.get(), 1);
        let sibling = OwnedNode(N::new_branch(1, left, right, &mut policy).cast());
        let root = OwnedNode(N::new_branch(1, original.node.0, sibling.0, &mut policy).cast());
        assert!(N::verify_raw(root.0));
        let rekey_before = original.snapshot();
        policy.observations.arm(1);
        let baseline = policy.funding.next;
        assert!(catch_unwind(AssertUnwindSafe(|| original
            .branch_mut()
            .rekey_by_idx(1, &mut policy)))
        .is_err());
        assert_eq!(original.snapshot(), rekey_before);
        refunded_since(baseline, policy.funding.next);
        policy.observations.arm(0);
        original.branch_mut().rekey_by_idx(1, &mut policy);
        assert!(record(rekey_before.0[0]).freed && record(rekey_before.0[0]).refunded);
        drop(root);
        drop(sibling);
        drop(original);
        drop(inserted);
        finish(&policy.observations);
    }
}

#[test]
fn explicit_policy_preserves_both_original_branches_on_merge_and_transfer_refusal() {
    for operation in 0..4 {
        let mut policy = Policy::new();
        let left_full = operation % 2 == 0;
        let count = if operation < 2 { 2 } else { L_CAPACITY };
        let mut left = OwnedBranch::new(&mut policy, 0, if left_full { count } else { 0 });
        let mut right = OwnedBranch::new(&mut policy, 1000, if left_full { 0 } else { count });
        let before_left = left.snapshot();
        let before_right = right.snapshot();
        let baseline = policy.funding.next;
        let apply = |left: &mut OwnedBranch, right: &mut OwnedBranch, policy: &mut Policy| {
            match operation {
                0 | 1 => left.branch_mut().merge(right.branch_mut(), policy),
                2 => left
                    .branch_mut()
                    .take_from_l_to_r(right.branch_mut(), policy),
                _ => left
                    .branch_mut()
                    .take_from_r_to_l(right.branch_mut(), policy),
            }
        };
        policy.observations.arm(1);
        assert!(catch_unwind(AssertUnwindSafe(|| apply(
            &mut left,
            &mut right,
            &mut policy
        )))
        .is_err());
        assert_eq!(left.snapshot(), before_left);
        assert_eq!(right.snapshot(), before_right);
        refunded_since(baseline, policy.funding.next);
        policy.observations.arm(0);
        apply(&mut left, &mut right, &mut policy);
        assert_eq!(policy.observations.calls.get(), 1);
        assert!(N::verify_raw(left.node.0));
        if operation >= 2 {
            assert!(N::verify_raw(right.node.0));
        }
        let mut before_children = before_left.1;
        before_children.extend(before_right.1);
        let mut after_children = left.snapshot().1;
        if operation >= 2 {
            after_children.extend(right.snapshot().1);
        }
        assert_eq!(after_children, before_children);
        drop(left);
        drop(right);
        finish(&policy.observations);
    }
}

type Buffer = FixedTrackingBuffer<*mut N, Charge>;
impl CursorMode<Payload, Payload> for Policy {
    type Buffer = Buffer;
    type Input = (Self, Buffer, Buffer);
    fn into_parts(input: Self::Input) -> Self::Input {
        input
    }
}
type MapCell = LinCowCell<
    SuperBlock<Payload, Payload, Policy>,
    CursorRead<Payload, Payload, Policy>,
    CursorWrite<Payload, Payload, Policy>,
>;
fn input(
    mut policy: Policy,
    first: usize,
    last: usize,
) -> <Policy as CursorMode<Payload, Payload>>::Input {
    let first_charge = policy.take_node_charge(Buffer::allocation_layout(first).unwrap());
    let first =
        Buffer::try_new(first, first_charge).unwrap_or_else(|_| panic!("valid first layout"));
    let last_charge = policy.take_node_charge(Buffer::allocation_layout(last).unwrap());
    let last = Buffer::try_new(last, last_charge).unwrap_or_else(|_| panic!("valid last layout"));
    (policy, first, last)
}

#[test]
fn original_cursor_preflight_returns_same_nested_payloads_without_invoking_clone_policy() {
    let mut policy = Policy::new();
    let observations = Rc::clone(&policy.observations);
    let root = unsafe { SuperBlock::new_with_funding(&mut policy) };
    let cell: MapCell = LinCowCell::new(root);
    let key = Payload::new(7, &mut policy);
    let value = Payload::new(21, &mut policy);
    let ids = (key.id, value.id);
    let mut writer = cell.write_with(|_| input(policy, 0, 1));
    let before = observations.next.get();
    let (key, value) = writer.try_insert(key, value).unwrap_err();
    assert_eq!((key.id, value.id), ids);
    assert_eq!(observations.calls.get(), 0);
    assert_eq!(observations.next.get(), before);
    assert!(writer.search(&7).is_none());
    drop(writer);
    drop((key, value));
    drop(cell);
    finish(&observations);
}

#[test]
fn original_cursor_clone_unwind_reclaims_nested_allocations_and_retains_published_values() {
    let mut policy = Policy::new();
    let observations = Rc::clone(&policy.observations);
    let root = unsafe { SuperBlock::new_with_funding(&mut policy) };
    let cell: MapCell = LinCowCell::new(root);
    // Inputs are funded before they enter the original writer; ordinary Clone
    // remains a hard failure even when the existing cursor grows its root.
    let entries: Vec<_> = (0..L_CAPACITY + 2)
        .map(|key| {
            (
                Payload::new(key * 10, &mut policy),
                Payload::new(key * 30, &mut policy),
            )
        })
        .collect();
    let mut writer = cell.write_with(|_| input(policy, 32, 32));
    for (key, value) in entries {
        assert!(writer.try_insert(key, value).unwrap().is_none());
    }
    writer.commit();
    let original = cell.read();
    let held = original.search(&0).unwrap();
    let original_id = held.id;
    let baseline = observations.next.get();
    observations.arm(2);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let mut next = Policy::next_operation(&observations);
        let key = Payload::new(5, &mut next);
        let value = Payload::new(15, &mut next);
        let mut writer = cell.write_with(|_| input(next, 32, 32));
        writer.try_insert(key, value).unwrap();
    }))
    .is_err());
    assert_eq!(observations.calls.get(), 2);
    refunded_since(baseline, observations.next.get());
    assert_eq!(original.search(&0).unwrap().id, original_id);
    assert_eq!(held.number(), 0);
    assert!(!record(original_id).freed);
    assert!(original.search(&5).is_none());
    assert!(cell.is_poisoned());
    drop(original);
    drop(cell);
    finish(&observations);
}
