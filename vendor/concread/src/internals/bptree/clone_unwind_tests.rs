//! Original node and nested payload custody across clone unwinding.

use super::*;
use crate::bptree::BptreeMap;
use std::cmp::Ordering as CmpOrdering;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Payloads {
    attempts: AtomicUsize,
    panic_at: AtomicUsize,
    compare_panic: AtomicBool,
    comparisons: AtomicUsize,
    drops: Mutex<Vec<usize>>,
}

impl Payloads {
    fn arm(&self, panic_at: usize) {
        self.attempts.store(0, Ordering::SeqCst);
        self.panic_at.store(panic_at, Ordering::SeqCst);
    }

    fn live(&self) -> Vec<usize> {
        self.drops
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(id, drops)| (*drops == 0).then_some(id))
            .collect()
    }

    fn allocated(&self) -> usize {
        self.drops.lock().unwrap().len()
    }

    fn assert_reclaimed_since(&self, first: usize) {
        let drops = self.drops.lock().unwrap()[first..].to_vec();
        assert!(drops.iter().all(|drops| *drops == 1));
    }

    fn assert_all_reclaimed(&self) {
        self.assert_reclaimed_since(0);
        assert!(self.live().is_empty());
    }
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    Key,
    Value,
}

struct Payload {
    id: usize,
    kind: Kind,
    // Each completed clone owns a distinct nested allocation. Drop records are
    // checked after the complete unwind, including this box's normal drop.
    bytes: Box<[usize; 4]>,
    owners: Arc<Payloads>,
}

impl Payload {
    fn new(number: usize, kind: Kind, owners: &Arc<Payloads>) -> Self {
        let bytes = Box::new([number; 4]);
        let mut drops = owners.drops.lock().unwrap();
        let id = drops.len();
        drops.push(0);
        Self {
            id,
            kind,
            bytes,
            owners: Arc::clone(owners),
        }
    }
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        let attempt = self.owners.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        // Never inject a panic while holding a bookkeeping lock or in Drop.
        assert_ne!(
            attempt,
            self.owners.panic_at.load(Ordering::SeqCst),
            "injected {:?} clone panic at {attempt}",
            self.kind,
        );
        let mut cloned = Self::new(self.bytes[0], self.kind, &self.owners);
        cloned.bytes.copy_from_slice(self.bytes.as_ref());
        cloned
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.owners.drops.lock().unwrap()[self.id] += 1;
    }
}

impl Debug for Payload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Payload")
            .field(&self.bytes[0])
            .finish()
    }
}

impl Borrow<usize> for Payload {
    fn borrow(&self) -> &usize {
        &self.bytes[0]
    }
}

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.bytes[0] == other.bytes[0]
    }
}

impl Eq for Payload {}

impl PartialOrd for Payload {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for Payload {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.owners.comparisons.fetch_add(1, Ordering::SeqCst);
        assert!(
            !self.owners.compare_panic.load(Ordering::SeqCst),
            "injected key comparison panic",
        );
        self.bytes[0].cmp(&other.bytes[0])
    }
}

struct OwnedNode(*mut Node<Payload, Payload>);

impl Drop for OwnedNode {
    fn drop(&mut self) {
        Node::free(self.0);
    }
}

fn leaf(owners: &Arc<Payloads>, start: usize, count: usize) -> OwnedNode {
    let node = OwnedNode(Node::<Payload, Payload>::new_leaf(1, &mut Untracked).cast());
    let leaf = unsafe { &mut *node.0.cast::<Leaf<Payload, Payload>>() };
    for number in start..start + count {
        assert!(matches!(
            leaf.insert_or_update(
                Payload::new(number, Kind::Key, owners),
                Payload::new(number, Kind::Value, owners),
                &mut Untracked
            ),
            LeafInsertState::Ok(None)
        ));
    }
    node
}

fn node_ids() -> Vec<usize> {
    #[cfg(not(miri))]
    {
        ALLOC_LIST.with(|ids| ids.lock().unwrap().iter().copied().collect())
    }
    #[cfg(miri)]
    {
        Vec::new()
    }
}

