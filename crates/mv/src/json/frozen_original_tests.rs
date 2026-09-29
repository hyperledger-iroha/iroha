//! Attached and frozen originals share the exact current/undo snapshot codec.

use super::*;
use crate::BlockMode;

#[test]
fn frozen_original_snapshot_bytes_preserve_modes_noops_deletions_and_bounded_errors() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        for changed in [false, true] {
            let cell = Cell::new(String::from("base"));
            let map = Storage::<String, u64>::from_iter([("base".into(), 7), ("remove".into(), 8)]);
            let mut tip = cell.block();
            *tip.get_mut() = "tip".into();
            tip.commit();
            let mut tip = map.block();
            tip.insert("base".into(), 9);
            tip.commit();
            let mut cell_block = if mode == BlockMode::Ordinary {
                cell.block()
            } else {
                cell.block_and_revert()
            };
            let mut map_block = if mode == BlockMode::Ordinary {
                map.block()
            } else {
                map.block_and_revert()
            };
            if changed {
                *cell_block.get_mut() = "candidate".into();
                map_block.insert("base".into(), 11);
                map_block.remove("remove".to_owned());
                map_block.insert("new".into(), 13);
                map_block.remove("absent".to_owned());
            }
            let cell_json = json::to_json(&cell_block).unwrap();
            let map_json = json::to_json(&map_block).unwrap();
            let cell_short = json::to_json_bounded(&cell_block, 1);
            let map_short = json::to_json_bounded(&map_block, 1);
            assert!(cell_short.is_err() && map_short.is_err());
            let cell_original = cell_block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            let map_original = map_block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            assert_eq!(json::to_json(&cell_original).unwrap(), cell_json);
            assert_eq!(json::to_json(&map_original).unwrap(), map_json);
            assert_eq!(json::to_json_bounded(&cell_original, 1), cell_short);
            assert_eq!(json::to_json_bounded(&map_original, 1), map_short);
            let cell_roundtrip: Cell<String> = json::from_json(&cell_json).unwrap();
            let map_roundtrip: Storage<String, u64> = json::from_json(&map_json).unwrap();
            assert_eq!(json::to_json(&cell_roundtrip).unwrap(), cell_json);
            assert_eq!(json::to_json(&map_roundtrip).unwrap(), map_json);
            // Inspecting/restoring independent snapshots never authorizes the
            // original source owner or substitutes its publication predecessor.
            assert!(!cell_original.matches_current(&cell_roundtrip));
            assert!(!map_original.matches_current(&map_roundtrip));
        }
    }
}
