//! Checked borrowed projections over the exact original MV current/undo images.

use super::*;
use crate::BlockMode;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::SeqCst},
};

fn exact_and_short(value: &impl JsonSerialize) {
    let ordinary = json::to_json(value).unwrap();
    assert_eq!(
        json::to_json_bounded(value, ordinary.len()),
        Ok(ordinary.clone())
    );
    assert_eq!(
        json::to_json_bounded(value, ordinary.len() - 1),
        Err(json::BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(json::to_json(value).unwrap(), ordinary);
}

#[test]
fn checked_live_current_and_undo_match_ordinary_bytes() {
    let map: Storage<String, u64> = json::from_str(
        r#"{"revert":{"absent":null,"old":{"value":3}},"blocks":{"live":7,"old":5}}"#,
    )
    .unwrap();
    let cell: Cell<Option<u64>> = json::from_str(r#"{"revert":null,"blocks":null}"#).unwrap();
    exact_and_short(&map);
    exact_and_short(&cell);
    let cell: Cell<Option<u64>> = json::from_str(r#"{"revert":null,"blocks":11}"#).unwrap();
    exact_and_short(&cell);
    let retained_null: Cell<Option<u64>> =
        json::from_str(r#"{"revert":{"value":null},"blocks":11}"#).unwrap();
    exact_and_short(&retained_null);
    let retained_null: Storage<String, Option<u64>> = json::from_str(
        r#"{"revert":{"absent":null,"present":{"value":null}},"blocks":{"present":11}}"#,
    )
    .unwrap();
    exact_and_short(&retained_null);
    assert_eq!(map.view().get("old"), Some(&5));
}

#[test]
fn checked_attached_and_detached_originals_preserve_modes_deletions_and_owner() {
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
            let cell_identity = cell_block.publication_identity();
            let map_identity = map_block.publication_identity();
            let cell_pointer = cell_block.get().as_ptr();
            let cell_before = cell_block
                .original_undo()
                .as_ref()
                .map(|value| value.as_ptr());
            let map_pointers: Vec<_> = map_block
                .iter()
                .map(|(key, value)| (key.as_ptr(), std::ptr::from_ref(value)))
                .collect();
            let cell_json = json::to_json(&cell_block).unwrap();
            let map_json = json::to_json(&map_block).unwrap();
            exact_and_short(&cell_block);
            exact_and_short(&map_block);
            let original_cell = cell_block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            let original_map = map_block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            assert_eq!(original_cell.get().as_ptr(), cell_pointer);
            assert_eq!(
                original_cell
                    .original_undo()
                    .as_ref()
                    .map(|value| value.as_ptr()),
                cell_before
            );
            assert_eq!(original_cell.publication_identity(), cell_identity);
            assert_eq!(
                original_map.original_images().publication_identity(),
                map_identity
            );
            assert_eq!(
                original_map
                    .original_images()
                    .current_entries()
                    .map(|(key, value)| (key.as_ptr(), std::ptr::from_ref(value)))
                    .collect::<Vec<_>>(),
                map_pointers
            );
            // A newer target value may not refresh the immutable captured originals.
            let mut next_cell = cell.block();
            *next_cell.get_mut() = "newer-target".into();
            next_cell.commit();
            let mut next_map = map.block();
            next_map.insert("target-only".into(), 99);
            next_map.commit();
            exact_and_short(&original_cell);
            exact_and_short(&original_map);
            assert_eq!(json::to_json(&original_cell).unwrap(), cell_json);
            assert_eq!(json::to_json(&original_map).unwrap(), map_json);
            assert_eq!(original_cell.get().as_ptr(), cell_pointer);
            assert_eq!(original_cell.publication_identity(), cell_identity);
            assert_eq!(
                original_map.original_images().publication_identity(),
                map_identity
            );
            assert!(!original_cell.matches_current(&cell));
            assert!(!original_map.matches_current(&map));
        }
    }
}

#[derive(Debug)]
struct ObservedLeaf {
    value: u64,
    clones: Arc<AtomicUsize>,
    writes: Arc<AtomicUsize>,
}
impl Clone for ObservedLeaf {
    fn clone(&self) -> Self {
        self.clones.fetch_add(1, SeqCst);
        Self {
            value: self.value,
            clones: Arc::clone(&self.clones),
            writes: Arc::clone(&self.writes),
        }
    }
}
impl JsonSerialize for ObservedLeaf {
    fn json_serialize(&self, out: &mut String) {
        self.value.json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.writes.fetch_add(1, SeqCst);
        self.value.json_serialize_to(out)
    }
}

#[test]
fn checked_byte_cap_refuses_before_original_leaf_and_preserves_charged_pair() {
    use iroha_allocation::{AllocationBudget, AllocationCharge};
    let layouts = Cell::<ObservedLeaf, AllocationCharge>::allocation_layouts();
    let pair = layouts.iter().map(|layout| layout.size()).sum();
    let budget = AllocationBudget::new(pair);
    let mut charge = budget.try_reserve_layouts(layouts).unwrap();
    let clones = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let cell = Cell::from_values_charged(
        ObservedLeaf {
            value: 17,
            clones: Arc::clone(&clones),
            writes: Arc::clone(&writes),
        },
        None,
        crate::cell::CellAllocationCharges::new(
            charge.try_split(layouts[0]).unwrap(),
            charge.try_split(layouts[1]).unwrap(),
        ),
    );
    drop(charge);
    assert_eq!(budget.reserved_bytes(), pair);
    assert_eq!(
        json::to_json_bounded(&cell, 0),
        Err(json::BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(writes.load(SeqCst), 0);
    assert_eq!(clones.load(SeqCst), 0);
    assert_eq!(budget.reserved_bytes(), pair);
    let ordinary = json::to_json(&cell).unwrap();
    assert_eq!(json::to_json_bounded(&cell, ordinary.len()), Ok(ordinary));
    assert_eq!(
        writes.load(SeqCst),
        2,
        "one leaf visit in each sole count/fill pass"
    );
    assert_eq!(clones.load(SeqCst), 0);
    assert_eq!(budget.reserved_bytes(), pair);
}

#[test]
fn checked_inherited_destination_refusal_retains_same_cell_and_retries() {
    let cell = Cell::new(17_u64);
    let ordinary = json::to_json(&cell).unwrap();
    let pointer = {
        let view = cell.view();
        std::ptr::from_ref(&*view)
    };
    let refused = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || json::to_json_bounded(&cell, ordinary.len()),
    );
    assert_eq!(
        refused,
        Err(json::BoundedJsonError::DecodeResource(
            norito::core::DecodeResourceError::TotalAllocationExceeded {
                attempted: ordinary.len() as u64,
                limit: 0
            },
        ))
    );
    assert_eq!(json::to_json(&cell).unwrap(), ordinary);
    assert_eq!(
        {
            let view = cell.view();
            std::ptr::from_ref(&*view)
        },
        pointer
    );
    assert_eq!(json::to_json_bounded(&cell, ordinary.len()), Ok(ordinary));
}

struct RefusingDepthSink {
    count: usize,
    limit: usize,
    depth: usize,
}
impl json::JsonWriteSink for RefusingDepthSink {
    fn push(&mut self, value: char) -> Result<(), json::BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0; 4]))
    }
    fn push_str(&mut self, value: &str) -> Result<(), json::BoundedJsonError> {
        if self.count + value.len() > self.limit {
            return Err(json::BoundedJsonError::BodyTooLarge);
        }
        self.count += value.len();
        Ok(())
    }
    fn begin_container(&mut self) -> Result<(), json::BoundedJsonError> {
        self.depth += 1;
        Ok(())
    }
    fn end_container(&mut self) {
        self.depth -= 1;
    }
}
#[test]
fn checked_map_refusal_balances_all_entered_container_depths() {
    let map = Storage::<String, u64>::from_iter([("key".to_owned(), 17)]);
    let ordinary = json::to_json(&map).unwrap();
    for limit in 0..ordinary.len() {
        let mut sink = RefusingDepthSink {
            count: 0,
            limit,
            depth: 0,
        };
        assert_eq!(
            map.json_serialize_to(&mut sink),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
        assert_eq!(sink.depth, 0, "returned refusal at byte cap {limit}");
    }
    fn check_nested_original(value: &impl JsonSerialize) {
        let ordinary = json::to_json(value).unwrap();
        for limit in 0..ordinary.len() {
            let mut sink = RefusingDepthSink {
                count: 0,
                limit,
                depth: 7,
            };
            assert_eq!(
                value.json_serialize_to(&mut sink),
                Err(json::BoundedJsonError::BodyTooLarge)
            );
            assert_eq!(
                sink.depth, 7,
                "nested refusal must retain inherited depth at byte cap {limit}"
            );
        }
    }
    let cell: Cell<Option<u64>> =
        json::from_str(r#"{"revert":{"value":null},"blocks":11}"#).unwrap();
    check_nested_original(&cell);
    let retained: Storage<String, Option<u64>> = json::from_str(
        r#"{"revert":{"absent":null,"present":{"value":null}},"blocks":{"present":11}}"#,
    )
    .unwrap();
    check_nested_original(&retained);
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct UnmigratedKey(u64);
impl JsonKeyCodec for UnmigratedKey {
    fn encode_json_key(&self, _: &mut String) {
        panic!("checked key must never use String fallback");
    }
    fn decode_json_key(_: &str) -> Result<Self, json::Error> {
        Err(json::Error::Message("not needed by writer control".into()))
    }
}
#[derive(Clone, Debug)]
struct UnmigratedLeaf;
impl JsonSerialize for UnmigratedLeaf {
    fn json_serialize(&self, _: &mut String) {
        panic!("checked leaf must never use String fallback");
    }
}
#[test]
fn checked_unmigrated_key_and_leaf_refuse_without_string_fallback() {
    let map = Storage::<UnmigratedKey, u64>::from_iter([(UnmigratedKey(1), 7)]);
    assert_eq!(
        json::to_json_bounded(&map, 1024),
        Err(json::BoundedJsonError::Unsupported)
    );
    let cell = Cell::new(UnmigratedLeaf);
    assert_eq!(
        json::to_json_bounded(&cell, 1024),
        Err(json::BoundedJsonError::Unsupported)
    );
}

fn key_map<T: JsonKeyCodec + Key>(key: T) {
    let map = Storage::<T, u64>::from_iter([(key, 11)]);
    exact_and_short(&map);
}
#[test]
fn checked_builtin_map_keys_preserve_upper_hex_and_tuple_quoting() {
    key_map("escape\"\\\n\u{1f}😀".to_owned());
    key_map(u64::MAX);
    key_map([0xab, 0x00, 0xff]);
    key_map(("left\"\\\u{1f}😀".to_owned(), "right".to_owned()));
    key_map(("one".to_owned(), "two\n".to_owned(), "three".to_owned()));
    key_map(("circuit".to_owned(), u32::MAX));
    key_map(("service".to_owned(), "version".to_owned(), u16::MAX));
    key_map(("a\u{1f}\"b\\c".to_owned(), u64::MAX, 0));
    key_map(([0xff, 0], 0, u64::MAX));
    key_map((("left".to_owned(), "right".to_owned()), 1, 2));
}

#[test]
fn checked_storage_streams_original_values_without_cloning_or_replacing_them() {
    let clones = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let map = Storage::<u64, ObservedLeaf>::from_iter([(
        7,
        ObservedLeaf {
            value: 17,
            clones: Arc::clone(&clones),
            writes: Arc::clone(&writes),
        },
    )]);
    clones.store(0, SeqCst);
    let pointer = {
        let view = map.view();
        std::ptr::from_ref(view.get(&7).unwrap())
    };
    let ordinary = json::to_json(&map).unwrap();
    assert_eq!(
        json::to_json_bounded(&map, 0),
        Err(json::BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(writes.load(SeqCst), 0);
    assert_eq!(json::to_json_bounded(&map, ordinary.len()), Ok(ordinary));
    assert_eq!(writes.load(SeqCst), 2);
    assert_eq!(clones.load(SeqCst), 0);
    assert_eq!(
        {
            let view = map.view();
            std::ptr::from_ref(view.get(&7).unwrap())
        },
        pointer
    );
}

#[test]
fn checked_present_value_frame_preserves_owned_null_and_manual_payload() {
    let null = mv_snapshot_undo_value(None::<u64>);
    exact_and_short(&null);
    let clones = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let manual = mv_snapshot_undo_value(ObservedLeaf {
        value: 17,
        clones: Arc::clone(&clones),
        writes: Arc::clone(&writes),
    });
    let original = std::ptr::from_ref(&manual.value);
    exact_and_short(&manual);
    assert_eq!(std::ptr::from_ref(&manual.value), original);
    assert_eq!(clones.load(SeqCst), 0);
    assert!(writes.load(SeqCst) > 0);
}

fn mv_snapshot_undo_value<V>(value: V) -> super::SnapshotUndoValue<V> {
    super::SnapshotUndoValue { value }
}

// These reuse the existing test module's RefusingDepthSink/exact_and_short and imports.
fn assert_original_nested_refusal_depth(value: &impl JsonSerialize) {
    let ordinary = json::to_json(value).unwrap();
    for limit in 0..ordinary.len() {
        let mut sink = RefusingDepthSink {
            count: 0,
            limit,
            depth: 7,
        };
        assert_eq!(
            value.json_serialize_to(&mut sink),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
        assert_eq!(
            sink.depth, 7,
            "original nested current/undo refusal at cap {limit}"
        );
    }
}

#[test]
fn checked_original_container_leaves_balance_current_and_present_undo_refusals() {
    let cell: Cell<Vec<u64>> = json::from_str(r#"{"revert":{"value":[3]},"blocks":[17]}"#).unwrap();
    let map: Storage<String, Vec<u64>> =
        json::from_str(r#"{"revert":{"old":{"value":[3]},"absent":null},"blocks":{"old":[17]}}"#)
            .unwrap();
    assert_original_nested_refusal_depth(&cell);
    assert_original_nested_refusal_depth(&map);
    exact_and_short(&cell);
    exact_and_short(&map);
}

struct ManualNoFastNoClone(u64);
impl JsonSerialize for ManualNoFastNoClone {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}
#[test]
fn checked_present_frame_accepts_manual_payload_without_fast_writer_or_clone_bound() {
    let value = SnapshotUndoValue {
        value: ManualNoFastNoClone(17),
    };
    let pointer = std::ptr::from_ref(&value.value);
    exact_and_short(&value);
    assert_eq!(std::ptr::from_ref(&value.value), pointer);
    assert_eq!(
        json::to_json_bounded(&value, 12),
        Ok(String::from("{\"value\":17}"))
    );
}
