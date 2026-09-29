//! Actual World baseline/delta equivalence and retained-predecessor controls.

use super::*;
use crate::{smartcontracts::isi::triggers::set::AUTHORITY_FIELDS, state::World};
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
    let plain = WorldStateBaseline::capture_current(&first.block(), &fixture_budget()).unwrap();
    {
        let mut setup = second.block();
        setup
            .smart_contract_state
            .insert(path("untouched/value"), vec![1]);
        *setup.soradns_last_publish_ms.get_mut() = Some(42);
        setup.commit();
    }
    let block = second.block();
    let initial = WorldStateBaseline::capture_current(&block, &fixture_budget()).unwrap();
    assert_ne!(plain.root(), initial.root());
    let canonical_fields = WORLD_FIELDS
        .iter()
        .filter(|field| matches!(field.role, Role::Canonical(_)) && field.id != "world.triggers")
        .count()
        + AUTHORITY_FIELDS
            .iter()
            .filter(|field| matches!(field.role, Role::Canonical(_)))
            .count();
    assert_eq!(plain.fields as usize, canonical_fields);
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
        WorldStateBaseline::capture_current(&noop, &fixture_budget())
            .unwrap()
            .root()
    );
    assert_eq!(initial.root(), initial.apply_block(&noop).unwrap().root());
}

#[test]
fn actual_incremental_versions_match_cold_capture_across_commit_and_replacement() {
    let world = World::default();
    let parent = WorldStateBaseline::capture_current(&world.block(), &fixture_budget()).unwrap();
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
        WorldStateBaseline::capture_current(&block, &fixture_budget())
            .unwrap()
            .root()
    );
    assert_eq!(
        parent.root(),
        WorldStateBaseline::capture_predecessor(&block, &fixture_budget())
            .unwrap()
            .root()
    );
    block.commit();
    assert_eq!(
        first.root(),
        WorldStateBaseline::capture_current(&world.block(), &fixture_budget())
            .unwrap()
            .root()
    );
    // Opening and dropping the intervening read overlay must preserve the undo
    // needed by replacement; the replacement's before cut is the original parent.
    let mut replacement = world.block_and_revert();
    assert_eq!(
        parent_root,
        WorldStateBaseline::capture_current(&replacement, &fixture_budget())
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
        WorldStateBaseline::capture_current(&replacement, &fixture_budget())
            .unwrap()
            .root()
    );
    assert_eq!(
        parent_root,
        WorldStateBaseline::capture_predecessor(&replacement, &fixture_budget())
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
    let parent = WorldStateBaseline::capture_current(&source.block(), &fixture_budget()).unwrap();
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
    assert!(error.to_string().contains("preimage mismatch"));
    assert_eq!(root, parent.root());
    assert_eq!(
        root,
        WorldStateBaseline::capture_current(&source.block(), &fixture_budget())
            .unwrap()
            .root()
    );
    let actual_parent = WorldStateBaseline::capture_predecessor(&block, &fixture_budget()).unwrap();
    assert_eq!(
        actual_parent.apply_block(&block).unwrap().root(),
        WorldStateBaseline::capture_current(&block, &fixture_budget())
            .unwrap()
            .root()
    );
}

#[test]
fn incremental_encoding_is_limited_to_touched_values_and_failures_keep_parent() {
    let storage: Storage<u64, Vec<u8>> = (0..512).map(|key| (key, vec![key as u8])).collect();
    let mut block = storage.block();
    let mut initial = BaselineBuilder::new(MerkleMap::new(&fixture_budget()), Direction::Capture);
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
    let mut cold = BaselineBuilder::new(MerkleMap::new(&fixture_budget()), Direction::Capture);
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
    let mut a = BaselineBuilder::new(MerkleMap::new(&fixture_budget()), Direction::Capture);
    a.append_storage_with("a", &block, hash_value).unwrap();
    let a = a.finish();
    let mut b = BaselineBuilder::new(MerkleMap::new(&fixture_budget()), Direction::Capture);
    b.append_storage_with("b", &block, hash_value).unwrap();
    assert_ne!(a.root(), b.finish().root());
    let cell = mv::cell::Cell::new(Vec::<u8>::new());
    let mut c = BaselineBuilder::new(MerkleMap::new(&fixture_budget()), Direction::Capture);
    c.append_cell_with("a", &cell.block(), hash_value).unwrap();
    let c = c.finish();
    assert_ne!(a.root(), c.root());
    assert_eq!(c.values.len(), 1);
    assert_eq!(a.values.len(), 0);
    let world = World::default();
    let block = world.block();
    let mut baseline = WorldStateBaseline::capture_current(&block, &fixture_budget()).unwrap();
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
    let parent = WorldStateBaseline::capture_current(&block, &fixture_budget()).unwrap();
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
        WorldStateBaseline::capture_current(&block, &fixture_budget())
            .unwrap()
            .root()
    );
    assert_ne!(after.root(), parent.root());
    assert_eq!(
        parent.root(),
        WorldStateBaseline::capture_predecessor(&block, &fixture_budget())
            .unwrap()
            .root()
    );
    assert_eq!(
        after.values.len(),
        parent.values.len() + 1,
        "the action is authoritative; its id and active-id indexes are derived"
    );
}

