//! Complete cold publication, original allocation ownership and certified-cut refusal.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{Identifiable, asset::AssetBalancePolicy, domain::Domain, isi::Register};
use iroha_model_base::domain::DomainId;
use std::cell::Cell;

#[test]
fn complete_cold_snapshot_matches_original_accumulator_and_registry() {
    let world = World::new();
    let overlay = world.block();
    let original = WorldStateAccumulator::capture(&overlay).unwrap();
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let captured = capture(&overlay, &original, &budget).unwrap();
    assert_eq!(captured.snapshot.root().unwrap(), original.root().unwrap());
    assert_eq!(
        captured.snapshot.schema_hash,
        field_index().as_ref().unwrap().schema
    );
    assert_eq!(
        captured.snapshot.schema_hash,
        State::native_world_schema_hash_v1().unwrap()
    );
    assert_eq!(captured.snapshot.entries.len() as u64, original.entries());
    assert!(captured.snapshot.entries.iter().all(|entry| {
        field_index()
            .as_ref()
            .unwrap()
            .ids
            .contains(&entry.field_id.as_str())
            && (entry.kind == WorldStateElementKindV1::Table) == entry.key_hash.is_some()
    }));
    // Untouched canonical cells must be present even when every table is empty.
    assert!(captured.snapshot.entries.iter().any(|entry| entry.field_id
        == "world.kagemusha_verifier_registry"
        && entry.kind == WorldStateElementKindV1::Cell));
    assert!(
        budget.reserved_bytes() > 0,
        "snapshot retains its original charges"
    );
    drop(captured);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "all snapshot storage is physically gone"
    );
}

