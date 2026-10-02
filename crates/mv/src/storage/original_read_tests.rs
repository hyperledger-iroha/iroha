//! Exact original map-pair ownership and publication race controls.

use super::*;

#[test]
fn committed_maps_preserve_previous_undo_after_ordinary_overlay_resets_it() {
    let storage: Storage<_, _> = [(1_u64, 10_u64), (2, 20)].into_iter().collect();
    let mut first = storage.block();
    first.insert(1, 11);
    first.remove(2);
    first.insert(3, 30);
    first.commit();
    let captured = storage.try_committed_view().unwrap();
    assert_eq!(captured.current().get(&1), Some(&11));
    assert_eq!(captured.current().get(&2), None);
    assert_eq!(captured.undo().get(&1), Some(&Some(10)));
    assert_eq!(captured.undo().get(&2), Some(&Some(20)));
    assert_eq!(captured.undo().get(&3), Some(&None));
    assert_eq!(captured.undo().get(&4), None);
    let mut ordinary = storage.block();
    assert_eq!(ordinary.touched_entries().count(), 0);
    assert!(captured.matches_block_source(&ordinary));
    ordinary.insert(1, 12);
    assert_eq!(ordinary.get_before_block(&1), Some(&11));
    assert_eq!(captured.undo().get(&1), Some(&Some(10)));
    assert_eq!(captured.current().get(&1), Some(&11));
    assert!(captured.matches_block_source(&ordinary));
    drop(ordinary);
    assert!(captured.same_publication(&storage.try_committed_view().unwrap()));
}

#[test]
fn replacement_overlay_does_not_relabel_the_committed_map_pair_as_reverted_state() {
    let storage: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let mut first = storage.block();
    first.insert(1, 11);
    first.insert(2, 22);
    first.commit();
    let captured = storage.try_committed_view().unwrap();
    let replacement = storage.block_and_revert();
    assert_eq!(replacement.mode(), BlockMode::Replace);
    assert_eq!(replacement.get(&1), Some(&10));
    assert_eq!(replacement.get(&2), None);
    assert!(captured.matches_block_source(&replacement));
    assert_eq!(captured.current().get(&1), Some(&11));
    assert_eq!(captured.current().get(&2), Some(&22));
    assert_eq!(captured.undo().get(&2), Some(&None));
}

#[test]
fn equal_foreign_maps_and_equal_republication_never_restore_original_source() {
    let storage: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let other: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let original = storage.try_committed_view().unwrap();
    assert!(!original.same_publication(&other.try_committed_view().unwrap()));
    assert!(!original.matches_block_source(&other.block()));
    storage.block().commit();
    assert_eq!(original.current().get(&1), storage.view().get(&1));
    assert!(!original.same_publication(&storage.try_committed_view().unwrap()));
    assert!(!original.matches_block_source(&storage.block()));
}

#[test]
fn any_publication_between_identity_observations_rejects_mixed_or_equal_reads() {
    let storage: Storage<_, _> = [(1_u64, 10_u64)].into_iter().collect();
    let original = storage
        .publication
        .try_capture_reads::<std::convert::Infallible>(|| true)
        .unwrap();
    let current = storage.blocks.read();
    let mut first = storage.block();
    first.insert(1, 11);
    first.commit();
    assert!(matches!(
        storage.finish_committed_reads(original, current, storage.revert.read()),
        Err(PublicationPreparationError::Changed)
    ));
    let original = storage
        .publication
        .try_capture_reads::<std::convert::Infallible>(|| true)
        .unwrap();
    let current = storage.blocks.read();
    let undo = storage.revert.read();
    storage.block().commit();
    assert!(matches!(
        storage.finish_committed_reads(original, current, undo),
        Err(PublicationPreparationError::Changed)
    ));
}

#[test]
fn concurrent_map_publication_returns_only_complete_original_current_undo_pairs() {
    let storage: Storage<_, _> = [(1_u64, 0_u64)].into_iter().collect();
    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            for value in 1..=128 {
                let mut block = storage.block();
                block.insert(1, value);
                block.commit();
            }
        });
        for _ in 0..256 {
            match storage.try_committed_view() {
                Ok(pair) => {
                    let value = *pair.current().get(&1).unwrap();
                    assert_eq!(pair.current().len(), 1);
                    if value == 0 {
                        assert!(pair.undo().is_empty());
                    } else {
                        assert_eq!(pair.undo().len(), 1);
                        assert_eq!(pair.undo().get(&1), Some(&Some(value - 1)));
                    }
                }
                Err(
                    PublicationPreparationError::Changed | PublicationPreparationError::Busy(_),
                ) => {}
                Err(error) => panic!("unexpected committed read refusal: {error:?}"),
            }
        }
        writer.join().unwrap();
    });
    let pair = storage.try_committed_view().unwrap();
    assert_eq!(pair.current().get(&1), Some(&128));
    assert_eq!(pair.undo().get(&1), Some(&Some(127)));
}

#[test]
fn map_source_identity_remains_unique_for_zero_sized_keys_and_values() {
    let storage: Storage<(), ()> = [((), ())].into_iter().collect();
    let other: Storage<(), ()> = [((), ())].into_iter().collect();
    let original = storage.try_committed_view().unwrap();
    assert_eq!(original.current().get(&()), Some(&()));
    assert!(!original.same_publication(&other.try_committed_view().unwrap()));
    assert!(!original.matches_block_source(&other.block()));
    storage.block().commit();
    assert!(!original.same_publication(&storage.try_committed_view().unwrap()));
}

#[test]
fn committed_map_readers_do_not_acquire_or_retain_the_physical_writers() {
    let storage: Storage<_, _> = [(1_u64, String::from("original"))].into_iter().collect();
    let current = storage.blocks.write();
    let undo = storage.revert.write();
    let captured = storage.try_committed_view().unwrap();
    let pointer = captured.current().get(&1).unwrap().as_ptr();
    assert_eq!(pointer, storage.view().get(&1).unwrap().as_ptr());
    drop((current, undo));
    assert!(storage.blocks.try_write().is_some());
    assert!(storage.revert.try_write().is_some());
    assert_eq!(captured.current().get(&1).unwrap().as_ptr(), pointer);
}