#[test]
fn every_leaf_key_and_value_clone_unwind_reclaims_only_completed_clones() {
    let owners = Arc::new(Payloads::default());
    let original = leaf(&owners, 0, L_CAPACITY);
    let source = unsafe { &*original.0.cast::<Leaf<Payload, Payload>>() };
    let original_payloads = owners.live();
    let original_nodes = node_ids();

    for panic_at in 1..=2 * L_CAPACITY {
        let first_clone = owners.allocated();
        owners.arm(panic_at);
        assert!(catch_unwind(AssertUnwindSafe(|| source.req_clone(2, &mut Untracked))).is_err());
        assert_eq!(owners.attempts.load(Ordering::SeqCst), panic_at);
        assert_eq!(owners.live(), original_payloads);
        owners.assert_reclaimed_since(first_clone);
        assert_eq!(node_ids(), original_nodes);
        assert_eq!(source.count(), L_CAPACITY);
        assert!(source.verify());
        for key in 0..L_CAPACITY {
            assert_eq!(source.get_ref(&key).unwrap().bytes[0], key);
        }

        owners.arm(0);
        let retry = OwnedNode(source.req_clone(2, &mut Untracked).unwrap());
        let cloned = unsafe { &*retry.0.cast::<Leaf<Payload, Payload>>() };
        assert_eq!(cloned.count(), source.count());
        assert!(cloned.verify());
        for index in 0..L_CAPACITY {
            let source_key = unsafe { source.key[index].assume_init_ref() };
            let cloned_key = unsafe { cloned.key[index].assume_init_ref() };
            let source_value = source.get_ref(&index).unwrap();
            let cloned_value = cloned.get_ref(&index).unwrap();
            assert_ne!(source_key.id, cloned_key.id);
            assert_ne!(source_value.id, cloned_value.id);
            assert_eq!(source_value.bytes, cloned_value.bytes);
        }
        drop(retry);
        assert_eq!(owners.live(), original_payloads);
        owners.assert_reclaimed_since(first_clone);
        assert_eq!(node_ids(), original_nodes);
    }

    drop(original);
    owners.assert_all_reclaimed();
    assert_released();
}

#[test]
fn every_branch_key_clone_unwind_preserves_original_children_and_separator_keys() {
    let owners = Arc::new(Payloads::default());
    let children: Vec<_> = (0..BV_CAPACITY)
        .map(|number| leaf(&owners, number, 1))
        .collect();
    let original =
        OwnedNode(Node::new_branch(1, children[0].0, children[1].0, &mut Untracked).cast());
    let source = unsafe { &mut *original.0.cast::<Branch<Payload, Payload>>() };
    for child in &children[2..] {
        assert!(matches!(
            source.add_node(child.0, &mut Untracked),
            BranchInsertState::Ok
        ));
    }
    assert_eq!(source.count(), L_CAPACITY);
    let original_payloads = owners.live();
    let original_nodes = node_ids();

    for panic_at in 1..=L_CAPACITY {
        let first_clone = owners.allocated();
        owners.arm(panic_at);
        assert!(catch_unwind(AssertUnwindSafe(|| source.req_clone(2, &mut Untracked))).is_err());
        assert_eq!(owners.attempts.load(Ordering::SeqCst), panic_at);
        assert_eq!(owners.live(), original_payloads);
        owners.assert_reclaimed_since(first_clone);
        assert_eq!(node_ids(), original_nodes);
        assert!(Node::verify_raw(original.0));
        assert_eq!(source.count(), L_CAPACITY);
        for (index, child) in children.iter().enumerate() {
            assert_eq!(source.nodes[index], child.0);
            let leaf = unsafe { &*child.0.cast::<Leaf<Payload, Payload>>() };
            assert_eq!(leaf.get_ref(&index).unwrap().bytes[0], index);
        }

        owners.arm(0);
        let retry = OwnedNode(source.req_clone(2, &mut Untracked).unwrap());
        let cloned = unsafe { &*retry.0.cast::<Branch<Payload, Payload>>() };
        assert_eq!(cloned.count(), source.count());
        assert_eq!(cloned.nodes, source.nodes);
        assert!(Node::verify_raw(retry.0));
        for index in 0..L_CAPACITY {
            let source_key = unsafe { source.key[index].assume_init_ref() };
            let cloned_key = unsafe { cloned.key[index].assume_init_ref() };
            assert_ne!(source_key.id, cloned_key.id);
            assert_eq!(source_key.bytes, cloned_key.bytes);
        }
        drop(retry);
        assert_eq!(owners.live(), original_payloads);
        owners.assert_reclaimed_since(first_clone);
        assert_eq!(node_ids(), original_nodes);
    }

    drop(original);
    drop(children);
    owners.assert_all_reclaimed();
    assert_released();
}

