//! Exercise original MV fields across exclusive execution and publication phases.
use super::*;
use mv::storage::StorageReadOnly;
use mv::{BlockMode, cell::Cell, storage::Storage};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

struct Pair<'a> {
    cell: CellField<'a, u64>,
    map: StorageField<'a, u64, u64>,
}
impl Drop for Pair<'_> {
    fn drop(&mut self) {
        self.cell.release_writers();
        self.map.release_writers();
    }
}

#[test]
fn transfer_to_capture_retains_original_executing_value_and_mode() {
    let value = Arc::new(17_u64);
    let target = Cell::new(Arc::clone(&value));
    let mut field = BlockField::new(target.block());
    assert!(Arc::ptr_eq(field.get(), &value));
    let replacement = Arc::new(23);
    *field.get_mut() = Arc::clone(&replacement);
    let original = field.into_executing();
    assert_eq!(original.mode(), BlockMode::Ordinary);
    assert!(Arc::ptr_eq(original.get(), &replacement));
    original.commit();
    assert!(Arc::ptr_eq(target.view().get(), &replacement));
}

#[test]
fn publication_preserves_ordinary_replacement_and_untouched_history() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        for edit in [false, true] {
            let cell = Cell::new(10_u64);
            let map = Storage::<u64, u64>::new();
            let mut first = map.block();
            first.insert(1, 10);
            first.commit();
            let mut c = cell.block();
            *c.get_mut() = 20;
            c.commit();
            let mut m = map.block();
            m.insert(1, 20);
            m.commit();
            let old_cell = cell.view();
            let old_map = map.view();
            let mut pair = Pair {
                cell: BlockField::new(match mode {
                    BlockMode::Ordinary => cell.block(),
                    BlockMode::Replace => cell.block_and_revert(),
                }),
                map: BlockField::new(match mode {
                    BlockMode::Ordinary => map.block(),
                    BlockMode::Replace => map.block_and_revert(),
                }),
            };
            if edit {
                *pair.cell.get_mut() = 31;
                pair.map.insert(1, 31);
            }
            pair.cell.prepare_publication();
            pair.map.prepare_publication();
            // Preparation retains native reader mutexes. The original immutable
            // views remain readable without reacquiring a publication guard.
            assert_eq!(*old_cell, 20);
            assert_eq!(old_map.get(&1), Some(&20));
            assert!(catch_unwind(AssertUnwindSafe(|| pair.cell.get())).is_err());
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    pair.map.insert(2, 99);
                }))
                .is_err()
            );
            pair.cell.publish_prepared();
            pair.map.publish_prepared();
            let before = if mode == BlockMode::Replace { 10 } else { 20 };
            let after = if edit { 31 } else { before };
            assert_eq!(*cell.view(), after);
            assert_eq!(map.view().get(&1), Some(&after));
            assert_eq!(*old_cell, 20);
            assert_eq!(old_map.get(&1), Some(&20));
            drop(pair);
            assert_eq!(*cell.block_and_revert().get(), before);
            assert_eq!(map.block_and_revert().get(&1), Some(&before));
        }
    }
}

#[test]
fn released_field_never_regains_execution_or_publication_authority() {
    let target = Cell::new(11_u64);
    let mut field = BlockField::new(target.block());
    *field.get_mut() = 29;
    field.prepare_publication();
    field.release_writers();
    field.release_writers();
    assert!(catch_unwind(AssertUnwindSafe(|| field.get())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| field.prepare_publication())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| field.publish_prepared())).is_err());
    assert_eq!(*target.view(), 11);
    drop(field);
    assert_eq!(*target.block().get(), 11);
}

#[test]
fn preparation_unwind_abandons_both_original_journals_without_publication() {
    let cell = Cell::new(10_u64);
    let map = Storage::<u64, u64>::new();
    let mut pair = Pair {
        cell: BlockField::new(cell.block()),
        map: BlockField::new(map.block()),
    };
    *pair.cell.get_mut() = 29;
    pair.map.insert(1, 29);
    assert!(
        catch_unwind(AssertUnwindSafe(move || {
            let mut pair = pair;
            pair.cell.prepare_publication();
            panic!("later aggregate preparation failed before preparing its sibling");
        }))
        .is_err()
    );
    assert_eq!(*cell.view(), 10);
    assert!(map.view().is_empty());
}

#[test]
fn aggregate_refuses_a_partial_preparation_and_repeated_publication() {
    let mut phase = AggregatePublication::Executing;
    phase.assert_executing();
    phase.begin_preparation();
    assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_publication())).is_err());
    assert_eq!(phase, AggregatePublication::Preparing);
    phase.finish_preparation();
    phase.begin_publication();
    phase.finish_publication();
    assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_preparation())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_publication())).is_err());
    assert_eq!(phase, AggregatePublication::Published);
}

#[test]
fn aggregate_release_is_terminal_in_every_phase() {
    for mut phase in [
        AggregatePublication::Executing,
        AggregatePublication::Capturing,
        AggregatePublication::Frozen,
        AggregatePublication::Reacquiring,
        AggregatePublication::Preparing,
        AggregatePublication::Prepared,
        AggregatePublication::Publishing,
        AggregatePublication::Published,
        AggregatePublication::Released,
    ] {
        phase.release();
        phase.release();
        assert!(catch_unwind(AssertUnwindSafe(|| phase.assert_executing())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_preparation())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_publication())).is_err());
        assert_eq!(phase, AggregatePublication::Released);
    }
}

