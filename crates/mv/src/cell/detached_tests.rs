//! Owned cell deltas release their writers without losing exact undo identity.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Reservation(Arc<AtomicBool>);
impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn detach<V: Value>(block: Block<'_, V>) -> Detached<V, ()> {
    block.try_detach(|_| Ok::<_, ()>(())).unwrap()
}

fn writers_are_free<V: Value>(cell: &Cell<V>) {
    assert!(cell.revert.try_write().is_some());
    assert!(cell.blocks.try_write().is_some());
}

#[test]
fn ordinary_capture_preserves_touch_and_reservation_without_publishing() {
    let cell = Cell::new(10_u64);
    let mut block = cell.block();
    {
        let mut child = block.transaction();
        *child.get_mut() = 20;
        child.apply();
    }
    {
        let mut aborted = block.transaction();
        *aborted.get_mut() = 90;
    }
    let released = Arc::new(AtomicBool::new(false));
    let journal = block
        .try_detach(|original| {
            assert_eq!(original.mode(), BlockMode::Ordinary);
            assert_eq!(*original.get_before_block(), 10);
            assert_eq!(*original.get(), 20);
            Ok::<_, ()>(Reservation(Arc::clone(&released)))
        })
        .unwrap();
    assert!(!journal.admission().0.load(Ordering::SeqCst));
    assert_eq!(journal.mode(), BlockMode::Ordinary);
    assert!(journal.is_dirty());
    let change = journal.touched_value().unwrap();
    assert_eq!((*change.before, *change.after), (10, 20));
    assert!(journal.matches_current(&cell));
    assert!(!journal.matches_current(&Cell::new(10)));
    writers_are_free(&cell);
    assert_eq!(*cell.view(), 10);
    assert!(cell.predecessor_view().is_none());
    drop(journal);
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(*cell.view(), 10);
    assert!(cell.predecessor_view().is_none());
}

#[test]
fn replacement_capture_keeps_undo_mode_and_discarded_tip_only_changes() {
    let cell = Cell::new(10_u64);
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    let published = cell.view();
    let undo = cell.predecessor_view();
    let unchanged = detach(cell.block_and_revert());
    assert_eq!(unchanged.mode(), BlockMode::Replace);
    assert!(unchanged.is_dirty());
    assert!(unchanged.touched_value().is_none());
    assert!(unchanged.matches_current(&cell));
    assert!(!unchanged.matches_block_predecessor(&cell.block()));
    let mut replacement = cell.block_and_revert();
    assert!(unchanged.matches_block_predecessor(&replacement));
    assert_eq!(
        *replacement, 10,
        "the mode includes the discarded tip's only change"
    );
    *replacement.get_mut() = 30;
    let journal = detach(replacement);
    let change = journal.touched_value().unwrap();
    assert_eq!((*change.before, *change.after), (10, 30));
    writers_are_free(&cell);
    assert_eq!(*cell.view(), 20);
    assert_eq!(*cell.predecessor_view(), Some(10));
    drop(journal);
    assert!(
        cell.block_and_revert()
            .try_detach(|_| Err::<(), _>("capacity"))
            .is_err()
    );
    writers_are_free(&cell);
    assert_eq!(*cell.view(), 20);
    assert_eq!(*cell.predecessor_view(), Some(10));
    assert_eq!(*published, 20);
    assert_eq!(*undo, Some(10));
    cell.block_and_revert().commit();
    assert_eq!(*cell.view(), 10);
    assert!(cell.predecessor_view().is_none());
    assert!(!unchanged.matches_current(&cell));
}