fn populated_map(owners: &Arc<Payloads>) -> BptreeMap<Payload, Payload> {
    map_with_entries(owners, 3 * L_CAPACITY)
}

fn map_with_entries(owners: &Arc<Payloads>, entries: usize) -> BptreeMap<Payload, Payload> {
    let map = BptreeMap::new();
    let mut writer = map.write();
    for number in 0..entries {
        writer.insert(
            Payload::new(number, Kind::Key, owners),
            Payload::new(number, Kind::Value, owners),
        );
    }
    writer.commit();
    map
}

#[test]
fn map_writer_clone_unwind_reclaims_completed_path_without_unpinning_original_reader() {
    let owners = Arc::new(Payloads::default());
    let map = populated_map(&owners);
    let reader = map.read();
    let key = reader.first_key_value().unwrap().0;
    let original_payloads = owners.live();
    let original_nodes = node_ids();
    let first_clone = owners.allocated();

    // get_mut clones the leaf before its parent. Measure the actual complete
    // path, abort normally, then panic on its last separator clone, after the
    // completed leaf is already retained in the writer's first_seen list.
    owners.arm(0);
    {
        let mut writer = map.write();
        assert!(writer.get_mut(key).is_some());
    }
    let path_clones = owners.attempts.load(Ordering::SeqCst);
    assert!(path_clones > 2 * L_CAPACITY);
    assert_eq!(owners.live(), original_payloads);
    owners.assert_reclaimed_since(first_clone);
    assert_eq!(node_ids(), original_nodes);

    owners.arm(path_clones);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let mut writer = map.write();
        writer.get_mut(key);
    }))
    .is_err());
    assert!(map.is_poisoned());
    assert_eq!(owners.attempts.load(Ordering::SeqCst), path_clones);
    assert_eq!(owners.live(), original_payloads);
    owners.assert_reclaimed_since(first_clone);
    assert_eq!(node_ids(), original_nodes);
    let after = map.read();
    for number in 0..3 * L_CAPACITY {
        assert_eq!(reader.get(&number).unwrap().bytes[0], number);
        assert_eq!(after.get(&number).unwrap().bytes[0], number);
    }
    drop(after);
    drop(reader);
    drop(map);
    owners.assert_all_reclaimed();
    assert_released();
}

#[test]
fn detached_writer_abort_and_retained_reader_chain_release_original_nested_allocations() {
    let owners = Arc::new(Payloads::default());
    let map = populated_map(&owners);
    let oldest = map.read();
    let key = oldest.first_key_value().unwrap().0;
    let original_payloads = owners.live();
    let original_nodes = node_ids();
    let first_clone = owners.allocated();
    let mut writer = map.write();
    writer.get_mut(key).unwrap().bytes[0] = 100;
    let detached = writer.detach();
    assert_eq!(detached.get(&0).unwrap().bytes[0], 100);
    assert_eq!(oldest.get(&0).unwrap().bytes[0], 0);
    assert!(owners.live().len() > original_payloads.len());
    drop(detached);
    assert_eq!(owners.live(), original_payloads);
    owners.assert_reclaimed_since(first_clone);
    assert_eq!(node_ids(), original_nodes);

    let mut writer = map.write();
    writer.get_mut(key).unwrap().bytes[0] = 100;
    writer.commit();
    let middle = map.read();
    let mut writer = map.write();
    writer.get_mut(key).unwrap().bytes[0] = 200;
    writer.commit();
    let newest = map.read();
    assert_eq!(oldest.get(&0).unwrap().bytes[0], 0);
    assert_eq!(middle.get(&0).unwrap().bytes[0], 100);
    assert_eq!(newest.get(&0).unwrap().bytes[0], 200);
    let chain_payloads = owners.live();
    let chain_nodes = node_ids();
    drop(middle);
    assert_eq!(owners.live(), chain_payloads);
    assert_eq!(node_ids(), chain_nodes);
    // The oldest reader, through the real successor chain, retained the
    // intermediate generation even after its direct reader was gone.
    drop(oldest);
    assert!(owners.live().len() < chain_payloads.len());
    #[cfg(not(miri))]
    assert!(node_ids().len() < chain_nodes.len());
    assert_eq!(newest.get(&0).unwrap().bytes[0], 200);
    drop(newest);
    drop(map);
    owners.assert_all_reclaimed();
    assert_released();
}

