//! Actual inline freeze phases and unchanged retained images across refusal/recovery.

use super::super::*;
use crate::test_allocations::allocations_during;
use mv::{
    BlockMode, PublicationPreparationError,
    storage::{Storage, StorageReadOnly},
};

#[test]
fn only_complete_frozen_phase_exposes_original_images_without_physical_allocation() {
    let target: Storage<u64, String> = [(1, "before".into())].into_iter().collect();
    let mut field = BlockField::new(target.block());
    field.insert(1, "after".into());
    let identity = field.publication_identity();
    let pointer = field.get(&1).unwrap().as_ptr();
    assert!(field.frozen_images().is_none());
    field.begin_freeze();
    assert!(field.frozen_images().is_none());
    field.finish_freeze();
    field.retire_frozen_cleanup();
    let allocations = allocations_during(|| {
        let images = field.frozen_images().expect("complete original freeze");
        assert_eq!(images.mode(), BlockMode::Ordinary);
        assert!(images.belongs_to(&target));
        assert_eq!(images.publication_identity(), identity);
        assert_eq!(images.current_entries().next().unwrap().1.as_ptr(), pointer);
        assert_eq!(
            images.undo_entries().next().unwrap().1.as_deref(),
            Some("before")
        );
    });
    assert_eq!(allocations, 0);
    field
        .begin_frozen_publication(|original| {
            Ok::<_, (mv::storage::Detached<u64, String, ()>, ())>(
                mv::storage::BlockPublicationSlot::from_frozen(original, &target),
            )
        })
        .unwrap();
    assert!(field.frozen_images().is_none());
    field.try_prepare_frozen_publication().unwrap();
    assert!(field.frozen_images().is_none());
    field.recover_frozen_publication();
    field.retire_frozen_cleanup();
    assert_eq!(
        field.frozen_images().unwrap().publication_identity(),
        identity
    );
    assert_eq!(
        field
            .frozen_images()
            .unwrap()
            .current_entries()
            .next()
            .unwrap()
            .1
            .as_ptr(),
        pointer
    );
    field.release_writers();
    assert!(field.frozen_images().is_none());
}

#[test]
fn real_replacement_deletion_and_noop_images_survive_equal_target_publication() {
    let target: Storage<u64, String> =
        [(1, "base"), (2, "removed"), (3, "equal"), (8, "untouched")]
            .into_iter()
            .map(|(key, value)| (key, value.into()))
            .collect();
    let mut tip = target.block();
    tip.insert(1, "tip".into());
    tip.commit();
    let mut field = BlockField::new(target.block_and_revert());
    field.insert(1, "replacement".into());
    field.remove(2);
    field.insert(3, "equal".into());
    field.remove(4);
    field.insert(5, "new".into());
    let identity = field.publication_identity();
    field.begin_freeze();
    field.finish_freeze();
    field.retire_frozen_cleanup();
    let check = |field: &StorageField<'_, u64, String>| {
        let images = field.frozen_images().unwrap();
        assert_eq!(images.mode(), BlockMode::Replace);
        assert_eq!(images.publication_identity(), identity);
        assert_eq!(
            images
                .current_entries()
                .map(|(k, v)| (*k, v.as_str()))
                .collect::<Vec<_>>(),
            [
                (1, "replacement"),
                (3, "equal"),
                (5, "new"),
                (8, "untouched")
            ]
        );
        assert_eq!(
            images
                .undo_entries()
                .map(|(k, v)| (*k, v.as_deref()))
                .collect::<Vec<_>>(),
            [
                (1, Some("base")),
                (2, Some("removed")),
                (3, Some("equal")),
                (4, None),
                (5, None)
            ]
        );
    };
    check(&field);
    target.block().commit();
    check(&field);
    field
        .begin_frozen_publication(|original| {
            Ok::<_, (mv::storage::Detached<u64, String, ()>, ())>(
                mv::storage::BlockPublicationSlot::from_frozen(original, &target),
            )
        })
        .unwrap();
    assert_eq!(
        field.try_prepare_frozen_publication(),
        Err(PublicationPreparationError::Changed)
    );
    assert!(field.frozen_images().is_none());
    field.recover_frozen_publication();
    field.retire_frozen_cleanup();
    check(&field);
    assert_eq!(target.view().get(&1).map(String::as_str), Some("tip"));
}
