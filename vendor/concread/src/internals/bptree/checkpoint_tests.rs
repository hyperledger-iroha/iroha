//! Original checkpoint metadata, allocation custody and private generation tags.

use super::*;
use crate::bptree::{NodeCloning, NodeFunding, Prepaid};
use crate::internals::bptree::cursor::{CursorReadOps, SuperBlock};
use crate::internals::bptree::node::allocation_tests::{
    all_refunded, prepaid, record, without_allocations, Charge, Prepaid as ObservedPrepaid,
};
use crate::internals::lincowcell::LinCowCellCapable;
use std::{alloc::Layout, cell::RefCell, rc::Rc};

#[derive(Clone)]
struct ScalarPolicy(Rc<RefCell<ObservedPrepaid>>);

impl NodeFunding for ScalarPolicy {
    type Charge = Charge;

    fn take_node_charge(&mut self, layout: Layout) -> Charge {
        self.0.borrow_mut().take_allocation_charge(layout)
    }
}

impl NodeCloning<usize, usize> for ScalarPolicy {
    fn clone_key(&mut self, key: &usize) -> usize {
        *key
    }

    fn clone_value(&mut self, value: &usize) -> usize {
        *value
    }
}

type Cursor = CursorWrite<usize, usize, Prepaid<ScalarPolicy>>;
type Checkpoint<'a> = CursorCheckpoint<'a, usize, usize, Prepaid<ScalarPolicy>>;
type Tracking = Buffer<usize, usize, Prepaid<ScalarPolicy>>;

struct Fixture {
    // Private allocations must disappear before the original published root.
    cursor: Cursor,
    source: SuperBlock<usize, usize, Prepaid<ScalarPolicy>>,
    policy: ScalarPolicy,
}

fn tracking(capacity: usize, policy: &mut ScalarPolicy) -> Tracking {
    let charge = policy.take_node_charge(Tracking::allocation_layout(capacity).unwrap());
    Tracking::try_new(capacity, charge).unwrap_or_else(|_| panic!("valid explicit capacity"))
}

