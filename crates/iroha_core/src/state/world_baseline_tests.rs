//! Actual World baseline/delta equivalence and retained-predecessor controls.

use super::*;
use crate::state::World;
use iroha_model_base::state_path::StatePath;
use mv::storage::Storage;
use std::cell::Cell;

fn path(key: &str) -> StatePath {
    key.parse().unwrap()
}

#[test]
fn baseline_binds_untouched_and_snapshot_skipped_values_without_undo_history() {
    let first = World::default();
    let second = World::default();
    let plain = WorldStateBaseline::capture_current(&first.block()).unwrap();
    {
        let mut setup = second.block();
        setup
            .smart_contract_state
            .insert(path("untouched/value"), vec![1]);
        *setup.soradns_last_publish_ms.get_mut() = Some(42);
        setup.commit();
    }
    let block = second.block();
    let initial = WorldStateBaseline::capture_current(&block).unwrap();
    assert_ne!(plain.root(), initial.root());
    assert_eq!(plain.fields, 291);
    assert_eq!(initial.fields, plain.fields);
    assert_eq!(
        first.block().net_state_delta().unwrap(),
        block.net_state_delta().unwrap()
    );
    drop(block);
    // A different undo journal with identical current values cannot change root.
    let mut noop = second.block();
    noop.smart_contract_state
        .insert(path("untouched/value"), vec![9]);
    noop.smart_contract_state
        .insert(path("untouched/value"), vec![1]);
    *noop.soradns_last_publish_ms.get_mut() = Some(42);
    assert_eq!(
        initial.root(),
        WorldStateBaseline::capture_current(&noop).unwrap().root()
    );
    assert_eq!(initial.root(), initial.apply_block(&noop).unwrap().root());
}

#[test]
fn actual_incremental_versions_match_cold_capture_across_commit_and_replacement() {
    let world = World::default();
    let parent = WorldStateBaseline::capture_current(&world.block()).unwrap();
    let parent_root = parent.root();
    let mut block = world.block();
    block.smart_contract_state.insert(path("app/kept"), vec![1]);
    block
        .smart_contract_state
        .insert(path("app/removed"), vec![]);
    *block.soradns_last_publish_ms.get_mut() = Some(7);
    {
        let mut aborted = block.smart_contract_state.transaction();
        aborted.insert(path("app/aborted"), vec![9]);
    }
    let first = parent.apply_block(&block).unwrap();
    assert_eq!(
        first.root(),
        WorldStateBaseline::capture_current(&block).unwrap().root()
    );
    assert_eq!(
        parent.root(),
        WorldStateBaseline::capture_predecessor(&block)
            .unwrap()
            .root()
    );
    block.commit();
    assert_eq!(
        first.root(),
        WorldStateBaseline::capture_current(&world.block())
            .unwrap()
            .root()
    );
    // Opening and dropping the intervening read overlay must preserve the undo
    // needed by replacement; the replacement's before cut is the original parent.
    let mut replacement = world.block_and_revert();
    assert_eq!(
        parent_root,
        WorldStateBaseline::capture_current(&replacement)
            .unwrap()
            .root()
    );
    replacement
        .smart_contract_state
        .insert(path("app/replacement"), vec![3]);
    *replacement.soradns_last_publish_ms.get_mut() = Some(8);
    let replaced = parent.apply_block(&replacement).unwrap();
    assert_eq!(
        replaced.root(),
        WorldStateBaseline::capture_current(&replacement)
            .unwrap()
            .root()
    );
    assert_eq!(
        parent_root,
        WorldStateBaseline::capture_predecessor(&replacement)
            .unwrap()
            .root()
    );
    replacement.commit();
    let mut next = world.block();
    next.smart_contract_state.remove(path("app/replacement"));
    *next.soradns_last_publish_ms.get_mut() = None;
    assert_eq!(parent_root, replaced.apply_block(&next).unwrap().root());
    assert_eq!(parent_root, parent.root());
}