#[test]
fn complete_cold_snapshot_preserves_actual_trigger_child_registry_identity() {
    use crate::smartcontracts::isi::triggers::specialized::{
        SpecializedAction, SpecializedTrigger,
    };
    use iroha_data_model::{events::execute_trigger::ExecuteTriggerEventFilter, prelude::*};
    let world = World::new();
    let mut overlay = world.block();
    {
        let mut transaction = overlay.triggers.transaction();
        let action = SpecializedAction::new(
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "complete native trigger cut".into(),
                ))]
                .into(),
            ),
            Repeats::Exactly(3),
            iroha_test_samples::ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            transaction
                .add_by_call_trigger(SpecializedTrigger::new(
                    "snapshot_cut".parse().unwrap(),
                    action,
                ))
                .unwrap()
        );
        transaction.apply();
    }
    let original = WorldStateAccumulator::capture(&overlay).unwrap();
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let captured = capture(&overlay, &original, &budget).unwrap();
    let row = captured
        .snapshot
        .entries
        .iter()
        .find(|entry| entry.field_id == "triggers.by_call")
        .unwrap();
    assert_eq!(row.kind, WorldStateElementKindV1::Table);
    assert!(row.key_hash.is_some());
    assert!(
        !captured
            .snapshot
            .entries
            .iter()
            .any(|entry| entry.field_id == "triggers.ids"
                || entry.field_id.starts_with("triggers.active_"))
    );
    assert_eq!(captured.snapshot.root().unwrap(), original.root().unwrap());
    drop(captured);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn complete_cold_snapshot_refuses_changed_accumulator_and_refunds_original_pool() {
    let world = World::new();
    let overlay = world.block();
    let mut original = WorldStateAccumulator::capture(&overlay).unwrap();
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    original.lanes[0] = original.lanes[0].wrapping_add(1);
    assert!(capture(&overlay, &original, &budget).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    original.entries += 1;
    assert!(capture(&overlay, &original, &budget).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn complete_cold_snapshot_has_no_replacement_budget_on_refusal() {
    let world = World::new();
    let overlay = world.block();
    let original = WorldStateAccumulator::capture(&overlay).unwrap();
    let budget = AllocationBudget::new(0);
    assert!(capture(&overlay, &original, &budget).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(SnapshotCollector::new(&budget, MAX_WORLD_STATE_SNAPSHOT_ENTRIES_V1 + 1, 1).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn snapshot_collector_rejects_omission_duplicate_and_excess_elements() {
    let budget = AllocationBudget::new(1024 * 1024);
    let schema = Hash::new(b"schema");
    let mut duplicate = SnapshotCollector::new(&budget, 2, 1).unwrap();
    for _ in 0..2 {
        duplicate
            .push(
                "world.test",
                WorldStateElementKindV1::Cell,
                None,
                Hash::new(b"v"),
            )
            .unwrap();
    }
    assert!(duplicate.finish(schema).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
    let mut excess = SnapshotCollector::new(&budget, 1, 1).unwrap();
    excess
        .push(
            "world.test",
            WorldStateElementKindV1::Cell,
            None,
            Hash::new(b"v"),
        )
        .unwrap();
    assert!(
        excess
            .push(
                "world.test",
                WorldStateElementKindV1::Cell,
                None,
                Hash::new(b"v")
            )
            .is_err()
    );
    drop(excess);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        SnapshotCollector::new(&budget, 1, 1)
            .unwrap()
            .finish(schema)
            .is_err()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

fn asset_chain() -> (CertifiedTestChain, AssetDefinitionId) {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    let domain = DomainId::try_new("snapshot", "universal").unwrap();
    let asset = AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
    config.genesis_instructions = vec![
        Register::domain(Domain::new(domain)).into(),
        Register::asset_definition(AssetDefinition::numeric(
            asset.clone(),
            "Snapshot coin",
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
    ];
    let mut chain = CertifiedTestChain::start(config).unwrap();
    chain.commit_at(2_000, Vec::new());
    (chain, asset)
}

#[test]
fn publisher_authenticates_original_native_cut_and_borrows_exact_targets() {
    let (mut chain, asset) = asset_chain();
    let original_tip = chain.committed(2);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let state = chain.state();
    let generation = state.state_view_generation();
    state
        .with_native_world_state_snapshot_v1(
            &original_tip,
            &asset,
            &budget,
            |snapshot, definition, incarnation, registry| {
                assert_eq!(
                    snapshot.schema_hash,
                    State::native_world_schema_hash_v1().unwrap()
                );
                assert_eq!(
                    snapshot.root().unwrap(),
                    original_tip.commitment().execution.world_state_root
                );
                assert_eq!(definition.id(), &asset);
                for (field, kind, key, value) in [
                    (
                        "world.asset_definitions",
                        WorldStateElementKindV1::Table,
                        Some(hash_value(&asset).unwrap()),
                        hash_value(definition).unwrap(),
                    ),
                    (
                        "world.axt_asset_incarnations",
                        WorldStateElementKindV1::Table,
                        Some(hash_value(&asset).unwrap()),
                        hash_value(incarnation).unwrap(),
                    ),
                    (
                        "world.kagemusha_verifier_registry",
                        WorldStateElementKindV1::Cell,
                        None,
                        hash_value(registry).unwrap(),
                    ),
                ] {
                    assert!(
                        snapshot.entries.iter().any(|entry| entry.field_id == field
                            && entry.kind == kind
                            && entry.key_hash == key
                            && entry.value_hash == value),
                        "{field}"
                    );
                }
                assert!(budget.reserved_bytes() > 0);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        state.state_view_generation(),
        generation,
        "read-only overlay publishes nothing"
    );
    assert_eq!(budget.reserved_bytes(), 0);
    let called = Cell::new(false);
    let absent = AssetDefinitionId::derive_from_components(
        DomainId::try_new("snapshot", "universal").unwrap(),
        "absent".parse().unwrap(),
    );
    assert!(
        state
            .with_native_world_state_snapshot_v1(&original_tip, &absent, &budget, |_, _, _, _| {
                called.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(!called.get());
    assert!(
        state
            .with_native_world_state_snapshot_v1(
                &original_tip,
                &asset,
                &AllocationBudget::new(0),
                |_, _, _, _| {
                    called.set(true);
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!called.get());
    // A data callback's output cannot escape after its original generation changes.
    assert!(
        state
            .with_native_world_state_snapshot_v1(&original_tip, &asset, &budget, |_, _, _, _| {
                let mut publication = state.state_view_publication();
                let _writer = publication.begin();
                Ok(())
            })
            .unwrap_err()
            .contains("generation changed")
    );
    {
        let mut publication = state.state_view_publication();
        let _writer = publication.begin();
        assert!(
            state
                .with_native_world_state_snapshot_v1(
                    &original_tip,
                    &asset,
                    &budget,
                    |_, _, _, _| {
                        called.set(true);
                        Ok(())
                    }
                )
                .is_err()
        );
        assert!(!called.get());
    }
    assert_eq!(budget.reserved_bytes(), 0);
    chain.commit_at(3_000, Vec::new());
    assert!(
        chain
            .state()
            .with_native_world_state_snapshot_v1(&original_tip, &asset, &budget, |_, _, _, _| {
                called.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(
        !called.get(),
        "retired native tip must refuse before invoking the callback"
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn cold_reconstruction_removes_tail_insertions_and_restores_deletions_and_updates() {
    use super::super::world_state_cut::JournalCapture;
    use iroha_model_base::state_path::StatePath;
    let (chain, _) = asset_chain();
    let native = chain.state().view().native_execution_tip().unwrap();
    let world = World::new();
    let path = |value: &str| value.parse::<StatePath>().unwrap();
    {
        let mut seed = world.block();
        seed.smart_contract_state
            .insert(path("app/deleted"), vec![1]);
        seed.smart_contract_state
            .insert(path("app/updated"), vec![2]);
        let accumulator = WorldStateAccumulator::capture(&seed).unwrap();
        *seed.state_accumulator.get_mut() = accumulator;
        seed.commit();
    }
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let mut block = world.block();
    block
        .smart_contract_state
        .insert(path("app/updated"), vec![3]);
    let cut = JournalCapture::capture(&block, false, &budget).unwrap();
    let at_result = WorldStateAccumulator::capture(&block).unwrap();
    block.smart_contract_state.remove(path("app/deleted"));
    block
        .smart_contract_state
        .insert(path("app/updated"), vec![4]);
    block
        .smart_contract_state
        .insert(path("app/tail-only"), vec![5]);
    block.advance_state_accumulator(false).unwrap();
    block.begin_freeze();
    block.finish_freeze();
    let capsule = cut.prepare(&block, native, 2, &budget).unwrap();
    let applied = capture(&block, block.state_accumulator.get(), &budget).unwrap();
    let certified = reconstruct(&applied, &capsule, &budget).unwrap();
    assert_eq!(
        certified.snapshot.root().unwrap(),
        at_result.root().unwrap()
    );
    let target = hash_value(&path("app/updated")).unwrap();
    require_target(
        &certified.snapshot,
        "world.smart_contract_state",
        WorldStateElementKindV1::Table,
        Some(target),
        hash_value(&vec![3_u8]).unwrap(),
    )
    .unwrap();
    assert!(
        require_target(
            &certified.snapshot,
            "world.smart_contract_state",
            WorldStateElementKindV1::Table,
            Some(target),
            hash_value(&vec![4_u8]).unwrap()
        )
        .is_err()
    );
    let deleted = hash_value(&path("app/deleted")).unwrap();
    require_target(
        &certified.snapshot,
        "world.smart_contract_state",
        WorldStateElementKindV1::Table,
        Some(deleted),
        hash_value(&vec![1_u8]).unwrap(),
    )
    .unwrap();
    let inserted = hash_value(&path("app/tail-only")).unwrap();
    assert!(
        !certified
            .snapshot
            .entries
            .iter()
            .any(|row| row.field_id == "world.smart_contract_state"
                && row.key_hash == Some(inserted))
    );
    drop(certified);
    drop(applied);
    drop(capsule);
    drop(cut);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn cold_reconstruction_rejects_missing_changed_or_foreign_complete_applied_rows() {
    use super::super::world_state_cut::JournalCapture;
    use iroha_model_base::state_path::StatePath;
    let (chain, _) = asset_chain();
    let native = chain.state().view().native_execution_tip().unwrap();
    let world = World::new();
    {
        let mut seed = world.block();
        let accumulator = WorldStateAccumulator::capture(&seed).unwrap();
        *seed.state_accumulator.get_mut() = accumulator;
        seed.commit();
    }
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let mut block = world.block();
    let cut = JournalCapture::capture(&block, false, &budget).unwrap();
    let key = "app/tail".parse::<StatePath>().unwrap();
    block.smart_contract_state.insert(key.clone(), vec![9]);
    block.advance_state_accumulator(false).unwrap();
    block.begin_freeze();
    block.finish_freeze();
    let capsule = cut.prepare(&block, native, 2, &budget).unwrap();
    let mut applied = capture(&block, block.state_accumulator.get(), &budget).unwrap();
    let original = applied.snapshot.entries.clone(); // Explicit test-only mutation specimen.
    let position = applied
        .snapshot
        .entries
        .iter()
        .position(|row| {
            row.field_id == "world.smart_contract_state"
                && row.key_hash == Some(hash_value(&key).unwrap())
        })
        .unwrap();
    applied.snapshot.entries[position].value_hash = Hash::new(b"substituted post-tail preimage");
    assert!(reconstruct(&applied, &capsule, &budget).is_err());
    applied.snapshot.entries = original;
    applied.snapshot.entries.remove(position);
    assert!(reconstruct(&applied, &capsule, &budget).is_err());
    assert!(reconstruct(&applied, &capsule, &AllocationBudget::new(0)).is_err());
}

#[test]
fn publisher_requires_original_capture_after_snapshot_restore_or_raw_commit() {
    let (chain, asset) = asset_chain();
    let tip = chain.committed(2);
    let state = chain.state();
    // Dropping the private original models the truthful decoded-restoration
    // boundary; a matching bare stored World root is never replacement authority.
    *state.native_world_cut.lock() = None;
    let called = Cell::new(false);
    let error = state
        .with_native_world_state_snapshot_v1(
            &tip,
            &asset,
            &AllocationBudget::new(16 * 1024 * 1024),
            |_, _, _, _| {
                called.set(true);
                Ok(())
            },
        )
        .unwrap_err();
    assert!(error.contains("requires native replay"));
    assert!(!called.get());
}
