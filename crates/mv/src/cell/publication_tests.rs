//! Exact detached cell publication under original writers and retained read views.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn detach<V: Value>(block: Block<'_, V>) -> Detached<V, ()> {
    block.try_detach(|_| Ok::<_, ()>(())).unwrap()
}
fn prepare<'a, V: Value, A>(
    journal: Detached<V, A>,
    target: &'a Cell<V>,
) -> PreparedPublication<'a, V, A, ()> {
    match journal.try_prepare_publication(target, |_, _| Ok::<_, ()>(())) {
        Ok(p) => p,
        Err((_, e)) => panic!("unexpected preparation refusal: {e:?}"),
    }
}

#[test]
fn prepared_cell_matches_direct_commit_without_changing_existing_readers() {
    let target = Cell::new(10_u64);
    let reference = Cell::new(10_u64);
    let reader = target.view();
    let undo = target.predecessor_view();
    let observer = detach(target.block());
    let mut candidate = target.block();
    let mut direct = reference.block();
    for block in [&mut candidate, &mut direct] {
        let mut child = block.transaction();
        *child.get_mut() = 20;
        child.apply();
        let mut aborted = block.transaction();
        *aborted.get_mut() = 99;
    }
    let prepared = prepare(detach(candidate), &target);
    assert!(target.revert.try_write().is_none());
    assert!(target.blocks.try_write().is_none());
    assert_eq!(*target.view(), 10);
    assert!(target.predecessor_view().is_none());
    assert!(observer.matches_current(&target));
    direct.commit();
    prepared.publish();
    assert_eq!(*target.view(), *reference.view());
    assert_eq!(*target.predecessor_view(), *reference.predecessor_view());
    assert_eq!(*reader, 10);
    assert_eq!(*undo, None);
    assert!(!observer.matches_current(&target));
}

#[test]
fn replacement_without_touch_and_with_touch_match_the_original_block_semantics() {
    for touched in [false, true] {
        let target = Cell::new(10_u64);
        let reference = Cell::new(10_u64);
        for cell in [&target, &reference] {
            let mut tip = cell.block();
            *tip.get_mut() = 20;
            tip.commit();
        }
        let reader = target.view();
        let undo = target.predecessor_view();
        let mut candidate = target.block_and_revert();
        let mut direct = reference.block_and_revert();
        if touched {
            *candidate.get_mut() = 30;
            *direct.get_mut() = 30;
        }
        let prepared = prepare(detach(candidate), &target);
        assert_eq!(*target.view(), 20);
        assert_eq!(*target.predecessor_view(), Some(10));
        direct.commit();
        prepared.publish();
        assert_eq!(*target.view(), *reference.view());
        assert_eq!(*target.predecessor_view(), *reference.predecessor_view());
        assert_eq!(*reader, 20);
        assert_eq!(*undo, Some(10));
        target.block_and_revert().commit();
        reference.block_and_revert().commit();
        assert_eq!(*target.view(), *reference.view());
        assert_eq!(*target.predecessor_view(), *reference.predecessor_view());
    }
}

#[test]
fn untouched_and_noop_publications_keep_distinct_undo_and_rotate_identity() {
    for touched in [false, true] {
        let target = Cell::new(10_u64);
        let mut tip = target.block();
        *tip.get_mut() = 20;
        tip.commit();
        let observer = detach(target.block());
        let mut block = target.block();
        if touched {
            let _ = block.get_mut();
        }
        prepare(detach(block), &target).publish();
        assert_eq!(*target.view(), 20);
        assert_eq!(*target.predecessor_view(), touched.then_some(20));
        assert!(!observer.matches_current(&target));
    }
}

#[test]
fn either_busy_writer_returns_original_values_and_releases_partial_locks() {
    let target = Cell::new(String::from("before"));
    let mut candidate = target.block();
    *candidate.get_mut() = String::from("after");
    let mut journal = detach(candidate);
    let ptr = journal.change.as_ref().unwrap().1.as_ptr();
    for which in 0..2 {
        let undo = (which == 0).then(|| target.revert.write());
        let current = (which == 1).then(|| target.blocks.write());
        let (returned, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .unwrap();
        assert_eq!(error, PublicationPreparationError::Busy);
        assert_eq!(returned.change.as_ref().unwrap().1.as_ptr(), ptr);
        drop(current);
        drop(undo);
        assert!(target.revert.try_write().is_some());
        assert!(target.blocks.try_write().is_some());
        journal = returned;
    }
    prepare(journal, &target).publish();
    assert_eq!(&*target.view(), "after");
    assert_eq!(target.predecessor_view().as_deref(), Some("before"));
}

#[test]
fn foreign_equal_pair_aba_and_change_during_admission_are_refused() {
    let target = Cell::new(10_u64);
    let foreign = Cell::new(10_u64);
    let journal = detach(target.block());
    let (journal, error) = journal
        .try_prepare_publication(&foreign, |_, _| -> Result<(), ()> {
            panic!("foreign admission")
        })
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Changed);
    target.replace_current_preserving_predecessor(11);
    target.replace_current_preserving_predecessor(10);
    let (_, error) = journal
        .try_prepare_publication(&target, |_, _| -> Result<(), ()> {
            panic!("ABA admission")
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
    assert_eq!(*target.view(), 10);
    assert!(target.revert.try_write().is_some());
    assert!(target.blocks.try_write().is_some());
}

#[test]
fn abort_preserves_the_exact_journal_for_retry_after_late_component_refusal() {
    let first = Cell::new(String::from("before"));
    let second = Cell::new(1_u64);
    let mut candidate = first.block();
    *candidate.get_mut() = String::from("after");
    let journal = detach(candidate);
    let ptr = journal.change.as_ref().unwrap().1.as_ptr();
    let prepared = prepare(journal, &first);
    let (_, error) = detach(second.block())
        .try_prepare_publication(&second, |_, _| Err::<(), _>("capacity"))
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Admission("capacity"));
    let journal = prepared.abort();
    assert_eq!(journal.change.as_ref().unwrap().1.as_ptr(), ptr);
    assert!(journal.matches_current(&first));
    assert_eq!(&*first.view(), "before");
    prepare(journal, &first).publish();
    assert_eq!(&*first.view(), "after");
}

#[test]
fn admission_precedes_value_copies_and_publication_transfers_resource_owners() {
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
    let target = Cell::new(Counted(Arc::clone(&copies)));
    let mut block = target.block();
    let _ = block.get_mut();
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

#[test]
fn dropping_preparation_releases_writers_before_installation_capacity() {
    struct Release<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Release<F> {
        fn drop(&mut self) {
            self.0.take().unwrap()();
        }
    }
    let target = Cell::new(10_u64);
    let mut candidate = target.block();
    *candidate.get_mut() = 20;
    let released = AtomicBool::new(false);
    let prepared = match detach(candidate).try_prepare_publication(&target, |_, _| {
        Ok::<_, ()>(Release(Some(|| {
            assert!(target.revert.try_write().is_some());
            assert!(target.blocks.try_write().is_some());
            released.store(true, Ordering::SeqCst);
        })))
    }) {
        Ok(p) => p,
        Err(_) => panic!("admission"),
    };
    drop(prepared);
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(*target.view(), 10);
    assert_eq!(*target.predecessor_view(), None);
}