#[test]
fn stale_touched_preimage_rejects_the_whole_candidate_without_changing_parent() {
    let source = World::default();
    let other = World::default();
    {
        let mut setup = other.block();
        setup
            .smart_contract_state
            .insert(path("app/stale"), vec![99]);
        setup.commit();
    }
    let parent = WorldStateBaseline::capture_current(&source.block()).unwrap();
    let root = parent.root();
    let mut block = other.block();
    block
        .smart_contract_state
        .insert(path("app/first"), vec![1]);
    block
        .smart_contract_state
        .insert(path("app/stale"), vec![2]);
    let error = match parent.apply_block(&block) {
        Err(error) => error,
        Ok(_) => panic!("stale parent accepted"),
    };
    assert!(error.contains("preimage mismatch"));
    assert_eq!(root, parent.root());
    assert_eq!(
        root,
        WorldStateBaseline::capture_current(&source.block())
            .unwrap()
            .root()
    );
    let actual_parent = WorldStateBaseline::capture_predecessor(&block).unwrap();
    assert_eq!(
        actual_parent.apply_block(&block).unwrap().root(),
        WorldStateBaseline::capture_current(&block).unwrap().root()
    );
}

#[test]
fn incremental_encoding_is_limited_to_touched_values_and_failures_keep_parent() {
    let storage: Storage<u64, Vec<u8>> = (0..512).map(|key| (key, vec![key as u8])).collect();
    let mut block = storage.block();
    let mut initial = BaselineBuilder::new(MerkleMap::new(), Direction::Capture);
    initial
        .append_storage_with("test", &block, hash_value)
        .unwrap();
    let initial = initial.finish();
    let root = initial.root();
    block.insert(17, vec![9]);
    let calls = Cell::new(0);
    let mut next = BaselineBuilder::new(initial.values.clone(), Direction::Forward);
    next.append_storage_with("test", &block, |value| {
        calls.set(calls.get() + 1);
        hash_value(value)
    })
    .unwrap();
    assert_eq!(
        calls.get(),
        2,
        "encode just one before/after pair, not 512 entries"
    );
    let next = next.finish();
    let mut cold = BaselineBuilder::new(MerkleMap::new(), Direction::Capture);
    cold.append_storage_with("test", &block, hash_value)
        .unwrap();
    assert_eq!(next.root(), cold.finish().root());
    let mut failed = BaselineBuilder::new(initial.values.clone(), Direction::Forward);
    assert!(
        failed
            .append_storage_with("test", &block, |_| Err("injected encoding error".into()))
            .is_err()
    );
    assert_eq!(initial.root(), root);
    assert_ne!(initial.root(), next.root());
}

#[test]
fn field_schema_kind_and_absence_are_bound_even_for_empty_stores() {
    let storage: Storage<u64, Vec<u8>> = Storage::new();
    let block = storage.block();
    let mut a = BaselineBuilder::new(MerkleMap::new(), Direction::Capture);
    a.append_storage_with("a", &block, hash_value).unwrap();
    let a = a.finish();
    let mut b = BaselineBuilder::new(MerkleMap::new(), Direction::Capture);
    b.append_storage_with("b", &block, hash_value).unwrap();
    assert_ne!(a.root(), b.finish().root());
    let cell = mv::cell::Cell::new(Vec::<u8>::new());
    let mut c = BaselineBuilder::new(MerkleMap::new(), Direction::Capture);
    c.append_cell_with("a", &cell.block(), hash_value).unwrap();
    let c = c.finish();
    assert_ne!(a.root(), c.root());
    assert_eq!(c.values.len(), 1);
    assert_eq!(a.values.len(), 0);
    let world = World::default();
    let block = world.block();
    let mut baseline = WorldStateBaseline::capture_current(&block).unwrap();
    baseline.schema = Hash::new(b"foreign schema");
    assert!(baseline.apply_block(&block).is_err());
}

#[test]
fn actual_trigger_stores_share_the_complete_baseline_visitor() {
    use crate::smartcontracts::isi::triggers::specialized::{
        SpecializedAction, SpecializedTrigger,
    };
    use iroha_data_model::{events::execute_trigger::ExecuteTriggerEventFilter, prelude::*};
    let world = World::default();
    let mut block = world.block();
    let parent = WorldStateBaseline::capture_current(&block).unwrap();
    {
        let mut tx = block.triggers.transaction();
        let action = SpecializedAction::new(
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "baseline".to_owned(),
                ))]
                .into(),
            ),
            Repeats::Exactly(3),
            iroha_test_samples::ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new("baseline".parse().unwrap(), action))
                .unwrap()
        );
        tx.apply();
    }
    let after = parent.apply_block(&block).unwrap();
    assert_eq!(
        after.root(),
        WorldStateBaseline::capture_current(&block).unwrap().root()
    );
    assert_ne!(after.root(), parent.root());
    assert_eq!(
        parent.root(),
        WorldStateBaseline::capture_predecessor(&block)
            .unwrap()
            .root()
    );
    assert_eq!(
        after.values.len(),
        parent.values.len() + 3,
        "action, id and active id"
    );
}
