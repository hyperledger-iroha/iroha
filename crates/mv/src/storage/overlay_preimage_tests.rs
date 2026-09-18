//! Actual storage overlay preimages across apply, rollback and block replacement.

use super::*;
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn entries<'a>(
    entries: impl Iterator<Item = TouchedEntry<'a, u64, Vec<u8>>>,
) -> Vec<(u64, Option<Vec<u8>>, Option<Vec<u8>>)> {
    entries
        .map(|entry| (*entry.key, entry.before.cloned(), entry.after.cloned()))
        .collect()
}

#[test]
fn applied_siblings_preserve_block_preimages_and_aborted_children_leave_no_delta() {
    let storage: Storage<_, _> = [(1, vec![10]), (2, vec![20])].into_iter().collect();
    let mut block = storage.block();
    assert_eq!(block.touched_entries().len(), 0);
    {
        let mut tx = block.transaction();
        tx.insert(3, Vec::new());
        tx.remove(2);
        tx.insert(1, vec![11]);
        assert_eq!(tx.get_before_transaction(&1), Some(&vec![10]));
        assert_eq!(tx.get_before_block(&1), Some(&vec![10]));
        assert_eq!(tx.get_before_transaction(&3), None);
        assert_eq!(tx.get_before_block(&3), None);
        assert_eq!(
            entries(tx.touched_entries()),
            vec![
                (1, Some(vec![10]), Some(vec![11])),
                (2, Some(vec![20]), None),
                (3, None, Some(Vec::new())),
            ]
        );
        tx.apply();
    }
    {
        let mut tx = block.transaction();
        assert_eq!(tx.get_before_transaction(&1), Some(&vec![11]));
        assert_eq!(tx.get_before_block(&1), Some(&vec![10]));
        tx.insert(1, vec![99]);
        tx.remove(3);
        tx.insert(4, vec![40]);
        assert_eq!(tx.get_before_transaction(&1), Some(&vec![11]));
        assert_eq!(tx.get_before_block(&1), Some(&vec![10]));
        assert_eq!(tx.get_before_transaction(&2), None);
        assert_eq!(tx.get_before_block(&2), Some(&vec![20]));
        assert_eq!(
            entries(tx.touched_entries()),
            vec![
                (1, Some(vec![11]), Some(vec![99])),
                (3, Some(Vec::new()), None),
                (4, None, Some(vec![40])),
            ]
        );
    }
    assert_eq!(
        entries(block.touched_entries()),
        vec![
            (1, Some(vec![10]), Some(vec![11])),
            (2, Some(vec![20]), None),
            (3, None, Some(Vec::new())),
        ]
    );
    assert_eq!(block.get_before_block(&2), Some(&vec![20]));
    assert_eq!(block.get(&4), None);
    block.commit();
    let view = storage.view();
    assert_eq!(view.get(&1), Some(&vec![11]));
    assert_eq!(view.get(&2), None);
    assert_eq!(view.get(&3), Some(&Vec::new()));
    assert_eq!(view.get(&4), None);
}

#[test]
fn touches_distinguish_noops_absence_empty_and_canonical_key_order() {
    let storage: Storage<_, _> = [(1, vec![10]), (2, Vec::new())].into_iter().collect();
    let mut block = storage.block();
    block.remove(9);
    block.insert(3, vec![30]);
    block.remove(3);
    assert_eq!(block.get_mut(&1), Some(&mut vec![10]));
    block.remove(2);
    block.insert(2, Vec::new());
    let expected = vec![
        (1, Some(vec![10]), Some(vec![10])),
        (2, Some(Vec::new()), Some(Vec::new())),
        (3, None, None),
        (9, None, None),
    ];
    assert_eq!(entries(block.touched_entries()), expected);
    let mut iter = block.touched_entries();
    assert_eq!(iter.len(), 4);
    assert_eq!(*iter.next_back().unwrap().key, 9);
    assert_eq!(*iter.next().unwrap().key, 1);
    assert_eq!(iter.len(), 2);
    assert_eq!(*iter.next_back().unwrap().key, 3);
    assert_eq!(*iter.next().unwrap().key, 2);
    assert!(iter.next().is_none());
    drop(iter);
    let mut tx = block.transaction();
    tx.remove(9);
    assert_eq!(tx.get_before_transaction(&9), None);
    assert_eq!(entries(tx.touched_entries()), vec![(9, None, None)]);
}

#[test]
fn replacement_block_starts_after_revert_and_keeps_persistent_views() {
    let storage: Storage<_, _> = [(1, vec![10]), (2, vec![20])].into_iter().collect();
    {
        let mut block = storage.block();
        block.insert(1, vec![11]);
        block.remove(2);
        block.insert(3, vec![30]);
        block.commit();
    }
    let previous = storage.view();
    let mut replacement = storage.block_and_revert();
    assert!(replacement.is_dirty());
    assert_eq!(replacement.touched_entries().len(), 0);
    assert_eq!(replacement.get_before_block(&1), Some(&vec![10]));
    assert_eq!(replacement.get_before_block(&2), Some(&vec![20]));
    assert_eq!(replacement.get_before_block(&3), None);
    {
        let mut tx = replacement.transaction();
        tx.insert(1, vec![12]);
        assert_eq!(tx.get_before_block(&1), Some(&vec![10]));
        assert_eq!(tx.get_before_transaction(&1), Some(&vec![10]));
        tx.apply();
    }
    assert_eq!(
        entries(replacement.touched_entries()),
        vec![(1, Some(vec![10]), Some(vec![12]))]
    );
    replacement.commit();
    assert_eq!(previous.get(&1), Some(&vec![11]));
    assert_eq!(previous.get(&2), None);
    assert_eq!(previous.get(&3), Some(&vec![30]));
    let next = storage.view();
    assert_eq!(next.get(&1), Some(&vec![12]));
    assert_eq!(next.get(&2), Some(&vec![20]));
    assert_eq!(next.get(&3), None);
}

