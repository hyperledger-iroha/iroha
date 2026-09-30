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
    assert_eq!(captured.snapshot.entries.len() as u64, original.entries());
    assert!(
        captured
            .snapshot
            .entries
            .iter()
            .all(|entry| entry.field_id.starts_with("world.")
                && (entry.kind == WorldStateElementKindV1::Table) == entry.key_hash.is_some())
    );
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
    // Exercise callback invalidation while the original cut still owns this generation.
    let callback_entered = Cell::new(false);
    let error = state
        .with_native_world_state_snapshot_v1(&original_tip, &asset, &budget, |_, _, _, _| {
            callback_entered.set(true);
            let mut publication = state.state_view_publication();
            let _writer = publication.begin();
            Ok(())
        })
        .unwrap_err();
    assert!(callback_entered.get());
    assert!(error.contains("generation changed"), "{error}");
    assert_eq!(budget.reserved_bytes(), 0);
    // Advancing publication cannot grant the old journal authority over a new generation.
    let error = state
        .with_native_world_state_snapshot_v1(&original_tip, &asset, &budget, |_, _, _, _| {
            called.set(true);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("another certified generation"), "{error}");
    assert!(!called.get());
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
