//! Exact detached map installation, including refusal, replacement and retained readers.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[test]
fn block_identity_binds_original_owner_predecessor_and_mode_without_reading_values() {
    let target: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let foreign: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let mut block = target.block();
    let identity = block.publication_identity();
    assert!(block.belongs_to(&target));
    assert!(!block.belongs_to(&foreign));
    assert_eq!(identity, block.publication_identity());
    assert_ne!(identity, foreign.block().publication_identity());
    block.insert(1, 99);
    assert!(block.belongs_to(&target));
    assert_eq!(identity, block.publication_identity());
    drop(block);
    assert_eq!(identity, target.block().publication_identity());
    assert_ne!(identity, target.block_and_revert().publication_identity());
    target.block().commit();
    assert_eq!(target.view().get(&1), Some(&10));
    assert_ne!(identity, target.block().publication_identity());
}

fn detach<K: Key, V: Value>(block: Block<'_, K, V>) -> Detached<K, V, ()> {
    block.try_detach(|_| Ok::<_, ()>(())).unwrap()
}

fn prepare<'a, K: Key, V: Value, A>(
    journal: Detached<K, V, A>,
    target: &'a Storage<K, V>,
) -> PreparedPublication<'a, K, V, A, ()> {
    match journal.try_prepare_publication(target, |_, _| Ok::<_, ()>(())) {
        Ok(prepared) => prepared,
        Err((_, error)) => panic!("unexpected preparation refusal: {error:?}"),
    }
}

fn values(storage: &Storage<u64, u64>) -> Vec<(u64, u64)> {
    storage.view().iter().map(|(k, v)| (*k, *v)).collect()
}

fn assert_same(actual: &Storage<u64, u64>, expected: &Storage<u64, u64>) {
    assert_eq!(values(actual), values(expected));
    assert_eq!(
        actual.snapshot().revert_map(),
        expected.snapshot().revert_map()
    );
}

#[test]
fn prepared_delta_matches_direct_commit_and_preserves_existing_readers() {
    let target: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let reference: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
    let reader = target.snapshot();
    let observer = detach(target.block());
    let mut block = target.block();
    let mut direct = reference.block();
    for candidate in [&mut block, &mut direct] {
        let mut child = candidate.transaction();
        child.insert(1, 11);
        child.remove(2);
        child.insert(3, 30);
        child.remove(99);
        child.apply();
    }
    direct.commit();
    let prepared = prepare(detach(block), &target);
    assert!(target.revert.try_write().is_none());
    assert!(target.blocks.try_write().is_none());
    assert_eq!(values(&target), [(1, 10), (2, 20)]);
    assert!(observer.matches_current(&target));
    prepared.publish();
    assert_same(&target, &reference);
    assert!(!observer.matches_current(&target));
    assert_eq!(reader.current().get(&1), Some(&10));
    assert_eq!(reader.current().get(&2), Some(&20));
    assert!(reader.revert_map().is_empty());
}

#[test]
fn replacement_restores_discarded_tip_only_keys_and_candidate_undo() {
    for touch in [false, true] {
        let target: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
        let reference: Storage<_, _> = [(1, 10), (2, 20)].into_iter().collect();
        for storage in [&target, &reference] {
            let mut tip = storage.block();
            tip.insert(1, 11);
            tip.remove(2);
            tip.insert(3, 30);
            tip.commit();
        }
        let reader = target.snapshot();
        let mut candidate = target.block_and_revert();
        let mut direct = reference.block_and_revert();
        if touch {
            for block in [&mut candidate, &mut direct] {
                block.insert(1, 12);
                block.remove(88);
            }
        }
        let prepared = prepare(detach(candidate), &target);
        assert_eq!(values(&target), [(1, 11), (3, 30)]);
        direct.commit();
        prepared.publish();
        assert_same(&target, &reference);
        assert_eq!(target.view().get(&2), Some(&20));
        assert_eq!(target.view().get(&3), None);
        assert_eq!(reader.current().get(&1), Some(&11));
        target.block_and_revert().commit();
        reference.block_and_revert().commit();
        assert_same(&target, &reference);
    }
}

#[test]
fn untouched_noop_and_absent_touches_publish_exact_undo_transitions() {
    for case in 0..3 {
        let target: Storage<_, _> = [(1, 10)].into_iter().collect();
        let mut tip = target.block();
        tip.insert(1, 11);
        tip.commit();
        let observer = detach(target.block());
        let mut candidate = target.block();
        match case {
            1 => {
                let _ = candidate.get_mut(&1);
            }
            2 => {
                candidate.remove(99);
            }
            _ => {}
        }
        let expected = match case {
            1 => BTreeMap::from([(1, Some(11))]),
            2 => BTreeMap::from([(99, None)]),
            _ => BTreeMap::new(),
        };
        prepare(detach(candidate), &target).publish();
        assert_eq!(values(&target), [(1, 11)]);
        assert_eq!(target.snapshot().revert_map(), &expected);
        assert!(!observer.matches_current(&target));
    }
}