#[test]
fn unwind_discards_child_preimages_without_erasing_an_applied_sibling() {
    let storage: Storage<_, _> = [(1, vec![10])].into_iter().collect();
    let mut block = storage.block();
    block.insert(1, vec![11]);
    let before = entries(block.touched_entries());
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut tx = block.transaction();
        tx.remove(1);
        tx.insert(2, vec![20]);
        assert_eq!(tx.get_before_transaction(&1), Some(&vec![11]));
        assert_eq!(tx.get_before_block(&1), Some(&vec![10]));
        panic!("interrupt actual storage child");
    }));
    assert!(result.is_err());
    assert_eq!(entries(block.touched_entries()), before);
    assert_eq!(block.get(&1), Some(&vec![11]));
    assert_eq!(block.get(&2), None);
    block.commit();
    let fresh = storage.block();
    assert_eq!(fresh.touched_entries().len(), 0);
    assert_eq!(fresh.get_before_block(&1), Some(&vec![11]));
}

#[test]
fn preimage_projection_clones_neither_stored_keys_nor_values() {
    #[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Key(u64);
    static KEY_CLONES: AtomicUsize = AtomicUsize::new(0);
    impl Clone for Key {
        fn clone(&self) -> Self {
            KEY_CLONES.fetch_add(1, Ordering::Relaxed);
            Self(self.0)
        }
    }
    struct Value(u64, Arc<AtomicUsize>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.fetch_add(1, Ordering::Relaxed);
            Self(self.0, self.1.clone())
        }
    }
    let clones = Arc::new(AtomicUsize::new(0));
    let storage: Storage<_, _> = [(Key(1), Value(10, clones.clone()))].into_iter().collect();
    let mut block = storage.block();
    block.insert(Key(1), Value(11, clones.clone()));
    clones.store(0, Ordering::Relaxed);
    KEY_CLONES.store(0, Ordering::Relaxed);
    let entry = block.touched_entries().next().unwrap();
    assert_eq!(entry.key.0, 1);
    assert_eq!(entry.before.unwrap().0, 10);
    assert_eq!(entry.after.unwrap().0, 11);
    assert_eq!(block.get_before_block(&Key(1)).unwrap().0, 10);
    assert_eq!(clones.load(Ordering::Relaxed), 0);
    assert_eq!(KEY_CLONES.load(Ordering::Relaxed), 0);
    let mut tx = block.transaction();
    tx.insert(Key(1), Value(12, clones.clone()));
    clones.store(0, Ordering::Relaxed);
    KEY_CLONES.store(0, Ordering::Relaxed);
    let entry = tx.touched_entries().next_back().unwrap();
    assert_eq!(entry.before.unwrap().0, 11);
    assert_eq!(entry.after.unwrap().0, 12);
    assert_eq!(tx.get_before_transaction(&Key(1)).unwrap().0, 11);
    assert_eq!(tx.get_before_block(&Key(1)).unwrap().0, 10);
    assert_eq!(clones.load(Ordering::Relaxed), 0);
    assert_eq!(KEY_CLONES.load(Ordering::Relaxed), 0);
}

#[test]
fn applied_delta_reconstructs_storage_from_original_values_across_many_transactions() {
    let initial: BTreeMap<u64, Vec<u8>> = [(0, vec![0]), (2, Vec::new()), (5, vec![5])].into();
    for seed in 0_u64..32 {
        let storage: Storage<_, _> = initial.clone().into_iter().collect();
        let mut block = storage.block();
        let mut expected = initial.clone();
        for step in 0..24_u64 {
            let apply = (seed + step) % 3 != 0;
            let mut next = expected.clone();
            let mut tx = block.transaction();
            for offset in 0..4_u64 {
                let key = (seed * 3 + step * 5 + offset) % 8;
                if (seed + step + offset) % 4 == 0 {
                    tx.remove(key);
                    next.remove(&key);
                } else {
                    let value = vec![step as u8, offset as u8];
                    tx.insert(key, value.clone());
                    next.insert(key, value);
                }
            }
            for key in 0..8 {
                assert_eq!(tx.get_before_transaction(&key), expected.get(&key));
                assert_eq!(tx.get_before_block(&key), initial.get(&key));
                assert_eq!(tx.get(&key), next.get(&key));
            }
            if apply {
                tx.apply();
                expected = next;
            }
        }
        let mut reconstructed = initial.clone();
        for change in block.touched_entries() {
            assert_eq!(change.before, initial.get(change.key));
            if let Some(value) = change.after {
                reconstructed.insert(*change.key, value.clone());
            } else {
                reconstructed.remove(change.key);
            }
        }
        assert_eq!(reconstructed, expected);
        assert_eq!(
            block
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        block.commit();
        assert_eq!(
            storage
                .view()
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
}