#[test]
fn wrapper_serializes_the_exact_original_cell_and_storage_layout() {
    let cell = Cell::new(17_u64);
    let map = Storage::<u64, u64>::new();
    let mut original_cell = cell.block();
    *original_cell.get_mut() = 23;
    let mut original_map = map.block();
    original_map.insert(1, 42);
    let mut expected_cell = String::new();
    let mut expected_map = String::new();
    json::JsonSerialize::json_serialize(&original_cell, &mut expected_cell);
    json::JsonSerialize::json_serialize(&original_map, &mut expected_map);
    let bounded_cell = json::to_json_bounded(&original_cell, 128);
    let bounded_map = json::to_json_bounded(&original_map, 128);
    let field_cell = BlockField::new(original_cell);
    let field_map = BlockField::new(original_map);
    let mut actual_cell = String::new();
    let mut actual_map = String::new();
    json::JsonSerialize::json_serialize(&field_cell, &mut actual_cell);
    json::JsonSerialize::json_serialize(&field_map, &mut actual_map);
    assert_eq!(actual_cell, expected_cell);
    assert_eq!(actual_map, expected_map);
    assert_eq!(json::to_json_bounded(&field_cell, 128), bounded_cell);
    assert_eq!(json::to_json_bounded(&field_map, 128), bounded_map);
}

#[test]
fn attached_owners_can_be_borrowed_for_a_shorter_execution_scope() {
    fn shorten<'long, 'short>(pair: Pair<'long>) -> Pair<'short>
    where
        'long: 'short,
    {
        pair
    }
    let cell = Cell::new(17_u64);
    let map = Storage::<u64, u64>::new();
    let pair = shorten(Pair {
        cell: BlockField::new(cell.block()),
        map: BlockField::new(map.block()),
    });
    assert_eq!(*pair.cell.get(), 17);
    assert!(pair.map.is_empty());
}

#[test]
fn borrowed_extraction_keeps_exact_original_writers_after_empty_shell_drops() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        let cell = Cell::new(String::from("initial"));
        let map = Storage::<u64, String>::from_iter([(1, "initial".into())]);
        let mut tip = cell.block();
        *tip.get_mut() = "tip".into();
        tip.commit();
        let mut tip = map.block();
        tip.insert(1, "tip".into());
        tip.commit();
        let cell_probe = cell.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let map_probe = map.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let mut cell_shell = BlockField::new(if mode == BlockMode::Ordinary {
            cell.block()
        } else {
            cell.block_and_revert()
        });
        let mut map_shell = BlockField::new(if mode == BlockMode::Ordinary {
            map.block()
        } else {
            map.block_and_revert()
        });
        *cell_shell.get_mut() = "original successor".into();
        map_shell.insert(1, "original successor".into());
        let cell_pointer = cell_shell.get().as_ptr();
        let map_pointer = map_shell.get(&1).unwrap().as_ptr();
        let cell_identity = cell_shell.publication_identity();
        let map_identity = map_shell.publication_identity();
        let original_cell = cell_shell.take_executing();
        let original_map = map_shell.take_executing();
        assert!(catch_unwind(AssertUnwindSafe(|| cell_shell.take_executing())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| map_shell.prepare_publication())).is_err());
        drop((cell_shell, map_shell));
        assert_eq!(original_cell.get().as_ptr(), cell_pointer);
        assert_eq!(original_map.get(&1).unwrap().as_ptr(), map_pointer);
        assert_eq!(original_cell.publication_identity(), cell_identity);
        assert_eq!(original_map.publication_identity(), map_identity);
        assert_eq!(original_cell.mode(), mode);
        assert_eq!(original_map.mode(), mode);
        let (cell_probe, error, cleanup) = cell_probe
            .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("empty shell drop does not release the moved Cell writer");
        assert!(matches!(error, mv::PublicationPreparationError::Busy(_)));
        drop(cleanup);
        let (map_probe, error, cleanup) = map_probe
            .try_prepare_publication(&map, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("empty shell drop does not release the moved Storage writer");
        assert!(matches!(error, mv::PublicationPreparationError::Busy(_)));
        drop(cleanup);
        assert_eq!(cell.view().get(), "tip");
        assert_eq!(map.view().get(&1).unwrap(), "tip");
        drop((original_cell, original_map));
        let cell_prepared = cell_probe
            .try_prepare_publication(&cell, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!("actual Cell writer drop releases its original authority"));
        let map_prepared = map_probe
            .try_prepare_publication(&map, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|_| {
                panic!("actual Storage writer drop releases its original authority")
            });
        drop((cell_prepared, map_prepared));
    }
}

#[test]
fn aggregate_freeze_retries_only_original_complete_phase() {
    let mut phase = AggregatePublication::Executing;
    phase.begin_freeze();
    assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_reacquisition())).is_err());
    phase.finish_freeze();
    phase.assert_frozen();
    for _ in 0..3 {
        phase.begin_reacquisition();
        assert!(catch_unwind(AssertUnwindSafe(|| phase.begin_publication())).is_err());
        phase.recover_reacquisition();
        phase.assert_frozen();
        assert!(catch_unwind(AssertUnwindSafe(|| phase.assert_executing())).is_err());
    }
    phase.begin_reacquisition();
    phase.finish_reacquisition();
    phase.recover_reacquisition();
    phase.begin_reacquisition();
    phase.finish_reacquisition();
    phase.begin_publication();
    assert!(catch_unwind(AssertUnwindSafe(|| phase.recover_reacquisition())).is_err());
    phase.finish_publication();
}
