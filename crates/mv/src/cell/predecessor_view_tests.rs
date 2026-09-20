//! Exact current/predecessor observations across actual MV publication and undo.

use super::Cell;

#[test]
fn predecessor_view_retains_the_published_pair_across_later_commits() {
    let cell = Cell::new(String::from("base"));
    assert!(cell.predecessor_view().is_none());
    let mut block = cell.block();
    *block.get_mut() = String::from("tip");
    block.commit();
    let current = cell.view();
    let predecessor = cell.predecessor_view();
    assert_eq!(current.as_str(), "tip");
    assert_eq!(predecessor.as_deref(), Some("base"));
    let mut next = cell.block();
    *next.get_mut() = String::from("next");
    next.commit();
    assert_eq!(current.as_str(), "tip");
    assert_eq!(predecessor.as_deref(), Some("base"));
    assert_eq!(cell.view().as_str(), "next");
    assert_eq!(cell.predecessor_view().as_deref(), Some("tip"));
}

#[test]
fn abandoned_replacement_preserves_published_undo_and_replacement_commits_base() {
    let cell = Cell::new(10_u64);
    let mut original = cell.block();
    *original.get_mut() = 20;
    original.commit();
    {
        let mut replacement = cell.block_and_revert();
        assert_eq!(*replacement.get(), 10);
        *replacement.get_mut() = 30;
    }
    assert_eq!(*cell.view().get(), 20);
    assert_eq!(*cell.predecessor_view().get(), Some(10));
    let mut replacement = cell.block_and_revert();
    assert_eq!(*replacement.get_before_block(), 10);
    *replacement.get_mut() = 40;
    replacement.commit();
    assert_eq!(*cell.view().get(), 40);
    assert_eq!(*cell.predecessor_view().get(), Some(10));
    cell.block().commit();
    assert_eq!(*cell.view().get(), 40);
    assert_eq!(*cell.predecessor_view().get(), None);
    let unchanged_replacement = cell.block_and_revert();
    assert_eq!(*unchanged_replacement.get_before_block(), 40);
    assert_eq!(*unchanged_replacement.get(), 40);
    drop(unchanged_replacement);
    assert_eq!(*cell.view(), 40);
}

#[test]
fn same_cut_replacement_preserves_absent_undo_and_retained_predecessor() {
    let cell = Cell::new(10_u64);
    cell.replace_current_preserving_predecessor(11);
    assert_eq!(*cell.view(), 11);
    assert!(cell.predecessor_view().is_none());
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    let retained_current = cell.view();
    let retained_predecessor = cell.predecessor_view();
    cell.replace_current_preserving_predecessor(21);
    assert_eq!(*retained_current, 20);
    assert_eq!(*retained_predecessor, Some(11));
    assert_eq!(*cell.view(), 21);
    assert_eq!(*cell.predecessor_view(), Some(11));
    assert_eq!(*cell.block_and_revert(), 11);
    assert_eq!(
        *cell.view(),
        21,
        "an abandoned replacement does not publish"
    );
}

#[test]
fn same_cut_owner_drop_preserves_values_identity_and_releases_both_writers() {
    let cell = Cell::new(10_u64);
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    let journal = cell.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let owner = cell.current_replacement();
    assert_eq!(*owner.get(), 20);
    assert_eq!(*cell.view(), 20);
    assert_eq!(*cell.predecessor_view(), Some(10));
    assert!(cell.revert.try_write().is_none());
    assert!(cell.blocks.try_write().is_none());
    assert!(journal.matches_current(&cell));
    drop(owner);
    assert!(cell.revert.try_write().is_some());
    assert!(cell.blocks.try_write().is_some());
    assert!(journal.matches_current(&cell));
    assert_eq!(*cell.view(), 20);
    assert_eq!(*cell.predecessor_view(), Some(10));
}

#[test]
fn same_cut_owner_publish_preserves_undo_and_retained_readers() {
    let cell = Cell::new(10_u64);
    let mut tip = cell.block();
    *tip.get_mut() = 20;
    tip.commit();
    let journal = cell.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let current = cell.view();
    let undo = cell.predecessor_view();
    let owner = cell.current_replacement();
    owner.publish(21);
    assert_eq!(*current, 20);
    assert_eq!(*undo, Some(10));
    assert_eq!(*cell.view(), 21);
    assert_eq!(*cell.predecessor_view(), Some(10));
    assert!(!journal.matches_current(&cell));
    assert_eq!(*cell.block_and_revert(), 10);
}
