//! Exact optional original values survive snapshot restoration and real revert.

use super::*;
use crate::BlockMode;

fn optional_cell() -> Cell<Option<u64>> {
    let cell = Cell::new(None);
    let mut block = cell.block();
    *block.get_mut() = Some(17);
    assert_eq!(block.original_undo(), &Some(None));
    block.commit();
    cell
}

fn optional_storage() -> Storage<String, Option<u64>> {
    let storage = Storage::from_iter([
        (String::from("present-null"), None),
        (String::from("removed-null"), None),
        (String::from("untouched-null"), None),
    ]);
    let mut block = storage.block();
    block.insert(String::from("present-null"), Some(17));
    block.remove(String::from("removed-null"));
    block.insert(String::from("prior-absence"), Some(23));
    block.remove(String::from("still-absent"));
    assert_eq!(block.revert_map().get("present-null"), Some(&Some(None)));
    assert_eq!(block.revert_map().get("removed-null"), Some(&Some(None)));
    assert_eq!(block.revert_map().get("prior-absence"), Some(&None));
    assert_eq!(block.revert_map().get("still-absent"), Some(&None));
    assert_eq!(block.revert_map().get("untouched-null"), None);
    block.commit();
    storage
}

#[test]
fn optional_cell_roundtrip_retains_same_original_null_predecessor_and_actual_revert() {
    let original = optional_cell();
    let retained_current = original.view();
    let retained_undo = original.predecessor_view();
    let current_pointer = retained_current.get() as *const Option<u64>;
    let undo_pointer = retained_undo.get() as *const Option<Option<u64>>;
    let encoded = json::to_json(&original).unwrap();
    let restored: Cell<Option<u64>> = json::from_json(&encoded).unwrap();
    assert_eq!(restored.view().get(), retained_current.get());
    assert_eq!(restored.predecessor_view().get(), retained_undo.get());
    assert_eq!(original.block_and_revert().get(), &None);
    assert_eq!(restored.block_and_revert().get(), &None);
    assert_eq!(retained_current.get() as *const _, current_pointer);
    assert_eq!(retained_undo.get() as *const _, undo_pointer);
    assert_eq!(original.view().get(), &Some(17));
    assert_eq!(original.predecessor_view().get(), &Some(None));
}

#[test]
fn optional_cell_present_null_and_absent_undo_have_different_snapshot_bytes() {
    let original = optional_cell();
    let untouched = Cell::new(Some(17_u64));
    assert_eq!(original.view().get(), untouched.view().get());
    assert_eq!(original.predecessor_view().get(), &Some(None));
    assert_eq!(untouched.predecessor_view().get(), &None);
    assert_ne!(
        json::to_json(&original).unwrap(),
        json::to_json(&untouched).unwrap()
    );
}

#[test]
fn optional_storage_roundtrip_retains_present_null_absence_and_untouched_revert() {
    let original = optional_storage();
    let retained = original.snapshot();
    let current_pointer = retained.current().get("present-null").unwrap() as *const Option<u64>;
    let undo_pointer =
        retained.revert_map().get("present-null").unwrap() as *const Option<Option<u64>>;
    let encoded = json::to_json(&original).unwrap();
    let restored: Storage<String, Option<u64>> = json::from_json(&encoded).unwrap();
    let restored_snapshot = restored.snapshot();
    assert_eq!(
        restored_snapshot.current().get("present-null"),
        Some(&Some(17))
    );
    assert_eq!(
        restored_snapshot.revert_map().get("present-null"),
        Some(&Some(None))
    );
    assert_eq!(
        restored_snapshot.revert_map().get("removed-null"),
        Some(&Some(None))
    );
    assert_eq!(
        restored_snapshot.revert_map().get("prior-absence"),
        Some(&None)
    );
    assert_eq!(
        restored_snapshot.revert_map().get("still-absent"),
        Some(&None)
    );
    assert_eq!(restored_snapshot.revert_map().get("untouched-null"), None);
    let previous = restored.block_and_revert();
    let original_previous = original.block_and_revert();
    assert_eq!(previous.get("present-null"), Some(&None));
    assert_eq!(previous.get("removed-null"), Some(&None));
    assert_eq!(previous.get("untouched-null"), Some(&None));
    assert_eq!(previous.get("prior-absence"), None);
    assert_eq!(previous.get("still-absent"), None);
    assert_eq!(
        previous.iter().collect::<Vec<_>>(),
        original_previous.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        retained.current().get("present-null").unwrap() as *const _,
        current_pointer
    );
    assert_eq!(
        retained.revert_map().get("present-null").unwrap() as *const _,
        undo_pointer
    );
    assert_eq!(original.view().get("present-null"), Some(&Some(17)));
}