#[test]
fn replacing_initialized_separator_reclaims_original_key_after_clone_succeeds() {
    let owners = Arc::new(Payloads::default());
    let left = leaf(&owners, 0, 1);
    let right = leaf(&owners, 10, 1);
    let original = OwnedNode(Node::new_branch(1, left.0, right.0, &mut Untracked).cast());
    let source = unsafe { &mut *original.0.cast::<Branch<Payload, Payload>>() };
    let old_key = unsafe { source.key[0].assume_init_ref() }.id;
    let original_payloads = owners.live();
    owners.arm(1);
    assert!(catch_unwind(AssertUnwindSafe(|| source.rekey_by_idx(1, &mut Untracked))).is_err());
    assert_eq!(owners.live(), original_payloads);
    assert_eq!(unsafe { source.key[0].assume_init_ref() }.id, old_key);
    owners.arm(0);
    source.rekey_by_idx(1, &mut Untracked);
    let old_key_drops = owners.drops.lock().unwrap()[old_key];
    assert_eq!(old_key_drops, 1);
    assert_ne!(unsafe { source.key[0].assume_init_ref() }.id, old_key);
    assert!(Node::verify_raw(original.0));
    drop(original);
    drop(left);
    drop(right);
    owners.assert_all_reclaimed();
    assert_released();
}

struct OwnedBranch {
    node: OwnedNode,
    // Branch destruction owns only its separator keys; these original leaves
    // stay alive independently, including when a split changes their parent.
    _children: Vec<OwnedNode>,
}

impl OwnedBranch {
    fn new(owners: &Arc<Payloads>, base: usize, keys: usize) -> Self {
        let children: Vec<_> = (0..(keys + 1).max(2))
            .map(|number| leaf(owners, base + number * 10, 1))
            .collect();
        let node =
            OwnedNode(Node::new_branch(1, children[0].0, children[1].0, &mut Untracked).cast());
        let branch = unsafe { &mut *node.0.cast::<Branch<Payload, Payload>>() };
        if keys == 0 {
            assert_eq!(branch.remove_by_idx(1), children[1].0);
        } else {
            for child in &children[2..] {
                assert!(matches!(
                    branch.add_node(child.0, &mut Untracked),
                    BranchInsertState::Ok
                ));
            }
        }
        Self {
            node,
            _children: children,
        }
    }

    fn branch(&self) -> &Branch<Payload, Payload> {
        unsafe { &*self.node.0.cast::<Branch<Payload, Payload>>() }
    }

    fn branch_mut(&mut self) -> &mut Branch<Payload, Payload> {
        unsafe { &mut *self.node.0.cast::<Branch<Payload, Payload>>() }
    }

    fn snapshot(&self) -> (Vec<usize>, Vec<usize>) {
        let branch = self.branch();
        let keys = branch.key[..branch.count()]
            .iter()
            .map(|key| unsafe { key.assume_init_ref() }.id)
            .collect();
        let children = branch.nodes[..=branch.count()]
            .iter()
            .map(|node| *node as usize)
            .collect();
        (keys, children)
    }
}