fn fixture() -> Fixture {
    let mut policy = ScalarPolicy(Rc::new(RefCell::new(prepaid())));
    let mut initial = Prepaid(Some(policy.clone()));
    // SAFETY: Fixture retains this original source until its only cursor drops.
    let source = unsafe { SuperBlock::new_with_funding(&mut initial) };
    let first = tracking(4, &mut policy);
    let last = tracking(4, &mut policy);
    let mut cursor = Cursor::with_input(&source, (Prepaid(Some(policy.clone())), first, last));
    cursor.begin_admitted_edit();
    assert_eq!(cursor.try_insert(0, 100), Ok(None));
    cursor.finish_admitted_funding();
    Fixture {
        cursor,
        source,
        policy,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Identity {
    root: usize,
    txid: u64,
    length: usize,
    first: (usize, usize, usize),
    last: (usize, usize, usize),
}

fn identity(cursor: &Cursor) -> Identity {
    let last = cursor
        .last_seen
        .as_ref()
        .expect("original retirement buffer");
    Identity {
        root: cursor.get_root() as usize,
        txid: cursor.get_txid(),
        length: cursor.len(),
        first: (
            cursor.first_seen.as_ptr() as usize,
            cursor.first_seen.capacity(),
            cursor.first_seen.as_slice().len(),
        ),
        last: (
            last.as_ptr() as usize,
            last.capacity(),
            last.as_slice().len(),
        ),
    }
}

fn insert_with_growth(
    checkpoint: &mut Checkpoint<'_>,
    policy: &ScalarPolicy,
    key: usize,
    capacity: usize,
) {
    // Explicit low-level growth isolates checkpoint custody. Public admitted
    // operation planning and complete MV funding are covered by their own tests.
    let mut provider = policy.clone();
    let first = tracking(capacity, &mut provider);
    let last = tracking(capacity, &mut provider);
    let (cursor, saved) = checkpoint.edit_parts();
    assert!(capacity > cursor.first_seen.capacity());
    assert!(capacity > cursor.last_seen.as_ref().unwrap().capacity());
    cursor.begin_admitted_edit();
    cursor.resume_admitted_funding(provider, Some(first), Some(last), Some(saved));
    assert_eq!(cursor.try_insert(key, key + 100), Ok(None));
    cursor.finish_admitted_funding();
    assert!(cursor.verify());
}

fn allocation_count(policy: &ScalarPolicy) -> usize {
    policy.0.as_ref().borrow().next
}

fn assert_reclaimed(start: usize, end: usize) {
    for id in start..end {
        assert!(record(id).freed, "allocation {id} was not actually freed");
        assert!(record(id).refunded, "allocation {id} retained its charge");
    }
}

fn finish(fixture: Fixture) {
    let count = allocation_count(&fixture.policy);
    without_allocations(|| drop(fixture));
    all_refunded(&ObservedPrepaid {
        next: count,
        remaining: 0,
    });
}

#[test]
fn abort_restores_original_root_tag_length_and_both_buffers_after_repeated_growth() {
    let mut fixture = fixture();
    let original = identity(&fixture.cursor);
    let cut = allocation_count(&fixture.policy);
    {
        let mut checkpoint = without_allocations(|| fixture.cursor.checkpoint().unwrap());
        insert_with_growth(&mut checkpoint, &fixture.policy, 1, 8);
        insert_with_growth(&mut checkpoint, &fixture.policy, 2, 16);
        let saved = &checkpoint.saved.as_ref().unwrap().buffers;
        assert_eq!(
            saved.first.as_ref().unwrap().as_ptr() as usize,
            original.first.0
        );
        assert_eq!(
            saved.last.as_ref().unwrap().as_ptr() as usize,
            original.last.0
        );
        assert!(!record(1).freed && !record(1).refunded);
        assert!(!record(2).freed && !record(2).refunded);
        assert_eq!(checkpoint.as_ref().len(), 3);
        without_allocations(|| drop(checkpoint));
    }
    assert_eq!(identity(&fixture.cursor), original);
    assert_eq!(fixture.cursor.search(&0), Some(&100));
    assert_eq!(fixture.cursor.search(&1), None);
    assert_eq!(fixture.cursor.search(&2), None);
    assert!(fixture.cursor.verify());
    assert_reclaimed(cut, allocation_count(&fixture.policy));
    assert!(!record(1).freed && !record(2).freed);
    finish(fixture);
}

#[test]
fn nested_apply_transfers_original_buffers_and_outer_abort_restores_them() {
    for outer_grows in [false, true] {
        let mut fixture = fixture();
        let original = identity(&fixture.cursor);
        let cut = allocation_count(&fixture.policy);
        let mut outer = without_allocations(|| fixture.cursor.checkpoint().unwrap());
        if outer_grows {
            insert_with_growth(&mut outer, &fixture.policy, 1, 8);
        }
        let mut child = without_allocations(|| outer.checkpoint().unwrap());
        insert_with_growth(&mut child, &fixture.policy, 2, 16);
        insert_with_growth(&mut child, &fixture.policy, 3, 32);
        let newest = child.as_ref().txid;
        without_allocations(|| child.apply());
        assert_eq!(outer.as_ref().txid, newest);
        let saved = &outer.saved.as_ref().unwrap().buffers;
        assert_eq!(
            saved.first.as_ref().unwrap().as_ptr() as usize,
            original.first.0
        );
        assert_eq!(
            saved.last.as_ref().unwrap().as_ptr() as usize,
            original.last.0
        );
        assert_eq!(outer.as_ref().search(&2), Some(&102));
        assert_eq!(outer.as_ref().search(&3), Some(&103));
        without_allocations(|| drop(outer));
        assert_eq!(identity(&fixture.cursor), original);
        assert_reclaimed(cut, allocation_count(&fixture.policy));
        assert!(fixture.cursor.verify());
        finish(fixture);
    }
}

#[test]
fn applied_newest_tag_survives_and_sibling_reuse_follows_actual_child_reclamation() {
    let mut fixture = fixture();
    let mut outer = fixture.cursor.checkpoint().unwrap();
    insert_with_growth(&mut outer, &fixture.policy, 1, 8);
    let mut child = outer.checkpoint().unwrap();
    insert_with_growth(&mut child, &fixture.policy, 2, 16);
    let applied_tag = child.as_ref().txid;
    without_allocations(|| child.apply());
    assert_eq!(outer.as_ref().txid, applied_tag);
    without_allocations(|| outer.apply());
    assert_eq!(fixture.cursor.txid, applied_tag);
    let parent = identity(&fixture.cursor);

    let cut = allocation_count(&fixture.policy);
    let mut aborted = fixture.cursor.checkpoint().unwrap();
    let reusable_tag = aborted.as_ref().txid;
    insert_with_growth(&mut aborted, &fixture.policy, 3, 32);
    without_allocations(|| drop(aborted));
    assert_eq!(identity(&fixture.cursor), parent);
    assert_reclaimed(cut, allocation_count(&fixture.policy));

    let mut sibling = without_allocations(|| fixture.cursor.checkpoint().unwrap());
    assert_eq!(sibling.as_ref().txid, reusable_tag);
    assert_eq!(sibling.as_ref().search(&3), None);
    insert_with_growth(&mut sibling, &fixture.policy, 4, 32);
    without_allocations(|| sibling.apply());
    assert_eq!(fixture.cursor.txid, reusable_tag);
    assert!(fixture.cursor.verify());

    let Fixture {
        cursor,
        mut source,
        policy,
    } = fixture;
    let old = source.create_reader();
    let current = without_allocations(|| source.pre_commit(cursor, &old));
    assert_eq!(source.txid, reusable_tag);
    for key in [0, 1, 2, 4] {
        assert_eq!(current.search(&key), Some(&(key + 100)));
    }
    assert_eq!(current.search(&3), None);
    assert!(old.search(&0).is_none());
    let count = allocation_count(&policy);
    without_allocations(|| {
        drop(old);
        drop(current);
        drop(source);
        drop(policy);
    });
    all_refunded(&ObservedPrepaid {
        next: count,
        remaining: 0,
    });
}

#[test]
fn exhausted_private_tag_refuses_without_allocating_or_changing_any_owner() {
    let mut fixture = fixture();
    let original_tag = fixture.cursor.txid;
    for exhausted in [(TXID_MASK >> TXID_SHF) - 1, u64::MAX] {
        fixture.cursor.txid = exhausted;
        let before = identity(&fixture.cursor);
        let count = allocation_count(&fixture.policy);
        without_allocations(|| assert!(fixture.cursor.checkpoint().is_none()));
        assert_eq!(identity(&fixture.cursor), before);
        assert_eq!(allocation_count(&fixture.policy), count);
        assert_eq!(fixture.cursor.search(&0), Some(&100));
    }
    fixture.cursor.txid = original_tag;
    finish(fixture);
}

#[test]
fn untracked_abort_restores_parent_nodes_and_cuts_without_allocating_after_growth() {
    let source = unsafe { SuperBlock::<usize, Box<usize>>::new() };
    let mut cursor = source.create_writer(());
    for key in 0..128 {
        cursor.insert(key, Box::new(key + 1000));
    }
    let root = cursor.root;
    let tag = cursor.txid;
    let length = cursor.length;
    let first_cut = cursor.first_seen.len();
    let last_cut = cursor.last_seen.as_ref().unwrap().len();
    let original = cursor.search(&10).unwrap().as_ref() as *const usize;
    let mut outer = without_allocations(|| cursor.checkpoint().unwrap());
    assert_eq!(
        outer.get_before(&10).unwrap().as_ref() as *const usize,
        original
    );
    for key in 128..512 {
        outer.edit_parts().0.insert(key, Box::new(key + 1000));
    }
    for key in 0..100 {
        outer.edit_parts().0.remove(&key);
    }
    **outer.edit_parts().0.get_mut_ref(&110).unwrap() = 999;
    let mut child = without_allocations(|| outer.checkpoint().unwrap());
    assert!(child.get_before(&10).is_none());
    child.edit_parts().0.insert(10, Box::new(10));
    let latest = child.as_ref().txid;
    without_allocations(|| child.apply());
    assert_eq!(outer.as_ref().txid, latest);
    assert_eq!(
        outer.get_before(&10).unwrap().as_ref() as *const usize,
        original
    );
    assert!(outer.get_before(&200).is_none());
    let capacities = (
        outer.cursor.first_seen.capacity(),
        outer.cursor.last_seen.as_ref().unwrap().capacity(),
    );
    without_allocations(|| drop(outer));
    assert_eq!(
        (cursor.root, cursor.txid, cursor.length),
        (root, tag, length)
    );
    assert_eq!(cursor.first_seen.len(), first_cut);
    assert_eq!(cursor.last_seen.as_ref().unwrap().len(), last_cut);
    assert_eq!(cursor.first_seen.capacity(), capacities.0);
    assert_eq!(cursor.last_seen.as_ref().unwrap().capacity(), capacities.1);
    assert_eq!(
        cursor.search(&10).unwrap().as_ref() as *const usize,
        original
    );
    for key in 0..128 {
        assert_eq!(cursor.search(&key).map(Box::as_ref), Some(&(key + 1000)));
    }
    assert!(cursor.verify());
    without_allocations(|| {
        drop(cursor);
        drop(source);
    });
}

#[test]
fn public_untracked_checkpoint_reads_saved_values_and_applies_without_allocation() {
    use crate::bptree::{BptreeMap, BptreeMapCheckpoint};
    let map: BptreeMap<usize, Box<usize>> = (0..64).map(|key| (key, Box::new(key))).collect();
    let old = map.read();
    let original = old.get(&20).unwrap().as_ref() as *const usize;
    let mut writer = map.write();
    let mut parent: BptreeMapCheckpoint<'_, usize, Box<usize>> =
        without_allocations(|| writer.checkpoint().unwrap());
    assert_eq!(parent.len(), 64);
    assert!(!parent.is_empty());
    assert!(parent.contains_key(&20));
    assert_eq!(
        parent.get_before(&20).unwrap().as_ref() as *const usize,
        original
    );
    assert_eq!(parent.first_key_value().map(|(key, _)| *key), Some(0));
    assert_eq!(parent.last_key_value().map(|(key, _)| *key), Some(63));
    assert_eq!(parent.range(10..20).count(), 10);
    assert_eq!(parent.iter().len(), 64);
    assert_eq!(parent.keys().count(), 64);
    assert_eq!(parent.values().count(), 64);
    assert_eq!(parent.to_snapshot().get(&20).map(Box::as_ref), Some(&20));
    assert_eq!(parent.remove(&20), Some(Box::new(20)));
    **parent.get_mut(&21).unwrap() = 121;
    parent.insert(64, Box::new(64));
    let mut child = without_allocations(|| parent.checkpoint().unwrap());
    assert!(child.get_before(&20).is_none());
    assert_eq!(child.get_before(&21).map(Box::as_ref), Some(&121));
    child.insert(20, Box::new(220));
    without_allocations(|| child.apply());
    assert_eq!(parent.get(&20).map(Box::as_ref), Some(&220));
    assert_eq!(
        parent.get_before(&20).unwrap().as_ref() as *const usize,
        original
    );
    without_allocations(|| parent.apply());
    assert_eq!(writer.get(&20).map(Box::as_ref), Some(&220));
    writer.commit();
    assert_eq!(map.read().get(&21).map(Box::as_ref), Some(&121));
    assert_eq!(old.get(&20).unwrap().as_ref() as *const usize, original);
    assert_eq!(old.len(), 64);
}

#[test]
fn caught_untracked_insert_remove_and_mutable_clone_panics_fail_the_original_cursor() {
    use crate::bptree::BptreeMap;
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    struct Value {
        data: Box<usize>,
        panic_clone: Arc<AtomicBool>,
    }
    impl Clone for Value {
        fn clone(&self) -> Self {
            assert!(
                !self.panic_clone.load(Ordering::Relaxed),
                "injected value clone panic"
            );
            Self {
                data: self.data.clone(),
                panic_clone: self.panic_clone.clone(),
            }
        }
    }
    for operation in 0..3 {
        let panic_clone = Arc::new(AtomicBool::new(false));
        let map: BptreeMap<usize, Value> = (0..32)
            .map(|key| {
                (
                    key,
                    Value {
                        data: Box::new(key),
                        panic_clone: panic_clone.clone(),
                    },
                )
            })
            .collect();
        let old = map.read();
        let original = old.get(&10).unwrap().data.as_ref() as *const usize;
        let mut writer = map.write();
        let mut checkpoint = writer.checkpoint().unwrap();
        panic_clone.store(true, Ordering::Relaxed);
        assert!(catch_unwind(AssertUnwindSafe(|| match operation {
            0 => {
                checkpoint.insert(
                    10,
                    Value {
                        data: Box::new(110),
                        panic_clone: panic_clone.clone(),
                    },
                );
            }
            1 => {
                checkpoint.remove(&10);
            }
            _ => {
                checkpoint.get_mut(&10);
            }
        }))
        .is_err());
        panic_clone.store(false, Ordering::Relaxed);
        assert!(catch_unwind(AssertUnwindSafe(|| checkpoint.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| checkpoint.get_before(&10))).is_err());
        drop(checkpoint);
        assert!(catch_unwind(AssertUnwindSafe(|| writer.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| writer.insert(
            99,
            Value {
                data: Box::new(99),
                panic_clone: panic_clone.clone(),
            }
        )))
        .is_err());
        drop(writer);
        assert_eq!(
            old.get(&10).unwrap().data.as_ref() as *const usize,
            original
        );
        assert_eq!(*map.read().get(&10).unwrap().data, 10);
        // Catching the edit panic did not unwind the physical guard. Only the
        // abandoned cursor is failed; a new original writer remains usable.
        let mut replacement = map.try_write().unwrap();
        replacement.insert(
            100,
            Value {
                data: Box::new(100),
                panic_clone: panic_clone.clone(),
            },
        );
        replacement.commit();
        assert_eq!(*map.read().get(&100).unwrap().data, 100);
    }
}

#[test]
fn untracked_checkpoint_retains_only_live_rollback_metadata() {
    use crate::bptree::{BptreeMapCheckpoint, Untracked};
    use std::mem::size_of;

    type Original = Saved<usize, usize, Untracked>;
    type Checkpoint<'a> = CursorCheckpoint<'a, usize, usize, Untracked>;
    // Ordinary Vec growth never displaces a tracking owner into these slots.
    // Every World storage transaction owns two checkpoints, so retaining two
    // impossible Option<Vec> values per guard needlessly expands its stack.
    assert_eq!(size_of::<CheckpointBuffers<usize, usize, Untracked>>(), 0);
    // A live saved root is non-null; the resolved state uses that same niche.
    assert_eq!(size_of::<Option<Original>>(), size_of::<Original>());
    assert_eq!(
        size_of::<Original>(),
        size_of::<(NonNull<Node<usize, usize>>, u64, usize, usize, usize)>()
    );
    assert_eq!(
        size_of::<Checkpoint<'_>>(),
        size_of::<Original>() + 2 * size_of::<usize>()
    );
    assert_eq!(
        size_of::<BptreeMapCheckpoint<'_, usize, usize>>(),
        size_of::<Checkpoint<'_>>()
    );
    // Prepaid checkpoints still carry both original charged buffer owners.
    assert_eq!(
        size_of::<CheckpointBuffers<usize, usize, Prepaid<ScalarPolicy>>>(),
        2 * size_of::<Option<Tracking>>()
    );
}
