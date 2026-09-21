//! Exact detached cell publication under original writers and retained read views.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[test]
fn block_identity_binds_original_owner_predecessor_and_mode_without_reading_values() {
    let target = Cell::new(10_u64);
    let foreign = Cell::new(10_u64);
    let mut block = target.block();
    let identity = block.publication_identity();
    assert!(block.belongs_to(&target));
    assert!(!block.belongs_to(&foreign));
    assert_eq!(identity, block.publication_identity());
    assert_ne!(identity, foreign.block().publication_identity());
    *block.get_mut() = 99;
    assert!(block.belongs_to(&target));
    assert_eq!(identity, block.publication_identity());
    drop(block);
    assert_eq!(identity, target.block().publication_identity());
    assert_ne!(identity, target.block_and_revert().publication_identity());
    target.block().commit();
    assert_eq!(*target.view(), 10);
    assert_ne!(identity, target.block().publication_identity());
}

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
    let ptr = journal.touched_value().unwrap().after.as_ptr();
    for which in 0..2 {
        let undo = (which == 0).then(|| target.revert.write());
        let current = (which == 1).then(|| target.blocks.write());
        let (returned, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .unwrap();
        assert!(matches!(error, PublicationPreparationError::Busy(_)));
        assert_eq!(returned.touched_value().unwrap().after.as_ptr(), ptr);
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
    let ptr = journal.touched_value().unwrap().after.as_ptr();
    let prepared = prepare(journal, &first);
    let (_, error) = detach(second.block())
        .try_prepare_publication(&second, |_, _| Err::<(), _>("capacity"))
        .err()
        .unwrap();
    assert_eq!(error, PublicationPreparationError::Admission("capacity"));
    let journal = prepared.abort();
    assert_eq!(journal.touched_value().unwrap().after.as_ptr(), ptr);
    assert!(journal.matches_current(&first));
    assert_eq!(&*first.view(), "before");
    prepare(journal, &first).publish();
    assert_eq!(&*first.view(), "after");
}

#[test]
fn original_successors_are_reused_without_value_copies_and_keep_resource_owners() {
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
    assert_eq!(
        copies.load(Ordering::SeqCst),
        0,
        "installation reuses the original successors"
    );
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

#[test]
fn cell_release_wakes_follow_both_values_identity_and_physical_unlocks() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Wake, Waker},
    };
    struct Probe {
        cell: Arc<Cell<u64>>,
        predecessor: CapturedPublication,
        current: u64,
        undo: Option<u64>,
        wakes: AtomicUsize,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            assert_eq!(
                self.predecessor
                    .try_check_current::<()>(&self.cell.publication),
                Err(PublicationPreparationError::Changed)
            );
            let current = self.cell.blocks.try_write().expect("current unlocked");
            let undo = self.cell.revert.try_write().expect("undo unlocked");
            assert_eq!(*current, self.current);
            assert_eq!(*undo, self.undo);
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn check(probe: Arc<Probe>, publish: impl FnOnce()) {
        let mut current = probe.cell.blocks_released.observe().wait_for_release();
        let mut undo = probe.cell.revert_released.observe().wait_for_release();
        let waker = Waker::from(Arc::clone(&probe));
        for wait in [&mut current, &mut undo] {
            assert!(
                Pin::new(wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        publish();
        assert_eq!(probe.wakes.load(Ordering::SeqCst), 2);
        for wait in [&mut current, &mut undo] {
            assert!(
                Pin::new(wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
            );
        }
    }
    for finish in 0..3 {
        for dirty in [false, true] {
            if finish == 2 && !dirty {
                continue;
            }
            let cell = Arc::new(Cell::new(10_u64));
            let mut tip = cell.block();
            *tip.get_mut() = 20;
            tip.commit();
            let probe = Arc::new(Probe {
                predecessor: cell.publication.capture(),
                cell: Arc::clone(&cell),
                current: if dirty { 30 } else { 20 },
                undo: if finish == 2 {
                    Some(10)
                } else {
                    dirty.then_some(20)
                },
                wakes: AtomicUsize::new(0),
            });
            if finish == 2 {
                let replacement = cell.current_replacement();
                check(probe, || replacement.publish(30));
            } else {
                let mut block = cell.block();
                if dirty {
                    *block.get_mut() = 30;
                }
                if finish == 1 {
                    let prepared = prepare(detach(block), &cell);
                    check(probe, || {
                        prepared.publish();
                    });
                } else {
                    check(probe, || block.commit());
                }
            }
        }
    }
}

#[test]
fn unchanged_cell_cleanup_panic_preserves_published_pair_and_healthy_contention() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Charge(Arc<AtomicBool>);
    impl Drop for Charge {
        fn drop(&mut self) {
            assert!(
                !self.0.swap(false, Ordering::SeqCst),
                "retired unchanged current"
            );
        }
    }
    let quiet = || Charge(Arc::new(AtomicBool::new(false)));
    for prepared in [false, true] {
        let cell = Cell::new_charged(10_u64, CellAllocationCharges::new(quiet(), quiet()));
        let predecessor = cell.publication.capture();
        let current_cleanup = Arc::new(AtomicBool::new(false));
        let block = cell.block_charged(CellAllocationCharges::new(
            Charge(Arc::clone(&current_cleanup)),
            quiet(),
        ));
        let result = if prepared {
            let original = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            let publisher = original
                .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
                .unwrap_or_else(|_| panic!("same original cell"));
            current_cleanup.store(true, Ordering::SeqCst);
            catch_unwind(AssertUnwindSafe(|| {
                publisher.publish();
            }))
        } else {
            current_cleanup.store(true, Ordering::SeqCst);
            catch_unwind(AssertUnwindSafe(|| block.commit()))
        };
        assert!(result.is_err());
        assert!(!current_cleanup.load(Ordering::SeqCst));
        assert_eq!(*cell.view(), 10);
        assert_eq!(*cell.predecessor_view(), None);
        assert_eq!(
            predecessor.try_check_current::<()>(&cell.publication),
            Err(PublicationPreparationError::Changed)
        );
        assert!(!cell.blocks_released.observe().is_poisoned());
        assert!(!cell.revert_released.observe().is_poisoned());
        let journal = cell
            .block_charged(CellAllocationCharges::new(quiet(), quiet()))
            .try_detach(|_| Ok::<_, ()>(()))
            .unwrap();
        let held = cell.block_charged(CellAllocationCharges::new(quiet(), quiet()));
        let expected = cell.revert_released.observe();
        let (journal, error) = journal
            .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("original undo held");
        assert_eq!(error, PublicationPreparationError::Busy(expected));
        drop(held);
        let retry = journal
            .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!("healthy original cell retry"));
        retry.publish();
        assert_eq!(*cell.view(), 10);
    }
}