#[test]
fn full_branch_split_clone_refusal_preserves_original_separator_and_child_owners() {
    for direction in 0..3 {
        let owners = Arc::new(Payloads::default());
        let mut original = OwnedBranch::new(&owners, 10, L_CAPACITY);
        let sibidx = if direction == 1 { L_CAPACITY - 1 } else { 0 };
        let minimum = if direction == 0 { 15 } else { sibidx * 10 + 5 };
        let inserted = leaf(&owners, minimum, 1);
        let before = original.snapshot();
        let original_payloads = owners.live();
        let original_nodes = node_ids();
        owners.arm(1);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if direction == 0 {
                original.branch_mut().add_node(inserted.0, &mut Untracked)
            } else {
                original
                    .branch_mut()
                    .add_node_left(inserted.0, sibidx, &mut Untracked)
            }
        }))
        .is_err());
        assert_eq!(original.snapshot(), before);
        assert_eq!(owners.live(), original_payloads);
        assert_eq!(node_ids(), original_nodes);

        owners.arm(0);
        let result = if direction == 0 {
            original.branch_mut().add_node(inserted.0, &mut Untracked)
        } else {
            original
                .branch_mut()
                .add_node_left(inserted.0, sibidx, &mut Untracked)
        };
        let BranchInsertState::Split(left, right) = result else {
            panic!("a full branch must return its actual split children");
        };
        assert_eq!(owners.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(original.branch().count(), L_CAPACITY - 1);
        assert!(Node::verify_raw(original.node.0));
        let mut before_children = before.1;
        before_children.push(inserted.0 as usize);
        before_children.sort_unstable();
        let mut after_children = original.snapshot().1;
        after_children.extend([left as usize, right as usize]);
        after_children.sort_unstable();
        assert_eq!(after_children, before_children);
        drop(original);
        drop(inserted);
        owners.assert_all_reclaimed();
        assert_released();
    }
}

#[test]
fn branch_merge_clone_refusal_preserves_both_original_initialized_prefixes() {
    for left_empty in [false, true] {
        let owners = Arc::new(Payloads::default());
        let mut left = OwnedBranch::new(&owners, 0, if left_empty { 0 } else { 2 });
        let mut right = OwnedBranch::new(&owners, 100, if left_empty { 2 } else { 0 });
        let left_before = left.snapshot();
        let right_before = right.snapshot();
        let original_payloads = owners.live();
        let original_nodes = node_ids();
        owners.arm(1);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            left.branch_mut().merge(right.branch_mut(), &mut Untracked);
        }))
        .is_err());
        assert_eq!(left.snapshot(), left_before);
        assert_eq!(right.snapshot(), right_before);
        assert_eq!(owners.live(), original_payloads);
        assert_eq!(node_ids(), original_nodes);

        owners.arm(0);
        left.branch_mut().merge(right.branch_mut(), &mut Untracked);
        assert_eq!(owners.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(left.branch().count(), 3);
        assert_eq!(right.branch().count(), 0);
        assert!(Node::verify_raw(left.node.0));
        let (keys, children) = left.snapshot();
        let mut original_keys = left_before.0;
        original_keys.extend(right_before.0);
        for key in original_keys {
            assert!(
                keys.contains(&key),
                "merge must move the original separator"
            );
        }
        let mut original_children = left_before.1;
        original_children.extend(right_before.1);
        assert_eq!(children, original_children);
        drop(left);
        drop(right);
        owners.assert_all_reclaimed();
        assert_released();
    }
}

#[test]
fn branch_redistribution_moves_existing_keys_and_prepares_only_the_bridge_before_mutation() {
    for left_to_right in [false, true] {
        let owners = Arc::new(Payloads::default());
        let mut left = OwnedBranch::new(&owners, 0, if left_to_right { L_CAPACITY } else { 0 });
        let mut right = OwnedBranch::new(&owners, 100, if left_to_right { 0 } else { L_CAPACITY });
        let left_before = left.snapshot();
        let right_before = right.snapshot();
        let original_payloads = owners.live();
        let original_nodes = node_ids();
        owners.arm(1);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if left_to_right {
                left.branch_mut()
                    .take_from_l_to_r(right.branch_mut(), &mut Untracked);
            } else {
                left.branch_mut()
                    .take_from_r_to_l(right.branch_mut(), &mut Untracked);
            }
        }))
        .is_err());
        assert_eq!(left.snapshot(), left_before);
        assert_eq!(right.snapshot(), right_before);
        assert_eq!(owners.live(), original_payloads);
        assert_eq!(node_ids(), original_nodes);

        owners.arm(0);
        if left_to_right {
            left.branch_mut()
                .take_from_l_to_r(right.branch_mut(), &mut Untracked);
        } else {
            left.branch_mut()
                .take_from_r_to_l(right.branch_mut(), &mut Untracked);
        }
        assert_eq!(owners.attempts.load(Ordering::SeqCst), 1);
        assert!(Node::verify_raw(left.node.0));
        assert!(Node::verify_raw(right.node.0));
        let count = L_CAPACITY / 2;
        let start = L_CAPACITY - count;
        let (left_keys, left_children) = left.snapshot();
        let (right_keys, right_children) = right.snapshot();
        let boundary = if left_to_right {
            assert_eq!(left_keys, left_before.0[..start]);
            assert_eq!(right_keys[..count - 1], left_before.0[start + 1..]);
            left_before.0[start]
        } else {
            assert_eq!(left_keys[1..], right_before.0[..count - 1]);
            assert_eq!(right_keys, right_before.0[count..]);
            right_before.0[count - 1]
        };
        let boundary_drops = owners.drops.lock().unwrap()[boundary];
        assert_eq!(boundary_drops, 1);
        let mut before_children = left_before.1;
        before_children.extend(right_before.1);
        let mut after_children = left_children;
        after_children.extend(right_children);
        assert_eq!(after_children, before_children);
        drop(left);
        drop(right);
        owners.assert_all_reclaimed();
        assert_released();
    }
}

