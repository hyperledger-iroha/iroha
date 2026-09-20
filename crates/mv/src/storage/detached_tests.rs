//! Detached storage deltas preserve the original pair and release every writer.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn detach<K: Key, V: Value>(block: Block<'_, K, V>) -> Detached<K, V, ()> {
    block.try_detach(|_| Ok::<_, ()>(())).unwrap()
}

fn entries(journal: &Detached<u64, u64, ()>) -> Vec<(u64, Option<u64>, Option<u64>)> {
    journal
        .touched_entries()
        .map(|entry| (*entry.key, entry.before.copied(), entry.after.copied()))
        .collect()
}

fn writers_are_free<K: Key, V: Value>(storage: &Storage<K, V>) {
    assert!(storage.revert.try_write().is_some());
    assert!(storage.blocks.try_write().is_some());
}

#[test]
fn ordinary_capture_retains_applied_noop_and_absence_touches_without_publication() {
    let storage: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let mut block = storage.block();
    {
        let mut child = block.transaction();
        child.insert(1, 11);
        child.remove(2);
        child.insert(3, 30);
        child.apply();
    }
    {
        let mut child = block.transaction();
        child.insert(1, 99);
        child.remove(3);
        child.insert(4, 40);
    }
    block.remove(5);
    let _ = block.get_mut(&1);
    let journal = detach(block);
    assert_eq!(journal.mode(), BlockMode::Ordinary);
    assert!(journal.is_dirty());
    assert_eq!(
        entries(&journal),
        [
            (1, Some(10), Some(11)),
            (2, Some(20), None),
            (3, None, Some(30)),
            (5, None, None)
        ]
    );
    writers_are_free(&storage);
    assert!(journal.matches_current(&storage));
    assert!(!journal.matches_current(&[(1, 10), (2, 20)].into_iter().collect()));
    let current = storage.snapshot();
    assert_eq!(
        current
            .current()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>(),
        [(1, 10), (2, 20)]
    );
    assert!(current.revert_map().is_empty());
    drop(journal);
    assert!(storage.snapshot().revert_map().is_empty());
    assert_eq!(storage.view().len(), 2);
}

#[test]
fn replacement_mode_retains_discarded_tip_only_changes_and_real_undo() {
    let storage: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let mut tip = storage.block();
    tip.insert(1, 11);
    tip.remove(2);
    tip.insert(3, 30);
    tip.commit();
    let before = storage.snapshot();
    let mut replacement = storage.block_and_revert();
    replacement.insert(1, 12);
    let journal = detach(replacement);
    assert_eq!(journal.mode(), BlockMode::Replace);
    assert!(journal.is_dirty());
    assert_eq!(entries(&journal), [(1, Some(10), Some(12))]);
    writers_are_free(&storage);
    assert!(
        storage
            .block_and_revert()
            .try_detach(|_| Err::<(), _>("capacity"))
            .is_err()
    );
    writers_are_free(&storage);
    assert_eq!(storage.view().get(&1), Some(&11));
    assert_eq!(
        storage.snapshot().revert_map().iter().collect::<Vec<_>>(),
        before.revert_map().iter().collect::<Vec<_>>()
    );
    assert!(!journal.matches_block_predecessor(&storage.block()));
    let mut actual_replacement = storage.block_and_revert();
    assert!(journal.matches_block_predecessor(&actual_replacement));
    assert_eq!(actual_replacement.get(&2), Some(&20));
    assert_eq!(actual_replacement.get(&3), None);
    // Exercise the existing block writer explicitly, not a detached publication
    // API: the replacement mode restores the untouched discarded-tip entries.
    for entry in journal.touched_entries() {
        match entry.after {
            Some(value) => {
                actual_replacement.insert(*entry.key, *value);
            }
            None => {
                actual_replacement.remove(*entry.key);
            }
        }
    }
    actual_replacement.commit();
    assert_eq!(storage.view().get(&1), Some(&12));
    assert_eq!(storage.view().get(&2), Some(&20));
    assert_eq!(storage.view().get(&3), None);
    assert_eq!(
        storage
            .snapshot()
            .revert_map()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([(1, Some(10))])
    );
    assert_eq!(before.current().get(&1), Some(&11));
    assert_eq!(before.current().get(&2), None);
    assert!(!journal.matches_current(&storage));
}