#[test]
fn untouched_noop_and_aborted_cells_keep_distinct_journals_and_exact_pair_versions() {
    let cell = Cell::new(10_u64);
    let untouched = detach(cell.block());
    assert!(!untouched.is_dirty());
    assert!(untouched.touched_value().is_none());
    let mut candidate = cell.block();
    {
        let mut aborted = candidate.transaction();
        *aborted.get_mut() = 99;
    }
    let aborted = detach(candidate);
    assert!(aborted.touched_value().is_none());
    assert!(!aborted.is_dirty());
    let mut candidate = cell.block();
    let _ = candidate.get_mut();
    let noop = detach(candidate);
    assert!(noop.is_dirty());
    assert_eq!(*noop.touched_value().unwrap().before, 10);
    assert_eq!(*noop.touched_value().unwrap().after, 10);
    assert!(untouched.matches_current(&cell));
    assert!(aborted.matches_current(&cell));
    assert!(noop.matches_current(&cell));
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    let before_undo_clear = detach(cell.block());
    cell.block().commit();
    assert_eq!(*cell.view(), 20);
    assert!(cell.predecessor_view().is_none());
    assert!(
        !before_undo_clear.matches_current(&cell),
        "undo-only publication invalidates the pair"
    );
    let before_aba = detach(cell.block());
    cell.replace_current_preserving_predecessor(21);
    cell.replace_current_preserving_predecessor(20);
    assert!(
        !before_aba.matches_current(&cell),
        "returning to equal values does not restore identity"
    );
    assert!(!untouched.matches_current(&cell));
}

#[test]
fn admission_refusal_releases_writers_before_any_final_value_copy() {
    struct Counted(Arc<AtomicUsize>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(&self.0))
        }
    }
    let copies = Arc::new(AtomicUsize::new(0));
    let cell = Cell::new(Counted(Arc::clone(&copies)));
    let mut block = cell.block();
    let _ = block.get_mut();
    copies.store(0, Ordering::SeqCst);
    let result = block.try_detach(|original| {
        assert!(original.touched_value().is_some());
        Err::<(), _>("capacity")
    });
    assert!(matches!(result, Err("capacity")));
    assert_eq!(copies.load(Ordering::SeqCst), 0);
    writers_are_free(&cell);
    assert!(cell.predecessor_view().is_none());
    let mut block = cell.block();
    let _ = block.get_mut();
    copies.store(0, Ordering::SeqCst);
    let journal = detach(block);
    assert_eq!(
        copies.load(Ordering::SeqCst),
        0,
        "touched capture retains the original successor allocations"
    );
    drop(journal);
    let block = cell.block();
    copies.store(0, Ordering::SeqCst);
    let journal = detach(block);
    assert_eq!(
        copies.load(Ordering::SeqCst),
        0,
        "untouched capture retains original successors without copying"
    );
    drop(journal);
}

#[test]
fn independently_prepared_cells_are_send_and_do_not_hold_each_others_writers() {
    fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    assert_send_sync_static::<Detached<u64, ()>>();
    let cell = Arc::new(Cell::new(10_u64));
    let worker = Arc::clone(&cell);
    let first = std::thread::spawn(move || {
        let mut block = worker.block();
        *block.get_mut() = 11;
        detach(block)
    })
    .join()
    .unwrap();
    writers_are_free(&cell);
    let worker = Arc::clone(&cell);
    let second = std::thread::spawn(move || {
        let mut block = worker.block();
        *block.get_mut() = 12;
        detach(block)
    })
    .join()
    .unwrap();
    assert_eq!(*first.touched_value().unwrap().after, 11);
    assert_eq!(*second.touched_value().unwrap().after, 12);
    assert!(first.matches_current(&cell) && second.matches_current(&cell));
    assert_eq!(*cell.view(), 10);
    drop((first, second));
    writers_are_free(&cell);
}

#[test]
fn json_restore_preserves_values_and_undo_but_mints_a_distinct_owner() {
    let cell = Cell::new(10_u64);
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    let journal = detach(cell.block_and_revert());
    let encoded = norito::json::to_json(&cell).unwrap();
    let restored: Cell<u64> = norito::json::from_json(&encoded).unwrap();
    assert_eq!(norito::json::to_json(&restored).unwrap(), encoded);
    assert_eq!(*restored.view(), 20);
    assert_eq!(*restored.predecessor_view(), Some(10));
    assert!(!journal.matches_current(&restored));
    assert!(journal.matches_current(&cell));
}

#[test]
fn detached_values_outlive_the_cell_without_a_reader_pin() {
    let journal = {
        let cell = Cell::new(String::from("before"));
        let mut block = cell.block();
        *block.get_mut() = String::from("after");
        detach(block)
    };
    let change = journal.touched_value().unwrap();
    assert_eq!(change.before, "before");
    assert_eq!(change.after, "after");
    assert!(!journal.matches_current(&Cell::new(String::from("before"))));
}