#[test]
fn multilevel_map_removal_abort_and_commit_reclaim_keys_after_oldest_reader_release() {
    for descending in [false, true] {
        let owners = Arc::new(Payloads::default());
        let entries = 2 * BV_CAPACITY * BV_CAPACITY;
        let map = map_with_entries(&owners, entries);
        let oldest = map.read();
        let original_keys: Vec<_> = oldest.keys().collect();
        let original_payloads = owners.live();
        let original_nodes = node_ids();
        let first_clone = owners.allocated();
        let key_at = |index| {
            if descending {
                entries - index - 1
            } else {
                index
            }
        };
        let mut writer = map.write();
        for index in 0..entries / 2 {
            let key = key_at(index);
            assert_eq!(writer.remove(original_keys[key]).unwrap().bytes[0], key);
        }
        let detached = writer.detach();
        assert_eq!(detached.to_snapshot().len(), entries - entries / 2);
        drop(detached);
        assert_eq!(owners.live(), original_payloads);
        owners.assert_reclaimed_since(first_clone);
        assert_eq!(node_ids(), original_nodes);

        let mut writer = map.write();
        for index in 0..entries {
            let key = key_at(index);
            assert_eq!(writer.remove(original_keys[key]).unwrap().bytes[0], key);
        }
        assert!(writer.is_empty());
        writer.commit();
        assert!(map.read().is_empty());
        for key in 0..entries {
            assert_eq!(oldest.get(&key).unwrap().bytes[0], key);
        }
        assert!(!owners.live().is_empty());
        drop(oldest);
        owners.assert_all_reclaimed();
        drop(map);
        assert_released();
    }
}

#[cfg(debug_assertions)]
#[test]
fn new_branch_verification_unwind_reclaims_owned_node_and_separator() {
    let owners = Arc::new(Payloads::default());
    let left = leaf(&owners, 0, 1);
    let right = leaf(&owners, 10, 1);
    let original_payloads = owners.live();
    let original_nodes = node_ids();
    let first_clone = owners.allocated();
    owners.comparisons.store(0, Ordering::SeqCst);
    owners.compare_panic.store(true, Ordering::SeqCst);
    // Single-entry children require no comparisons during the initial input
    // verification. The first comparison occurs after constructing the parent
    // and cloning its separator, while verifying that new parent.
    assert!(catch_unwind(AssertUnwindSafe(|| {
        OwnedNode(Node::new_branch(1, left.0, right.0, &mut Untracked).cast())
    }))
    .is_err());
    owners.compare_panic.store(false, Ordering::SeqCst);
    assert_eq!(owners.comparisons.load(Ordering::SeqCst), 1);
    assert_eq!(owners.allocated(), first_clone + 1);
    assert_eq!(owners.live(), original_payloads);
    owners.assert_reclaimed_since(first_clone);
    assert_eq!(node_ids(), original_nodes);

    let retry = OwnedNode(Node::new_branch(1, left.0, right.0, &mut Untracked).cast());
    assert!(Node::verify_raw(retry.0));
    let branch = unsafe { &*retry.0.cast::<Branch<Payload, Payload>>() };
    assert_eq!(&branch.nodes[..=branch.count()], &[left.0, right.0]);
    drop(retry);
    assert_eq!(owners.live(), original_payloads);
    owners.assert_reclaimed_since(first_clone);
    assert_eq!(node_ids(), original_nodes);
    drop(left);
    drop(right);
    owners.assert_all_reclaimed();
    assert_released();
}