#[test]
fn attached_and_detached_optional_originals_keep_exact_images_modes_and_publication() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        for changed in [false, true] {
            let cell = optional_cell();
            let storage = optional_storage();
            let mut cell_block = if mode == BlockMode::Ordinary {
                cell.block()
            } else {
                cell.block_and_revert()
            };
            let mut map_block = if mode == BlockMode::Ordinary {
                storage.block()
            } else {
                storage.block_and_revert()
            };
            if changed {
                *cell_block.get_mut() = None;
                map_block.insert(String::from("present-null"), None);
                map_block.remove(String::from("untouched-null"));
            }
            let original_cell_current = *cell_block.get();
            let original_cell_undo = *cell_block.original_undo();
            let original_map_current = map_block
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect::<BTreeMap<_, _>>();
            let original_map_undo = map_block
                .revert_map()
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect::<BTreeMap<_, _>>();
            let cell_identity = cell_block.publication_identity();
            let map_identity = map_block.publication_identity();
            let cell_pointer = cell_block.get() as *const Option<u64>;
            let cell_undo_pointer = cell_block.original_undo() as *const Option<Option<u64>>;
            let cell_json = json::to_json(&cell_block).unwrap();
            let map_json = json::to_json(&map_block).unwrap();
            assert_eq!(cell_block.get() as *const _, cell_pointer);
            assert_eq!(cell_block.original_undo() as *const _, cell_undo_pointer);
            assert_eq!(cell_block.publication_identity(), cell_identity);
            assert_eq!(map_block.publication_identity(), map_identity);
            let frozen_cell = cell_block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            let frozen_map = map_block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            assert_eq!(frozen_cell.get() as *const _, cell_pointer);
            assert_eq!(frozen_cell.original_undo() as *const _, cell_undo_pointer);
            assert_eq!(frozen_cell.publication_identity(), cell_identity);
            assert_eq!(
                frozen_map.original_images().publication_identity(),
                map_identity
            );
            assert_eq!(json::to_json(&frozen_cell).unwrap(), cell_json);
            assert_eq!(json::to_json(&frozen_map).unwrap(), map_json);
            let restored_cell: Cell<Option<u64>> = json::from_json(&cell_json).unwrap();
            let restored_map: Storage<String, Option<u64>> = json::from_json(&map_json).unwrap();
            assert_eq!(*restored_cell.view().get(), original_cell_current);
            assert_eq!(*restored_cell.predecessor_view().get(), original_cell_undo);
            assert_eq!(
                restored_map
                    .view()
                    .iter()
                    .map(|(key, value)| (key.clone(), *value))
                    .collect::<BTreeMap<_, _>>(),
                original_map_current
            );
            assert_eq!(
                restored_map
                    .snapshot()
                    .revert_map()
                    .iter()
                    .map(|(key, value)| (key.clone(), *value))
                    .collect::<BTreeMap<_, _>>(),
                original_map_undo
            );
            assert!(!frozen_cell.matches_current(&restored_cell));
            assert!(!frozen_map.matches_current(&restored_map));
            let mut newer = cell.block();
            *newer.get_mut() = Some(99);
            newer.commit();
            let mut newer = storage.block();
            newer.insert(String::from("newer"), Some(99));
            newer.commit();
            assert_eq!(json::to_json(&frozen_cell).unwrap(), cell_json);
            assert_eq!(json::to_json(&frozen_map).unwrap(), map_json);
            assert_eq!(frozen_cell.publication_identity(), cell_identity);
            assert_eq!(
                frozen_map.original_images().publication_identity(),
                map_identity
            );
        }
    }
}

#[test]
fn optional_storage_projection_keeps_first_present_null_undo_and_real_commit_bytes() {
    let original = optional_storage();
    let equivalent = optional_storage();
    let block = original.block();
    let mut changed = equivalent.block();
    let changes = BTreeMap::from([
        (String::from("present-null"), Some(None)),
        (String::from("untouched-null"), Some(Some(41))),
        (String::from("new-null"), Some(None)),
        (String::from("missing"), None),
    ]);
    let original_bytes = json::to_json(&block).unwrap();
    let mut projected = String::new();
    json_serialize_storage_block_with_changes(&block, &changes, &mut projected);
    for (key, value) in changes {
        match value {
            Some(value) => {
                changed.insert(key, value);
            }
            None => {
                changed.remove(key);
            }
        }
    }
    assert_eq!(projected, json::to_json(&changed).unwrap());
    assert_eq!(json::to_json(&block).unwrap(), original_bytes);
    let exact_undo = changed
        .revert_map()
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect::<BTreeMap<_, _>>();
    changed.commit();
    assert_eq!(projected, json::to_json(&equivalent).unwrap());
    let restored: Storage<String, Option<u64>> = json::from_json(&projected).unwrap();
    assert_eq!(
        restored
            .snapshot()
            .revert_map()
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect::<BTreeMap<_, _>>(),
        exact_undo
    );
    assert_eq!(
        restored.block_and_revert().get("untouched-null"),
        Some(&None)
    );
}