#[test]
fn unchanged_replacement_and_undo_only_commit_have_distinct_pair_identity() {
    let storage: Storage<u64, u64> = [(1, 10)].into_iter().collect();
    let mut tip = storage.block();
    tip.insert(1, 11);
    tip.commit();
    let journal = detach(storage.block_and_revert());
    assert!(journal.is_dirty());
    assert_eq!(journal.mode(), BlockMode::Replace);
    assert!(entries(&journal).is_empty());
    assert!(journal.matches_current(&storage));
    let ordinary = detach(storage.block());
    assert!(!ordinary.is_dirty());
    assert!(ordinary.touched_entries().next().is_none());
    storage.block().commit();
    assert_eq!(storage.view().get(&1), Some(&11));
    assert!(storage.snapshot().revert_map().is_empty());
    assert!(!journal.matches_current(&storage));
    assert!(
        !ordinary.matches_current(&storage),
        "clearing undo is a real pair publication"
    );
}

#[test]
fn direct_insert_and_reverted_predecessor_cannot_reuse_original_identity() {
    let mut storage: Storage<_, _> = [(1, 10)].into_iter().collect();
    let mut tip = storage.block();
    tip.insert(1, 11);
    tip.commit();
    let journal = detach(storage.block());
    storage.insert(1, 12);
    assert!(!journal.matches_current(&storage));
    storage.insert(1, 11);
    assert_eq!(
        storage
            .snapshot()
            .revert_map()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([(1, Some(10))])
    );
    assert!(
        !journal.matches_current(&storage),
        "equal current and undo cannot recreate an opaque version"
    );
    let exact = detach(storage.block());
    storage.block_and_revert().commit();
    assert_eq!(storage.view().get(&1), Some(&10));
    assert!(!exact.matches_current(&storage));
}

#[test]
fn aborted_children_and_noop_touches_survive_detachment_without_invented_entries() {
    let storage: Storage<_, _> = [(1, 10)].into_iter().collect();
    let mut block = storage.block();
    {
        let mut aborted = block.transaction();
        aborted.insert(1, 99);
        aborted.remove(8);
    }
    let journal = detach(block);
    assert!(!journal.is_dirty());
    assert!(entries(&journal).is_empty());
    let mut block = storage.block();
    let _ = block.get_mut(&1);
    block.remove(8);
    let touched = detach(block);
    assert_eq!(
        entries(&touched),
        [(1, Some(10), Some(10)), (8, None, None)]
    );
    assert!(journal.matches_current(&storage) && touched.matches_current(&storage));
    let mut absent = storage.block();
    absent.remove(9);
    let absent = detach(absent);
    assert!(
        !absent.is_dirty(),
        "absent removal needs only undo publication"
    );
    assert_eq!(entries(&absent), [(9, None, None)]);
    drop((journal, touched, absent));
    assert!(storage.snapshot().revert_map().is_empty());
}

