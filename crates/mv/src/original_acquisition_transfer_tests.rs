//! Borrowed extraction retains exact original slots through precondition failure.

use crate::{
    BlockAcquisition, BlockMode,
    cell::Cell,
    storage::{Storage, StorageReadOnly},
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn borrowed_original_cell_and_map_transfer_checks_before_moving_custody() {
    let cell = Cell::new(String::from("original"));
    let map = Storage::<u64, String>::from_iter([(1, String::from("original"))]);
    let mut cell_slot = cell.block_acquisition();
    let mut map_slot = map.block_acquisition();
    assert!(!cell_slot.is_initialized() && !map_slot.is_initialized());
    assert!(catch_unwind(AssertUnwindSafe(|| cell_slot.take_block())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| map_slot.take_block())).is_err());
    cell_slot.initialize(BlockMode::Ordinary);
    map_slot.initialize(BlockMode::Ordinary);
    assert!(cell_slot.is_initialized() && map_slot.is_initialized());
    let mut cell_block = cell_slot.take_block();
    let mut map_block = map_slot.take_block();
    assert!(!cell_slot.is_initialized() && !map_slot.is_initialized());
    *cell_block.get_mut() = "cell successor".into();
    map_block.insert(1, "map successor".into());
    let cell_pointer = cell_block.get().as_ptr();
    let map_pointer = map_block.get(&1).unwrap().as_ptr();
    drop((cell_slot, map_slot));
    assert_eq!(cell_block.get().as_ptr(), cell_pointer);
    assert_eq!(map_block.get(&1).unwrap().as_ptr(), map_pointer);
    cell_block.commit();
    map_block.commit();
    assert_eq!(cell.view().get().as_ptr(), cell_pointer);
    assert_eq!(map.view().get(&1).unwrap().as_ptr(), map_pointer);
}