#[test]
fn derived_index_changes_do_not_create_independent_world_authority() {
    use iroha_model_base::domain::DomainId;
    use std::collections::BTreeSet;

    let world = World::default();
    let mut block = world.block();
    let parent = WorldStateBaseline::capture_current(&block, &fixture_budget()).unwrap();
    block.domains_by_owner.insert(
        iroha_test_samples::ALICE_ID.clone(),
        BTreeSet::from([DomainId::try_new("derived", "only").unwrap()]),
    );
    let current = WorldStateBaseline::capture_current(&block, &fixture_budget()).unwrap();
    assert_eq!(current.root(), parent.root());
    assert_eq!(parent.apply_block(&block).unwrap().root(), parent.root());

    block
        .smart_contract_state
        .insert(path("authority/changed"), vec![1]);
    let changed = WorldStateBaseline::capture_current(&block, &fixture_budget()).unwrap();
    assert_ne!(changed.root(), parent.root());
    assert_eq!(parent.apply_block(&block).unwrap().root(), changed.root());
}

#[test]
fn world_baseline_refuses_unclassified_or_mistyped_fields() {
    assert!(is_world_authority("not_registered", 0).is_err());
    assert!(is_world_authority("parameters", 0).is_err());
    assert_eq!(is_world_authority("domains_by_owner", 0), Ok(false));
    assert_eq!(is_world_authority("smart_contract_state", 0), Ok(true));
}

#[test]
fn world_baseline_schema_binds_declared_codec_and_semantic_identities() {
    use crate::state::authority_registry::{V1_LAYOUT, schema};

    let u32_schema = schema_fingerprint(schema::<u32>()).unwrap();
    let u64_schema = schema_fingerprint(schema::<u64>()).unwrap();
    assert_ne!(u32_schema, u64_schema, "Norito nominal identity is bound");
    let resolver = authority_schemas("musubi_resolver_index", 0)
        .unwrap()
        .expect("canonical resolver table");
    let directory = authority_schemas("musubi_public_directory", 0)
        .unwrap()
        .expect("canonical directory table");
    assert_ne!(
        schema_fingerprint(resolver.1).unwrap(),
        schema_fingerprint(directory.1).unwrap(),
        "independent semantic identities are bound"
    );
    let altered_layout = Schema::Semantic {
        identity: "iroha:state:musubi-resolver-authority:v1",
        encoder: "test",
        layout: crate::state::authority_registry::CanonicalLayout {
            flags: V1_LAYOUT.flags ^ 1,
            ..V1_LAYOUT
        },
    };
    assert_ne!(
        schema_fingerprint(resolver.1).unwrap(),
        schema_fingerprint(altered_layout).unwrap(),
        "canonical layout flags are bound"
    );
    assert!(
        schema_fingerprint(Schema::Required {
            identity: "unresolved",
            obligation: "TODO: test refusal",
        })
        .is_err(),
        "an unresolved value schema cannot enter the baseline"
    );
}

#[test]
fn musubi_availability_baseline_uses_anchor_while_publication_binds_full_row() {
    use iroha_data_model::musubi::{
        ArchiveId, MusubiArchiveAvailabilityV1, MusubiStorageAvailabilityV1,
    };

    let world = World::default();
    let archive_id = ArchiveId::new([0x41; 32]);
    let original = MusubiArchiveAvailabilityV1 {
        archive_id,
        availability: MusubiStorageAvailabilityV1::Unavailable,
        healthy_replicas: 0,
        active_locations: 0,
        finalized_height: 7,
        finalized_block_hash: [0x42; 32],
        index_revision: 9,
    };
    original.validate().unwrap();
    let mut first = world.block();
    first
        .musubi_archive_availability
        .insert(archive_id, original);
    first.commit();

    let mut second = world.block();
    let baseline = WorldStateBaseline::capture_current(&second, &fixture_budget()).unwrap();
    let before_publication = second.publication_state_delta().unwrap();
    let mut derived_change = original;
    derived_change.availability = MusubiStorageAvailabilityV1::BelowQuorum;
    derived_change.healthy_replicas = 1;
    derived_change.active_locations = 1;
    derived_change.validate().unwrap();
    second
        .musubi_archive_availability
        .insert(archive_id, derived_change);
    let after = WorldStateBaseline::capture_current(&second, &fixture_budget()).unwrap();
    assert_eq!(after.root(), baseline.root());
    assert_eq!(baseline.apply_block(&second).unwrap().root(), after.root());
    assert_ne!(
        second.publication_state_delta().unwrap(),
        before_publication
    );

    let mut new_anchor = derived_change;
    new_anchor.index_revision += 1;
    second
        .musubi_archive_availability
        .insert(archive_id, new_anchor);
    assert_ne!(
        WorldStateBaseline::capture_current(&second, &fixture_budget())
            .unwrap()
            .root(),
        baseline.root()
    );
}

fn fixture_budget() -> mv::allocation::AllocationBudget {
    mv::allocation::AllocationBudget::new(64 * 1024 * 1024)
}