#[test]
fn detachment_retains_original_values_without_clones_and_releases_reservation_last() {
    #[derive(Debug)]
    struct Counted(u64, Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.1.fetch_add(1, Ordering::SeqCst);
            Self(self.0, Arc::clone(&self.1))
        }
    }
    struct Reservation(Arc<AtomicBool>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let copies = Arc::new(AtomicUsize::new(0));
    let storage: Storage<u64, _> = (0..100)
        .map(|key| (key, Counted(key, Arc::clone(&copies))))
        .collect();
    let mut block = storage.block();
    block.insert(2, Counted(22, Arc::clone(&copies)));
    copies.store(0, Ordering::SeqCst);
    let result = block.try_detach(|original| {
        assert_eq!(original.touched_entries().len(), 1);
        Err::<(), _>("budget")
    });
    assert!(matches!(result, Err("budget")));
    assert_eq!(copies.load(Ordering::SeqCst), 0);
    writers_are_free(&storage);
    assert_eq!(storage.view().get(&2).unwrap().0, 2);
    assert!(storage.snapshot().revert_map().is_empty());
    let mut block = storage.block();
    block.insert(2, Counted(22, Arc::clone(&copies)));
    block.remove(3);
    copies.store(0, Ordering::SeqCst);
    let released = Arc::new(AtomicBool::new(false));
    let journal = block
        .try_detach(|original| {
            assert_eq!(original.touched_entries().len(), 2);
            Ok::<_, ()>(Reservation(Arc::clone(&released)))
        })
        .unwrap();
    assert_eq!(
        copies.load(Ordering::SeqCst),
        0,
        "both original successors are retained without copying any key or value"
    );
    assert_eq!(journal.touched_entries().len(), 2);
    assert!(!journal.admission().0.load(Ordering::SeqCst));
    drop(journal);
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(storage.view().len(), 100);
}

#[test]
fn disjoint_candidates_are_owned_send_journals_and_release_all_writers() {
    fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    assert_send_sync_static::<Detached<u64, u64, ()>>();
    let storage: Arc<Storage<_, _>> = Arc::new([(1, 10), (2, 20)].into_iter().collect());
    let worker = Arc::clone(&storage);
    let first = std::thread::spawn(move || {
        let mut block = worker.block();
        block.insert(1, 11);
        detach(block)
    })
    .join()
    .unwrap();
    writers_are_free(&storage);
    let worker = Arc::clone(&storage);
    let second = std::thread::spawn(move || {
        let mut block = worker.block();
        block.insert(2, 22);
        detach(block)
    })
    .join()
    .unwrap();
    assert_eq!(entries(&first), [(1, Some(10), Some(11))]);
    assert_eq!(entries(&second), [(2, Some(20), Some(22))]);
    assert!(first.matches_current(&storage) && second.matches_current(&storage));
    assert_eq!(storage.view().get(&1), Some(&10));
    assert_eq!(storage.view().get(&2), Some(&20));
    drop((first, second));
    writers_are_free(&storage);
}

#[test]
fn snapshot_json_and_history_projection_create_new_owners_with_exact_images() {
    let mut storage: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let mut block = storage.block();
    block.insert(1, 11);
    block.remove(2);
    block.remove(9);
    block.commit();
    let journal = detach(storage.block_and_revert());
    let encoded = norito::json::to_json(&storage).unwrap();
    let decoded: Storage<u64, u64> = norito::json::from_json(&encoded).unwrap();
    let snapshot = storage.snapshot();
    let restored = Storage::from_snapshot_parts(
        snapshot
            .current()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
        snapshot
            .revert_map()
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect(),
    );
    drop(snapshot);
    let projected = storage.history().project(|value| Some(*value));
    for other in [&decoded, &restored, &projected] {
        assert_eq!(norito::json::to_json(other).unwrap(), encoded);
        assert!(!journal.matches_current(other));
        let replacement = other.block_and_revert();
        assert_eq!(replacement.get(&1), Some(&10));
        assert_eq!(replacement.get(&2), Some(&20));
        assert_eq!(replacement.get(&9), None);
    }
    assert!(journal.matches_current(&storage));
}

#[test]
fn detached_values_outlive_the_storage_without_a_reader_pin() {
    let journal = {
        let storage: Storage<u64, String> = [(1, String::from("before"))].into_iter().collect();
        let mut block = storage.block();
        block.insert(1, String::from("after"));
        detach(block)
    };
    let entry = journal.touched_entries().next().unwrap();
    assert_eq!(*entry.key, 1);
    assert_eq!(entry.before.unwrap(), "before");
    assert_eq!(entry.after.unwrap(), "after");
    assert!(!journal.matches_current(&Storage::new()));
}