#[test]
fn busy_writers_return_same_journal_and_release_partial_acquisition() {
    let target: Storage<_, _> = [(1, 10)].into_iter().collect();
    let mut candidate = target.block();
    candidate.insert(1, 12);
    let mut journal = detach(candidate);
    let original = journal.entries.as_ptr();
    for which in 0..2 {
        let undo = (which == 0).then(|| target.revert.write());
        let current = (which == 1).then(|| target.blocks.write());
        let (returned, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .unwrap();
        assert!(matches!(error, PublicationPreparationError::Busy(_)));
        assert_eq!(returned.entries.as_ptr(), original);
        drop(current);
        drop(undo);
        assert!(target.revert.try_write().is_some());
        assert!(target.blocks.try_write().is_some());
        assert_eq!(values(&target), [(1, 10)]);
        journal = returned;
    }
    prepare(journal, &target).publish();
    assert_eq!(values(&target), [(1, 12)]);
}

#[test]
fn foreign_aba_and_admission_race_cannot_publish_a_stale_journal() {
    let mut target: Storage<_, _> = [(1, 10)].into_iter().collect();
    let foreign: Storage<_, _> = [(1, 10)].into_iter().collect();
    let journal = detach(target.block());
    let (journal, error) = journal
        .try_prepare_publication(&foreign, |_, _| -> Result<(), ()> {
            panic!("foreign owner admitted")
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    target.insert(1, 11);
    target.insert(1, 10);
    let (_, error) = journal
        .try_prepare_publication(&target, |_, _| -> Result<(), ()> {
            panic!("stale owner admitted")
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    let journal = detach(target.block());
    let (_, error) = journal
        .try_prepare_publication(&target, |_, owner| {
            owner.block().commit();
            Ok::<_, ()>(())
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    assert_eq!(values(&target), [(1, 10)]);
    assert!(target.revert.try_write().is_some());
    assert!(target.blocks.try_write().is_some());
}

#[test]
fn abort_keeps_original_owner_available_after_another_component_refuses() {
    let first: Storage<_, _> = [(1, 10)].into_iter().collect();
    let second: Storage<_, _> = [(1, 20)].into_iter().collect();
    let mut block = first.block();
    block.insert(1, 11);
    let journal = detach(block);
    let ptr = journal.entries.as_ptr();
    let prepared = prepare(journal, &first);
    let mut block = second.block();
    block.insert(1, 21);
    let journal2 = detach(block);
    let (_, error) = journal2
        .try_prepare_publication(&second, |_, _| Err::<(), _>("capacity"))
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Admission("capacity"));
    let journal = prepared.abort();
    assert_eq!(journal.entries.as_ptr(), ptr);
    assert_eq!(values(&first), [(1, 10)]);
    assert_eq!(values(&second), [(1, 20)]);
    assert!(journal.matches_current(&first));
    prepare(journal, &first).publish();
    assert_eq!(values(&first), [(1, 11)]);
}

#[test]
fn admission_precedes_cow_and_both_reservations_survive_publication() {
    #[derive(Debug)]
    struct Counted(Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(&self.0))
        }
    }
    struct Reservation(Arc<AtomicBool>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let copies = Arc::new(AtomicUsize::new(0));
    let target: Storage<u64, _> = [(1, Counted(Arc::clone(&copies)))].into_iter().collect();
    let mut block = target.block();
    let _ = block.get_mut(&1);
    let captured = Arc::new(AtomicBool::new(false));
    let journal = block
        .try_detach(|_| Ok::<_, ()>(Reservation(Arc::clone(&captured))))
        .unwrap();
    copies.store(0, Ordering::SeqCst);
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| Err::<(), _>("capacity"))
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Admission("capacity"));
    assert_eq!(copies.load(Ordering::SeqCst), 0);
    let installed = Arc::new(AtomicBool::new(false));
    let prepared = match journal.try_prepare_publication(&target, |_, _| {
        assert_eq!(copies.load(Ordering::SeqCst), 0);
        Ok::<_, ()>(Reservation(Arc::clone(&installed)))
    }) {
        Ok(p) => p,
        Err(_) => panic!("admission"),
    };
    assert!(copies.load(Ordering::SeqCst) > 0);
    let reservations = prepared.publish();
    assert!(!captured.load(Ordering::SeqCst));
    assert!(!installed.load(Ordering::SeqCst));
    assert!(target.revert.try_write().is_some());
    assert!(target.blocks.try_write().is_some());
    drop(reservations);
    assert!(captured.load(Ordering::SeqCst));
    assert!(installed.load(Ordering::SeqCst));
}
